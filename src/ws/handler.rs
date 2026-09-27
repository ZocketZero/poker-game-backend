use actix_web::{Error, HttpRequest, HttpResponse, web};
use actix_ws::Message;
use futures_util::StreamExt;
use poker_engine::table::TableConfig;
use tokio::sync::mpsc;

use crate::auth;
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

    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    let user_id = claims.sub.clone();
    let username = claims.username.clone();

    // Channel for server -> client messages
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

    // Clone state for the spawned task
    let state = state.into_inner();

    // Spawn the WebSocket session handler
    actix_rt::spawn(async move {
        log::info!("WebSocket connected: {} ({})", username, user_id);

        // Sender task: forward ServerMessages to the WebSocket
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

        // Receiver loop: read client messages from the WebSocket
        while let Some(Ok(msg)) = msg_stream.next().await {
            match msg {
                Message::Text(text) => {
                    let text_str = text.to_string();
                    match serde_json::from_str::<ClientMessage>(&text_str) {
                        Ok(client_msg) => {
                            handle_client_message(
                                &state,
                                &user_id,
                                &username,
                                client_msg,
                                &tx,
                                &mut session,
                            )
                            .await;
                        }
                        Err(e) => {
                            let err_msg = ServerMessage::Error {
                                message: format!("Invalid message: {e}"),
                            };
                            if let Ok(json) = serde_json::to_string(&err_msg) {
                                let _ = session.text(json).await;
                            }
                        }
                    }
                }
                Message::Ping(bytes) => {
                    let _ = session.pong(&bytes).await;
                }
                Message::Close(_) => {
                    break;
                }
                _ => {}
            }
        }

        log::info!("WebSocket disconnected: {} ({})", username, user_id);
        sender_handle.abort();
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
    session: &mut actix_ws::Session,
) {
    let result: Result<Option<ServerMessage>, String> = async {
        match msg {
            ClientMessage::ListTables => {
                let lobby = state.lobby.read().await;
                let tables = lobby.list_tables().await;
                Ok(Some(ServerMessage::TableList { tables }))
            }

            ClientMessage::CreateTable {
                small_blind,
                big_blind,
                ante,
                max_players,
            } => {
                let config = TableConfig {
                    small_blind,
                    big_blind,
                    ante,
                    max_players,
                };
                let mut lobby = state.lobby.write().await;
                let table_id = lobby.create_table(config);
                let tables = lobby.list_tables().await;
                log::info!("Table {} created by {}", table_id, username);
                Ok(Some(ServerMessage::TableList { tables }))
            }

            ClientMessage::JoinTable {
                table_id,
                seat,
                buy_in,
            } => {
                let lobby = state.lobby.read().await;
                lobby
                    .join_table(
                        &table_id,
                        seat,
                        user_id.to_string(),
                        username.to_string(),
                        buy_in,
                        tx.clone(),
                    )
                    .await?;

                // Send the joiner a table state snapshot
                let table_state = lobby.get_table_state(&table_id).await?;
                let _ = tx.send(table_state);

                Ok(Some(ServerMessage::JoinedTable { table_id, seat }))
            }

            ClientMessage::LeaveTable { table_id } => {
                let lobby = state.lobby.read().await;
                lobby.leave_table(&table_id, user_id).await?;
                Ok(None)
            }

            ClientMessage::StartHand { table_id } => {
                let lobby = state.lobby.read().await;
                lobby.start_hand(&table_id).await?;
                Ok(None)
            }

            ClientMessage::PlayerAction { table_id, action } => {
                let engine_action: poker_engine::Action = action.into();
                let lobby = state.lobby.read().await;
                lobby.player_action(&table_id, user_id, engine_action).await?;
                Ok(None)
            }
        }
    }
    .await;

    match result {
        Ok(Some(response)) => {
            if let Ok(json) = serde_json::to_string(&response) {
                let _ = session.text(json).await;
            }
        }
        Ok(None) => {}
        Err(e) => {
            let err_msg = ServerMessage::Error { message: e };
            if let Ok(json) = serde_json::to_string(&err_msg) {
                let _ = session.text(json).await;
            }
        }
    }
}
