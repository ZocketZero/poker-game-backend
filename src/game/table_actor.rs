use std::collections::HashMap;

use chrono::Utc;
use mongodb::Database;
use poker_engine::table::TableConfig;
use poker_engine::{Action, GameEvent, Player, Table};
use tokio::sync::mpsc;

use crate::db::models::{HandHistoryDoc, HandPlayer};
use crate::game::messages::{GameMode, SeatInfo, ServerMessage, SidePotInfo};
use crate::repositories::user_repository;

/// A connected player at a table.
#[derive(Debug, Clone)]
pub struct ConnectedPlayer {
    pub user_id: String,
    pub username: String,
    pub sender: mpsc::UnboundedSender<ServerMessage>,
}

/// In-progress hand data for hand history recording.
pub struct CurrentHand {
    pub hand_number: u64,
    pub starting_players: Vec<HandPlayer>,
    pub events: Vec<serde_json::Value>,
}

/// Wraps `poker_engine::Table` with connected player tracking, tournament management, and message broadcasting.
pub struct GameTable {
    pub id: String,
    pub name: String,
    pub creator_id: String,
    pub creator_username: String,
    pub engine: Table,
    /// seat index -> connected player
    pub players: HashMap<usize, ConnectedPlayer>,
    pub db: Option<Database>,
    pub current_hand: Option<CurrentHand>,
    pub game_mode: GameMode,
    pub starting_chips: u64,
    pub is_started: bool,
    pub prize_pool: u64,
    pub auto_start_epoch: u64,
    pub is_auto_start_scheduled: bool,
}

impl GameTable {
    pub fn new(
        id: String,
        name: String,
        creator_id: String,
        creator_username: String,
        config: TableConfig,
        db: Option<Database>,
        game_mode: GameMode,
        starting_chips: u64,
    ) -> Self {
        Self {
            id,
            name,
            creator_id,
            creator_username,
            engine: Table::new(config),
            players: HashMap::new(),
            db,
            current_hand: None,
            game_mode,
            starting_chips,
            is_started: false,
            prize_pool: 0,
            auto_start_epoch: 0,
            is_auto_start_scheduled: false,
        }
    }

    /// Sit a player at the table and connect their WebSocket sender.
    /// In tournament mode, validates that the game hasn't started and assigns equal starting chips.
    pub fn sit_player(
        &mut self,
        seat: usize,
        user_id: String,
        username: String,
        buy_in: u64,
        sender: mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<u64, String> {
        if seat >= self.engine.config.max_players {
            return Err(format!(
                "Seat {} is out of table bounds (max {})",
                seat, self.engine.config.max_players
            ));
        }

        // In tournament mode, new players cannot join after the game has begun
        if self.game_mode == GameMode::Tournament && self.is_started {
            let is_reconnecting = self
                .players
                .get(&seat)
                .map_or(false, |p| p.user_id == user_id && p.sender.is_closed());

            if !is_reconnecting {
                return Err("Cannot join room: tournament has already started".to_string());
            }
        }

        // Check if this seat already has a player
        if let Some(existing) = self.players.get_mut(&seat) {
            // If the same user is reconnecting with a closed sender, reattach
            if existing.user_id == user_id && existing.sender.is_closed() {
                existing.sender = sender;
                let chips = self.engine.player(seat).map_or(0, |p| p.chips);
                return Ok(chips);
            }
            return Err(format!("Seat {} is already occupied", seat));
        }

        if self.engine.player(seat).is_some() {
            return Err(format!("Seat {} is already occupied", seat));
        }

        // In tournament mode, every player starts with the exact same starting chips
        let chips_assigned = if self.game_mode == GameMode::Tournament {
            self.starting_chips
        } else {
            buy_in
        };

        let player = Player::new(seat, &username, chips_assigned);
        self.engine.sit_player(seat, player)?;

        if self.game_mode == GameMode::Tournament {
            self.prize_pool += chips_assigned;
        }

        self.players.insert(
            seat,
            ConnectedPlayer {
                user_id,
                username,
                sender,
            },
        );

        Ok(chips_assigned)
    }

    /// Remove a player from the table and return their connection info and remaining chips.
    pub fn remove_player(&mut self, seat: usize) -> (Option<ConnectedPlayer>, u64) {
        let connected = self.players.remove(&seat);
        let chips = if let Some(player) = self.engine.remove_player(seat) {
            player.chips
        } else {
            0
        };
        (connected, chips)
    }

    /// Process a player's request to leave their seat.
    /// In tournament mode before game start, unregisters with full refund.
    /// In tournament mode after game start, forfeits (0 refund).
    pub fn leave_seat(&mut self, seat: usize) -> Result<(Option<ConnectedPlayer>, u64), String> {
        if self.game_mode == GameMode::Tournament {
            if !self.is_started {
                // Unregister before tournament starts: refund starting chips
                self.prize_pool = self.prize_pool.saturating_sub(self.starting_chips);
                let (connected, _) = self.remove_player(seat);
                return Ok((connected, self.starting_chips));
            } else {
                // Tournament already running: forfeit (0 cash refund)
                if self.engine.stage != poker_engine::events::Stage::HandEnded
                    && self.engine.player(seat).is_some_and(|p| p.is_in_hand())
                {
                    if self.engine.current_player == Some(seat) {
                        let _ = self.apply_action(Action::Fold);
                    }
                }
                let (connected, _) = self.remove_player(seat);
                return Ok((connected, 0));
            }
        }

        // Cash game leave logic
        if self.engine.stage != poker_engine::events::Stage::HandEnded
            && self.engine.player(seat).is_some_and(|p| p.is_in_hand())
        {
            if self.engine.current_player == Some(seat) {
                let _ = self.apply_action(Action::Fold);
            } else {
                return Err("Cannot leave table while involved in an active hand. Please fold on your turn first.".to_string());
            }
        }
        Ok(self.remove_player(seat))
    }

    /// Find which seat a user is sitting at.
    pub fn find_seat_by_user(&self, user_id: &str) -> Option<usize> {
        self.players
            .iter()
            .find(|(_, p)| p.user_id == user_id)
            .map(|(&seat, _)| seat)
    }

    /// Start a new hand and broadcast all resulting events.
    /// Marks the tournament as started so no new players can join.
    pub fn start_hand(&mut self) -> Result<(), String> {
        if self.game_mode == GameMode::Tournament {
            if self.player_count() < 2 {
                return Err("At least 2 players are required to start a tournament".to_string());
            }
            self.is_started = true;
        }

        let starting_players: Vec<HandPlayer> = self
            .players
            .iter()
            .filter_map(|(&seat, cp)| {
                self.engine.player(seat).map(|p| HandPlayer {
                    user_id: cp.user_id.clone(),
                    username: cp.username.clone(),
                    seat,
                    starting_chips: p.chips,
                    ending_chips: p.chips,
                })
            })
            .collect();

        self.engine.start_hand()?;

        self.current_hand = Some(CurrentHand {
            hand_number: self.engine.hand_count,
            starting_players,
            events: Vec::new(),
        });

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
    fn broadcast_events(&mut self) {
        let events: Vec<GameEvent> = self.engine.events.drain(..).collect();
        let mut auto_actions = Vec::new();

        for event in &events {
            let event_json = serde_json::to_value(event).unwrap_or_default();
            if let Some(hand) = &mut self.current_hand {
                hand.events.push(event_json.clone());
            }

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

            // Send YourTurn only to the acting player; if disconnected, queue auto action
            if let GameEvent::PlayerTurn {
                player_id,
                legal_actions,
            } = event
            {
                let is_disconnected = self
                    .players
                    .get(player_id)
                    .map_or(true, |c| c.sender.is_closed());

                if is_disconnected {
                    let action = if legal_actions.can_check {
                        Action::Check
                    } else {
                        Action::Fold
                    };
                    auto_actions.push(action);
                } else if let Some(connected) = self.players.get(player_id) {
                    let _ = connected.sender.send(ServerMessage::YourTurn {
                        table_id: self.id.clone(),
                        legal_actions: legal_actions.clone(),
                    });
                }
            }

            // Broadcast generic public event (without private legal_actions) to all players
            let public_event_json = if let GameEvent::PlayerTurn { player_id, .. } = event {
                serde_json::json!({
                    "PlayerTurn": {
                        "player_id": player_id
                    }
                })
            } else {
                event_json
            };

            let msg = ServerMessage::GameEvent {
                table_id: self.id.clone(),
                event: public_event_json,
            };

            for connected in self.players.values() {
                let _ = connected.sender.send(msg.clone());
            }

            if matches!(event, GameEvent::HandEnded) {
                self.finish_hand();
            }
        }

        // Apply auto-actions for disconnected players
        for action in auto_actions {
            let _ = self.apply_action(action);
        }
    }

    /// Complete current hand, persist history doc to DB, handle tournament eliminations/winner,
    /// and clean up disconnected players.
    fn finish_hand(&mut self) {
        if let Some(mut hand) = self.current_hand.take() {
            for p in &mut hand.starting_players {
                p.ending_chips = self
                    .engine
                    .player(p.seat)
                    .map_or(0, |engine_p| engine_p.chips);
            }
            let doc = HandHistoryDoc {
                id: None,
                table_id: self.id.clone(),
                hand_number: hand.hand_number,
                players: hand.starting_players,
                events: hand.events,
                created_at: Utc::now(),
            };
            if let Some(db) = &self.db {
                let db = db.clone();
                tokio::spawn(async move {
                    if let Err(e) = user_repository::save_hand_history(&db, &doc).await {
                        log::error!("Failed to save hand history: {e}");
                    }
                });
            }
        }

        // Tournament elimination & winner logic
        if self.game_mode == GameMode::Tournament && self.is_started {
            // Find players with 0 chips
            let mut eliminated = Vec::new();
            for (&seat, cp) in &self.players {
                if let Some(engine_p) = self.engine.player(seat) {
                    if engine_p.chips == 0 {
                        eliminated.push((seat, cp.clone()));
                    }
                }
            }

            let remaining_after = self.players.len().saturating_sub(eliminated.len());
            for (seat, cp) in eliminated {
                let rank = remaining_after + 1;
                self.remove_player(seat);

                let elim_msg = ServerMessage::PlayerEliminated {
                    table_id: self.id.clone(),
                    seat,
                    username: cp.username.clone(),
                    rank,
                };
                for connected in self.players.values() {
                    let _ = connected.sender.send(elim_msg.clone());
                }
                let _ = cp.sender.send(elim_msg);
            }

            // Check if tournament has a winner (only 1 player remains)
            if self.players.len() == 1 {
                let (&_winner_seat, winner) = self.players.iter().next().unwrap();
                let prize = self.prize_pool;
                let winner_username = winner.username.clone();

                let ended_msg = ServerMessage::TournamentEnded {
                    table_id: self.id.clone(),
                    winner_username: winner_username.clone(),
                    prize,
                };
                for connected in self.players.values() {
                    let _ = connected.sender.send(ended_msg.clone());
                }

                if let Some(db) = &self.db {
                    let db = db.clone();
                    let uname = winner_username;
                    tokio::spawn(async move {
                        if let Ok(Some(u)) =
                            user_repository::find_user_by_username(&db, &uname).await
                        {
                            let _ = user_repository::update_chips(
                                &db,
                                &uname,
                                u.chips.saturating_add(prize),
                            )
                            .await;
                        }
                    });
                }
            }
            return;
        }

        // Cash game: remove disconnected players after hand finishes and refund chips
        let disconnected_seats: Vec<usize> = self
            .players
            .iter()
            .filter(|(_, cp)| cp.sender.is_closed())
            .map(|(&seat, _)| seat)
            .collect();

        for seat in disconnected_seats {
            let (cp_opt, chips) = self.remove_player(seat);
            if let Some(cp) = cp_opt {
                let leave_msg = ServerMessage::PlayerLeft {
                    table_id: self.id.clone(),
                    seat,
                    username: cp.username.clone(),
                };
                for connected in self.players.values() {
                    let _ = connected.sender.send(leave_msg.clone());
                }
                if let Some(db) = &self.db {
                    let db = db.clone();
                    let username = cp.username;
                    tokio::spawn(async move {
                        if let Ok(Some(u)) =
                            user_repository::find_user_by_username(&db, &username).await
                        {
                            let _ = user_repository::update_chips(
                                &db,
                                &username,
                                u.chips.saturating_add(chips),
                            )
                            .await;
                        }
                    });
                }
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
                        .map(|cp| cp.username.clone())
                        .or_else(|| Some(p.name.clone()));
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

        // Build side-pot breakdown: eligible = players still in the hand (Active or AllIn).
        let eligible: std::collections::HashSet<usize> = self
            .engine
            .seats
            .iter()
            .enumerate()
            .filter_map(|(idx, p)| p.as_ref().filter(|p| p.is_in_hand()).map(|_| idx))
            .collect();
        let raw_pots = self.engine.pot_manager.calculate_pots(&eligible);
        let side_pots: Vec<SidePotInfo> = raw_pots
            .into_iter()
            .map(|pot| {
                let mut eligible_seats: Vec<usize> = pot.eligible_players.into_iter().collect();
                eligible_seats.sort_unstable();
                SidePotInfo {
                    amount: pot.amount,
                    eligible_seats,
                }
            })
            .collect();

        ServerMessage::TableState {
            table_id: self.id.clone(),
            seats,
            stage: format!("{:?}", self.engine.stage),
            board,
            pot: self.engine.pot_manager.total_pot(),
            side_pots,
            current_player: self.engine.current_player,
            game_mode: self.game_mode,
            is_started: self.is_started,
            creator_id: Some(self.creator_id.clone()),
            creator_username: Some(self.creator_username.clone()),
        }
    }

    /// Number of connected players who have chips (used for lobby display).
    /// Players with 0 chips who are still seated (e.g. waiting for elimination) are excluded.
    pub fn player_count(&self) -> usize {
        self.players
            .iter()
            .filter(|&(&seat, _)| self.engine.player(seat).map_or(false, |p| p.chips > 0))
            .count()
    }
}
