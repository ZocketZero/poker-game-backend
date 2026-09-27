use std::sync::Arc;

use mongodb::Database;
use tokio::sync::RwLock;

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod game;
pub mod ws;

use crate::config::Config;
use crate::game::lobby::Lobby;

/// Shared application state available to all request handlers.
#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: Database,
    pub lobby: Arc<RwLock<Lobby>>,
}
