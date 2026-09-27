use actix_web::{Error, HttpRequest, HttpResponse, web};
use actix_ws::Message;
use futures_util::StreamExt;
use poker_engine::table::TableConfig;
use tokio::sync::mpsc;

use crate::auth;
use crate::db::repository;
use crate::game::messages::{ClientMessage, ServerMessage};
use crate::AppState;

#[derive(Debug, serde::Deserialize)]
pub struct WsQuery {
    token: String,
}

/// GET /ws?token=<JWT>
///
/// Upgrades the HTTP connection to a WebSocket.
/// Authenticates via JWT token in the query string.
pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    state: web::Data<AppState>,
    query: web::Query<WsQuery>,
) -> Result<HttpResponse, Error> {
    // Validate JWT
    let claims = auth::validate_token(&query.token, &state.config.jwt_secret)
        .map_err(|e| actix_web::error::ErrorUnauthorized(e.to_string()))?;

    let (response, session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    let user_id = claims.sub.clone();
    let username = claims.username.clone();

    // Channel for server -> client messages
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

    // Clone state for the spawned task
    let state = state.into_inner();

    // Spawn the WebSocket session handler
    actix_rt::spawn(async move {
        log::info!("WebSocket connected: {} ({})", username, user_id);

        // Sender task: forwards ServerMessages to the WebSocket in strictly ordered FIFO
        let mut send_session = session.clone();
        let sender_handle = actix_rt::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Ok(json) = serde_json::to_string(&msg) {
                    if send_session.text(json).await.is_err() {
                        break;
                    }
                }
            }
        });

        // Receiver loop: reads client messages from the WebSocket
        while let Some(Ok(msg)) = msg_stream.next().await {
            match msg {
                Message::Text(text) => {
                    let text_str = text.to_string();
                    match serde_json::from_str::<ClientMessage>(&text_str) {
                        Ok(client_msg) => {
                            if let Err(e) = handle_client_message(
                                &state,
                                &user_id,
                                &username,
                                client_msg,
                                &tx,
                            )
                            .await
                            {
                                let _ = tx.send(ServerMessage::Error { message: e });
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(ServerMessage::Error {
                                message: format!("Invalid message: {e}"),
                            });
                        }
                    }
                }
                Message::Ping(bytes) => {
                    let mut s = session.clone();
                    let _ = s.pong(&bytes).await;
                }
                Message::Close(_) => {
                    break;
                }
                _ => {}
            }
        }

        log::info!("WebSocket disconnected: {} ({})", username, user_id);
        sender_handle.abort();

        // Clean up tables the user was seated at and refund their chips to DB
        let lobby = state.lobby.read().await;
        let refunded = lobby.leave_all_tables(&user_id).await;
        for (_table_id, chips) in refunded {
            if chips > 0 {
                if let Ok(Some(user)) = repository::find_user_by_username(&state.db, &username).await {
                    let new_balance = user.chips.saturating_add(chips);
                    let _ = repository::update_chips(&state.db, &username, new_balance).await;
                }
            }
        }
    });

    Ok(response)
}

/// Dispatch a parsed client message to the appropriate handler.
async fn handle_client_message(
    state: &AppState,
    user_id: &str,
    username: &str,
    msg: ClientMessage,
    tx: &mpsc::UnboundedSender<ServerMessage>,
) -> Result<(), String> {
    match msg {
        ClientMessage::ListTables => {
            let lobby = state.lobby.read().await;
            let tables = lobby.list_tables().await;
            let _ = tx.send(ServerMessage::TableList { tables });
        }

        ClientMessage::CreateTable {
            small_blind,
            big_blind,
            ante,
            max_players,
            game_mode,
            starting_chips,
        } => {
            let config = TableConfig {
                small_blind,
                big_blind,
                ante,
                max_players,
            };
            let table_id = {
                let mut lobby = state.lobby.write().await;
                lobby.create_table(config, game_mode, starting_chips)?
            };
            log::info!("Table {} ({:?}) created by {}", table_id, game_mode, username);
            let lobby = state.lobby.read().await;
            let tables = lobby.list_tables().await;
            let _ = tx.send(ServerMessage::TableList { tables });
        }

        ClientMessage::CreateTournament {
            small_blind,
            big_blind,
            ante,
            max_players,
            starting_chips,
        } => {
            let config = TableConfig {
                small_blind,
                big_blind,
                ante,
                max_players,
            };
            let table_id = {
                let mut lobby = state.lobby.write().await;
                lobby.create_table(
                    config,
                    crate::game::messages::GameMode::Tournament,
                    Some(starting_chips),
                )?
            };
            log::info!("Tournament {} created by {}", table_id, username);
            let lobby = state.lobby.read().await;
            let tables = lobby.list_tables().await;
            let _ = tx.send(ServerMessage::TableList { tables });
        }

        ClientMessage::JoinTable {
            table_id,
            seat,
            buy_in,
        } => {
            // Determine required chips and tournament start status
            let (game_mode, is_started, required_chips) = {
                let lobby = state.lobby.read().await;
                let table_lock = lobby
                    .get_table(&table_id)
                    .ok_or_else(|| format!("Table '{}' not found", table_id))?;
                let table = table_lock.read().await;
                let required = if table.game_mode == crate::game::messages::GameMode::Tournament {
                    table.starting_chips
                } else {
                    buy_in
                };
                (table.game_mode, table.is_started, required)
            };

            // In tournament mode, late registration is prohibited once the game has begun
            if game_mode == crate::game::messages::GameMode::Tournament && is_started {
                let is_seated = {
                    let lobby = state.lobby.read().await;
                    if let Some(table_lock) = lobby.get_table(&table_id) {
                        let table = table_lock.read().await;
                        table.find_seat_by_user(user_id) == Some(seat)
                    } else {
                        false
                    }
                };
                if !is_seated {
                    return Err("Cannot join room: tournament has already started".to_string());
                }
            }

            if required_chips == 0 {
                return Err("Buy-in must be greater than 0".to_string());
            }

            // Verify user has sufficient chips in database
            let user = repository::find_user_by_username(&state.db, username)
                .await
                .map_err(|e| format!("Database error: {e}"))?
                .ok_or_else(|| "User not found".to_string())?;

            if user.chips < required_chips {
                return Err(format!(
                    "Insufficient chips: you have {}, required {}",
                    user.chips, required_chips
                ));
            }

            // Deduct required buy-in chips from database
            let new_balance = user.chips - required_chips;
            repository::update_chips(&state.db, username, new_balance)
                .await
                .map_err(|e| format!("Database error: {e}"))?;

            let lobby = state.lobby.read().await;
            let join_res = lobby
                .join_table(
                    &table_id,
                    seat,
                    user_id.to_string(),
                    username.to_string(),
                    buy_in,
                    tx.clone(),
                )
                .await;

            if let Err(e) = join_res {
                // Refund chips if join failed
                let _ = repository::update_chips(&state.db, username, user.chips).await;
                return Err(e);
            }

            // Send confirmation followed by full table state snapshot
            let _ = tx.send(ServerMessage::JoinedTable {
                table_id: table_id.clone(),
                seat,
            });
            let table_state = lobby.get_table_state(&table_id).await?;
            let _ = tx.send(table_state);
        }

        ClientMessage::LeaveTable { table_id } => {
            let lobby = state.lobby.read().await;
            let chips = lobby.leave_table(&table_id, user_id).await?;
            if chips > 0 {
                if let Ok(Some(user)) = repository::find_user_by_username(&state.db, username).await {
                    let new_balance = user.chips.saturating_add(chips);
                    let _ = repository::update_chips(&state.db, username, new_balance).await;
                }
            }
        }

        ClientMessage::StartHand { table_id } => {
            let lobby = state.lobby.read().await;
            lobby.start_hand(&table_id).await?;
        }

        ClientMessage::PlayerAction { table_id, action } => {
            let engine_action: poker_engine::Action = action.into();
            let lobby = state.lobby.read().await;
            lobby.player_action(&table_id, user_id, engine_action).await?;
        }
    }
    Ok(())
}
