use std::collections::HashMap;
use std::sync::Arc;

use mongodb::Database;
use poker_engine::Action;
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
        creator_id: String,
        creator_username: String,
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
            creator_id,
            creator_username,
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
                creator_id: Some(table.creator_id.clone()),
                creator_username: Some(table.creator_username.clone()),
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
        if let Some(existing_seat) = table.find_seat_by_user(&user_id)
            && existing_seat != seat
        {
            return Err("You are already seated at another seat at this table".to_string());
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
        drop(table);

        self.check_and_schedule_auto_start(table_id).await;

        Ok(actual_chips)
    }

    /// Remove a player from a table and return the chips they leave with.
    pub async fn leave_table(&self, table_id: &str, user_id: &str) -> Result<u64, String> {
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
        drop(table);

        self.check_and_schedule_turn_timer(table_id);
        self.check_and_schedule_auto_start(table_id).await;

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

                let (removed, chips) = if table.engine.stage
                    == poker_engine::events::Stage::HandEnded
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

        for (table_id, _) in &results {
            self.check_and_schedule_turn_timer(table_id);
            self.check_and_schedule_auto_start(table_id).await;
        }

        results
    }

    /// Start a hand at a table.
    /// In tournament mode, only the creator who created the room can start the tournament for the first time.
    /// In cash mode, any seated player may trigger manual hand start (though it auto-starts on 2+ players).
    pub async fn start_hand(&self, table_id: &str, user_id: &str) -> Result<(), String> {
        let table_lock = self
            .tables
            .get(table_id)
            .ok_or_else(|| format!("Table '{}' not found", table_id))?;

        let mut table = table_lock.write().await;

        if table.game_mode == GameMode::Tournament {
            if table.is_started {
                return Err("Tournament has already started".to_string());
            }
            if user_id != table.creator_id {
                return Err("Only the room creator can start the tournament".to_string());
            }
            if table.player_count() < 2 {
                return Err("At least 2 players are required to start a tournament".to_string());
            }
            table.is_started = true;
        } else {
            // Cash mode: only a seated player may trigger manual hand start
            table
                .find_seat_by_user(user_id)
                .ok_or_else(|| "You must be seated at the table to start a hand".to_string())?;
        }

        let res = table.start_hand();
        drop(table);
        res?;

        self.check_and_schedule_turn_timer(table_id);
        Ok(())
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

        let res = table.apply_action(action);
        drop(table);
        res?;

        self.check_and_schedule_turn_timer(table_id);
        self.check_and_schedule_auto_start(table_id).await;
        Ok(())
    }

    /// Check table conditions and schedule auto-start of the next hand if appropriate.
    pub async fn check_and_schedule_auto_start(&self, table_id: &str) {
        if let Some(table_lock) = self.tables.get(table_id) {
            Self::check_and_schedule_auto_start_on_table(table_lock.clone()).await;
        }
    }

    /// Check table conditions on an Arc<RwLock<GameTable>> and schedule auto-start.
    pub async fn check_and_schedule_auto_start_on_table(table_lock: Arc<RwLock<GameTable>>) {
        let mut table = table_lock.write().await;

        // Must be in HandEnded stage
        if table.engine.stage != poker_engine::events::Stage::HandEnded {
            return;
        }

        // Must not already have a timer scheduled
        if table.is_auto_start_scheduled {
            return;
        }

        // Check if criteria for starting are met
        let should_start = match table.game_mode {
            GameMode::Cash => table.player_count() >= 2,
            GameMode::Tournament => table.is_started && table.player_count() >= 2,
        };

        if !should_start {
            return;
        }

        table.is_auto_start_scheduled = true;
        table.auto_start_epoch += 1;
        let epoch = table.auto_start_epoch;
        let table_id_str = table.id.clone();

        // 1.5s for initial table start, 3s between consecutive hands to view showdown/results
        let delay_ms = if table.engine.hand_count == 0 {
            1500
        } else {
            3000
        };
        let delay = std::time::Duration::from_millis(delay_ms);

        let table_lock_for_task = table_lock.clone();
        drop(table);

        tokio::spawn(async move {
            tokio::time::sleep(delay).await;

            let mut table = table_lock_for_task.write().await;
            if table.auto_start_epoch != epoch {
                return;
            }
            table.is_auto_start_scheduled = false;

            if table.engine.stage != poker_engine::events::Stage::HandEnded {
                return;
            }

            let can_start = match table.game_mode {
                GameMode::Cash => table.player_count() >= 2,
                GameMode::Tournament => table.is_started && table.player_count() >= 2,
            };

            if can_start {
                if let Err(e) = table.start_hand() {
                    log::error!(
                        "Failed to auto-start hand for table {}: {}",
                        table_id_str,
                        e
                    );
                } else {
                    log::info!("Auto-started hand for table {}", table_id_str);
                    Self::check_and_schedule_turn_timer_on_table(table_lock_for_task.clone());
                }
            }
        });
    }

    /// Check if a player has an active turn and spawn a timer to auto-fold (or check) if they time out.
    pub fn check_and_schedule_turn_timer_on_table(table_lock: Arc<RwLock<GameTable>>) {
        tokio::spawn(async move {
            let (epoch, timeout_secs, current_seat) = {
                let table = table_lock.read().await;
                if table.engine.stage == poker_engine::events::Stage::HandEnded {
                    return;
                }
                match table.engine.current_player {
                    Some(seat) => (table.turn_epoch, table.turn_timeout_secs, seat),
                    None => return,
                }
            };

            let duration = std::time::Duration::from_secs(timeout_secs);
            tokio::time::sleep(duration).await;

            let mut table = table_lock.write().await;
            if table.turn_epoch != epoch
                || table.engine.stage == poker_engine::events::Stage::HandEnded
                || table.engine.current_player != Some(current_seat)
            {
                return;
            }

            log::info!(
                "Decision time limit reached for seat {} on table {}. Auto-acting.",
                current_seat,
                table.id
            );

            let can_check = table
                .engine
                .legal_actions(current_seat)
                .map_or(false, |la| la.can_check);
            let action = if can_check {
                Action::Check
            } else {
                Action::Fold
            };

            if let Err(e) = table.apply_action(action) {
                log::error!(
                    "Failed to apply auto-timeout action {:?} for seat {} on table {}: {}",
                    action,
                    current_seat,
                    table.id,
                    e
                );
            } else {
                drop(table);
                Self::check_and_schedule_turn_timer_on_table(table_lock.clone());
                Self::check_and_schedule_auto_start_on_table(table_lock.clone()).await;
            }
        });
    }

    /// Check if a player has an active turn and spawn a timer for the given table ID.
    pub fn check_and_schedule_turn_timer(&self, table_id: &str) {
        if let Some(table_lock) = self.tables.get(table_id) {
            Self::check_and_schedule_turn_timer_on_table(table_lock.clone());
        }
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
