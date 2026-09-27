use std::sync::Arc;

use actix_web::{App, HttpResponse, HttpServer, web};
use mongodb::Database;
use tokio::sync::RwLock;

mod auth;
mod config;
mod db;
mod error;
mod game;
mod ws;

use crate::config::Config;
use crate::game::lobby::Lobby;

/// Shared application state available to all request handlers.
#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: Database,
    pub lobby: Arc<RwLock<Lobby>>,
}

async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "service": "poker-backend",
    }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    env_logger::init();

    let config = Config::from_env();

    log::info!(
        "Starting poker-backend on {}:{}",
        config.server_host,
        config.server_port
    );

    // Connect to MongoDB
    let database = db::connect(&config)
        .await
        .expect("Failed to connect to MongoDB");

    let lobby = Arc::new(RwLock::new(Lobby::new()));

    let bind_addr = format!("{}:{}", config.server_host, config.server_port);

    let app_state = AppState {
        config,
        db: database,
        lobby,
    };

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            // Health check
            .route("/health", web::get().to(health))
            // Auth REST endpoints
            .service(
                web::scope("/api/auth")
                    .route("/register", web::post().to(auth::handlers::register))
                    .route("/login", web::post().to(auth::handlers::login)),
            )
            // WebSocket endpoint
            .route("/ws", web::get().to(ws::handler::ws_handler))
    })
    .bind(&bind_addr)?
    .run()
    .await
}
