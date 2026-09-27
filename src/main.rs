use std::sync::Arc;

use actix_cors::Cors;
use actix_web::{App, HttpResponse, HttpServer, http, web};
use tokio::sync::RwLock;

use poker_backend::config::Config;
use poker_backend::db;
use poker_backend::game::lobby::Lobby;
use poker_backend::{AppState, auth, ws};

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

    let lobby = Arc::new(RwLock::new(Lobby::new(Some(database.clone()))));

    let bind_addr = format!("{}:{}", config.server_host, config.server_port);

    let app_state = AppState {
        config,
        db: database,
        lobby,
    };

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("http://localhost:3000")
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
            .allowed_headers(vec![
                http::header::AUTHORIZATION,
                http::header::ACCEPT,
                http::header::CONTENT_TYPE,
            ])
            .supports_credentials() // Omit if using send_wildcard()
            .max_age(3600);

        App::new()
            .wrap(cors)
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
