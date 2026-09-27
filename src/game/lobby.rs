use std::collections::HashMap;
use std::sync::Arc;

use mongodb::Database;
use poker_engine::table::TableConfig;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

use crate::game::messages::{GameMode, ServerMessage, TableInfo};
use crate::game::table_actor::GameTable;

/// Manages all active tables and player sessions.
pub struct Lobby {
    /// table_id -> GameTable
    tables: HashMap<String, Arc<RwLock<GameTable>>>,
    db: Option<Database>,
}

impl Lobby {
    pub fn new(db: Option<Database>) -> Self {
        Self {
            tables: HashMap::new(),
            db,
        }
    }

    /// Create a new table or tournament room and return its ID.
    pub fn create_table(
        &mut self,
        config: TableConfig,
        game_mode: GameMode,
        starting_chips: Option<u64>,
    ) -> Result<String, String> {
        if config.max_players < 2 || config.max_players > 10 {
            return Err("max_players must be between 2 and 10".to_string());
        }
        if config.big_blind == 0 {
            return Err("big_blind must be greater than 0".to_string());
        }
        if config.small_blind > config.big_blind {
            return Err("small_blind cannot be greater than big_blind".to_string());
        }

        let starting_chips = starting_chips.unwrap_or(1000);
        if starting_chips == 0 {
            return Err("starting_chips must be greater than 0".to_string());
        }

        let table_id = Uuid::new_v4().to_string();
        let prefix = match game_mode {
            GameMode::Cash => "Table",
            GameMode::Tournament => "Tournament",
        };
        let table_name = format!("{}-{}", prefix, &table_id[..8]);
        let game_table = GameTable::new(
            table_id.clone(),
            table_name,
            config,
            self.db.clone(),
            game_mode,
            starting_chips,
        );
        self.tables
            .insert(table_id.clone(), Arc::new(RwLock::new(game_table)));
        log::info!("Created {:?} table {}", game_mode, table_id);
        Ok(table_id)
    }

    /// List all tables with their metadata.
    pub async fn list_tables(&self) -> Vec<TableInfo> {
        let mut infos = Vec::new();
        for (id, table_lock) in &self.tables {
            let table = table_lock.read().await;
            infos.push(TableInfo {
                id: id.clone(),
                name: table.name.clone(),
                player_count: table.player_count(),
                max_players: table.engine.config.max_players,
                small_blind: table.engine.config.small_blind,
                big_blind: table.engine.config.big_blind,
                stage: format!("{:?}", table.engine.stage),
                game_mode: table.game_mode,
                is_started: table.is_started,
                starting_chips: if table.game_mode == GameMode::Tournament {
                    Some(table.starting_chips)
                } else {
                    None
                },
            });
        }
        infos
    }

    /// Get a reference to a table by ID.
    pub fn get_table(&self, table_id: &str) -> Option<Arc<RwLock<GameTable>>> {
        self.tables.get(table_id).cloned()
    }

    /// Join a player to a table. Returns the number of chips actually required/charged.
    pub async fn join_table(
        &self,
        table_id: &str,
        seat: usize,
        user_id: String,
        username: String,
        buy_in: u64,
        sender: mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<u64, String> {
        let table_lock = self
            .tables
            .get(table_id)
            .ok_or_else(|| format!("Table '{}' not found", table_id))?;

        let mut table = table_lock.write().await;

        // Check if user is already seated at another seat on this table
        if let Some(existing_seat) = table.find_seat_by_user(&user_id) {
            if existing_seat != seat {
                return Err("You are already seated at another seat at this table".to_string());
            }
        }

        // Sit the player first to validate seat availability, bounds, and tournament start state
        let actual_chips = table.sit_player(seat, user_id, username.clone(), buy_in, sender)?;

        // Notify other seated players of the new joiner
        let join_msg = ServerMessage::PlayerJoined {
            table_id: table_id.to_string(),
            seat,
            username,
            chips: actual_chips,
        };
        for (&s, connected) in &table.players {
            if s != seat {
                let _ = connected.sender.send(join_msg.clone());
            }
        }

        Ok(actual_chips)
    }

    /// Remove a player from a table and return the chips they leave with.
    pub async fn leave_table(
        &self,
        table_id: &str,
        user_id: &str,
    ) -> Result<u64, String> {
        let table_lock = self
            .tables
            .get(table_id)
            .ok_or_else(|| format!("Table '{}' not found", table_id))?;

        let mut table = table_lock.write().await;

        let seat = table
            .find_seat_by_user(user_id)
            .ok_or_else(|| "You are not seated at this table".to_string())?;

        let (removed, chips) = table.leave_seat(seat)?;

        // Notify remaining players
        if let Some(player) = removed {
            let leave_msg = ServerMessage::PlayerLeft {
                table_id: table_id.to_string(),
                seat,
                username: player.username,
            };
            for connected in table.players.values() {
                let _ = connected.sender.send(leave_msg.clone());
            }
        }

        Ok(chips)
    }

    /// Remove a user from all tables they are seated at (e.g. on disconnect).
    /// Returns vector of (table_id, returned_chips).
    pub async fn leave_all_tables(&self, user_id: &str) -> Vec<(String, u64)> {
        let mut results = Vec::new();
        for (table_id, table_lock) in &self.tables {
            let mut table = table_lock.write().await;
            if let Some(seat) = table.find_seat_by_user(user_id) {
                // In tournament mode, if tournament is running, do not remove player stack
                // (they stay in tournament to be blinded out / auto-fold until eliminated).
                if table.game_mode == GameMode::Tournament && table.is_started {
                    continue;
                }

                let (removed, chips) = if table.engine.stage == poker_engine::events::Stage::HandEnded
                    || table.engine.player(seat).map_or(true, |p| !p.is_in_hand())
                {
                    table.leave_seat(seat).unwrap_or((None, 0))
                } else if table.engine.current_player == Some(seat) {
                    let _ = table.apply_action(poker_engine::Action::Fold);
                    table.leave_seat(seat).unwrap_or((None, 0))
                } else {
                    continue;
                };

                if let Some(player) = removed {
                    let leave_msg = ServerMessage::PlayerLeft {
                        table_id: table_id.clone(),
                        seat,
                        username: player.username,
                    };
                    for connected in table.players.values() {
                        let _ = connected.sender.send(leave_msg.clone());
                    }
                    results.push((table_id.clone(), chips));
                }
            }
        }
        results
    }

    /// Start a hand at a table.
    /// `user_id` must belong to a player already seated at the table.
    pub async fn start_hand(&self, table_id: &str, user_id: &str) -> Result<(), String> {
        let table_lock = self
            .tables
            .get(table_id)
            .ok_or_else(|| format!("Table '{}' not found", table_id))?;

        let mut table = table_lock.write().await;

        // Only a seated player may trigger the hand start.
        table
            .find_seat_by_user(user_id)
            .ok_or_else(|| "You must be seated at the table to start a hand".to_string())?;

        table.start_hand()
    }

    /// Apply a player action at a table.
    pub async fn player_action(
        &self,
        table_id: &str,
        user_id: &str,
        action: poker_engine::Action,
    ) -> Result<(), String> {
        let table_lock = self
            .tables
            .get(table_id)
            .ok_or_else(|| format!("Table '{}' not found", table_id))?;

        let mut table = table_lock.write().await;

        // Verify it's this player's turn
        let seat = table
            .find_seat_by_user(user_id)
            .ok_or_else(|| "You are not seated at this table".to_string())?;

        if table.engine.current_player != Some(seat) {
            return Err("It is not your turn".to_string());
        }

        table.apply_action(action)
    }

    /// Get the table state snapshot for a specific table.
    pub async fn get_table_state(&self, table_id: &str) -> Result<ServerMessage, String> {
        let table_lock = self
            .tables
            .get(table_id)
            .ok_or_else(|| format!("Table '{}' not found", table_id))?;

        let table = table_lock.read().await;
        Ok(table.build_table_state())
    }
}
