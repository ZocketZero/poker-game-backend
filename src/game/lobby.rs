use std::collections::HashMap;
use std::sync::Arc;

use poker_engine::table::TableConfig;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

use crate::game::messages::{ServerMessage, TableInfo};
use crate::game::table_actor::GameTable;

/// Manages all active tables and player sessions.
pub struct Lobby {
    /// table_id -> GameTable
    tables: HashMap<String, Arc<RwLock<GameTable>>>,
}

impl Lobby {
    pub fn new() -> Self {
        Self {
            tables: HashMap::new(),
        }
    }

    /// Create a new table and return its ID.
    pub fn create_table(&mut self, config: TableConfig) -> String {
        let table_id = Uuid::new_v4().to_string();
        let table_name = format!("Table-{}", &table_id[..8]);
        let game_table = GameTable::new(table_id.clone(), table_name, config);
        self.tables
            .insert(table_id.clone(), Arc::new(RwLock::new(game_table)));
        log::info!("Created table {}", table_id);
        table_id
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
            });
        }
        infos
    }

    /// Get a reference to a table by ID.
    pub fn get_table(&self, table_id: &str) -> Option<Arc<RwLock<GameTable>>> {
        self.tables.get(table_id).cloned()
    }

    /// Join a player to a table.
    pub async fn join_table(
        &self,
        table_id: &str,
        seat: usize,
        user_id: String,
        username: String,
        buy_in: u64,
        sender: mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<(), String> {
        let table_lock = self
            .tables
            .get(table_id)
            .ok_or_else(|| format!("Table '{}' not found", table_id))?;

        let mut table = table_lock.write().await;

        // Check if user is already seated
        if table.find_seat_by_user(&user_id).is_some() {
            return Err("You are already seated at this table".to_string());
        }

        // Notify existing players
        let join_msg = ServerMessage::PlayerJoined {
            table_id: table_id.to_string(),
            seat,
            username: username.clone(),
            chips: buy_in,
        };
        for connected in table.players.values() {
            let _ = connected.sender.send(join_msg.clone());
        }

        table.sit_player(seat, user_id, username, buy_in, sender)?;

        Ok(())
    }

    /// Remove a player from a table.
    pub async fn leave_table(
        &self,
        table_id: &str,
        user_id: &str,
    ) -> Result<(), String> {
        let table_lock = self
            .tables
            .get(table_id)
            .ok_or_else(|| format!("Table '{}' not found", table_id))?;

        let mut table = table_lock.write().await;

        let seat = table
            .find_seat_by_user(user_id)
            .ok_or_else(|| "You are not seated at this table".to_string())?;

        let removed = table.remove_player(seat);

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

        Ok(())
    }

    /// Start a hand at a table.
    pub async fn start_hand(&self, table_id: &str) -> Result<(), String> {
        let table_lock = self
            .tables
            .get(table_id)
            .ok_or_else(|| format!("Table '{}' not found", table_id))?;

        let mut table = table_lock.write().await;
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
