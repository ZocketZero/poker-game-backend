pub mod models;
pub mod repository;

use mongodb::{Client, Database};

use crate::config::Config;

/// Initialize MongoDB client and return the database handle.
pub async fn connect(config: &Config) -> Result<Database, mongodb::error::Error> {
    let client = Client::with_uri_str(&config.mongodb_uri).await?;

    // Ping to verify connectivity
    client
        .database("admin")
        .run_command(mongodb::bson::doc! { "ping": 1 })
        .await?;

    log::info!("Connected to MongoDB successfully");

    let db = client.database(&config.database_name);

    // Ensure indexes
    repository::ensure_indexes(&db).await?;

    Ok(db)
}
