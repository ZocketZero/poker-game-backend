use std::collections::HashMap;

use poker_engine::table::TableConfig;
use poker_engine::{Action, GameEvent, Player, Table};
use tokio::sync::mpsc;

use crate::game::messages::{SeatInfo, ServerMessage};

/// A connected player at a table.
#[derive(Debug, Clone)]
pub struct ConnectedPlayer {
    pub user_id: String,
    pub username: String,
    pub sender: mpsc::UnboundedSender<ServerMessage>,
}

/// Wraps `poker_engine::Table` with connected player tracking and message broadcasting.
pub struct GameTable {
    pub id: String,
    pub name: String,
    pub engine: Table,
    /// seat index -> connected player
    pub players: HashMap<usize, ConnectedPlayer>,
}

impl GameTable {
    pub fn new(id: String, name: String, config: TableConfig) -> Self {
        Self {
            id,
            name,
            engine: Table::new(config),
            players: HashMap::new(),
        }
    }

    /// Sit a player at the table and connect their WebSocket sender.
    pub fn sit_player(
        &mut self,
        seat: usize,
        user_id: String,
        username: String,
        buy_in: u64,
        sender: mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<(), String> {
        if self.players.contains_key(&seat) {
            return Err(format!("Seat {} is already occupied", seat));
        }

        let player = Player::new(seat, &username, buy_in);
        self.engine.sit_player(seat, player)?;

        self.players.insert(
            seat,
            ConnectedPlayer {
                user_id,
                username,
                sender,
            },
        );

        Ok(())
    }

    /// Remove a player from the table.
    pub fn remove_player(&mut self, seat: usize) -> Option<ConnectedPlayer> {
        self.engine.remove_player(seat);
        self.players.remove(&seat)
    }

    /// Find which seat a user is sitting at.
    pub fn find_seat_by_user(&self, user_id: &str) -> Option<usize> {
        self.players
            .iter()
            .find(|(_, p)| p.user_id == user_id)
            .map(|(&seat, _)| seat)
    }

    /// Start a new hand and broadcast all resulting events.
    pub fn start_hand(&mut self) -> Result<(), String> {
        self.engine.start_hand()?;
        self.broadcast_events();
        Ok(())
    }

    /// Apply a player action and broadcast all resulting events.
    pub fn apply_action(&mut self, action: Action) -> Result<(), String> {
        self.engine.apply_action(action)?;
        self.broadcast_events();
        Ok(())
    }

    /// Drain engine events and dispatch them to connected players.
    fn broadcast_events(&self) {
        for event in &self.engine.events {
            // Send private hole cards only to the owning player
            if let GameEvent::HoleCardsDealt { player_id, cards } = event {
                if let Some(connected) = self.players.get(player_id) {
                    let _ = connected.sender.send(ServerMessage::HoleCards {
                        table_id: self.id.clone(),
                        cards: *cards,
                    });
                }
                continue;
            }

            // Send YourTurn only to the acting player
            if let GameEvent::PlayerTurn {
                player_id,
                legal_actions,
            } = event
            {
                if let Some(connected) = self.players.get(player_id) {
                    let _ = connected.sender.send(ServerMessage::YourTurn {
                        table_id: self.id.clone(),
                        legal_actions: legal_actions.clone(),
                    });
                }
                // Still broadcast a generic event (without legal_actions) to all
            }

            // Broadcast the event as JSON to all players at the table
            let event_json = serde_json::to_value(event).unwrap_or_default();
            let msg = ServerMessage::GameEvent {
                table_id: self.id.clone(),
                event: event_json,
            };

            for connected in self.players.values() {
                let _ = connected.sender.send(msg.clone());
            }
        }
    }

    /// Build a table state snapshot (for new joiners).
    pub fn build_table_state(&self) -> ServerMessage {
        let seats: Vec<SeatInfo> = (0..self.engine.config.max_players)
            .map(|i| {
                if let Some(p) = self.engine.player(i) {
                    let username = self
                        .players
                        .get(&i)
                        .map(|cp| cp.username.clone());
                    SeatInfo {
                        seat: i,
                        username,
                        chips: Some(p.chips),
                        status: Some(format!("{:?}", p.status)),
                        current_bet: Some(p.current_bet),
                    }
                } else {
                    SeatInfo {
                        seat: i,
                        username: None,
                        chips: None,
                        status: None,
                        current_bet: None,
                    }
                }
            })
            .collect();

        let board: Vec<String> = self.engine.board.iter().map(|c| c.to_string()).collect();

        ServerMessage::TableState {
            table_id: self.id.clone(),
            seats,
            stage: format!("{:?}", self.engine.stage),
            board,
            pot: self.engine.pot_manager.total_pot(),
            current_player: self.engine.current_player,
        }
    }

    /// Number of connected players.
    pub fn player_count(&self) -> usize {
        self.players.len()
    }
}
