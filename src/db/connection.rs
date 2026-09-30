use mongodb::{Client, Database, IndexModel, bson::doc, options::IndexOptions};

use crate::{
    config::Config,
    db::{collections::USERS_COLLECTION, models::UserDoc},
};

/// Create required indexes on startup.
pub async fn ensure_indexes(db: &Database) -> Result<(), mongodb::error::Error> {
    let users = db.collection::<UserDoc>(USERS_COLLECTION);

    // Unique index on username
    let index = IndexModel::builder()
        .keys(doc! { "username": 1 })
        .options(IndexOptions::builder().unique(true).build())
        .build();

    users.create_index(index).await?;

    log::info!("Database indexes ensured");
    Ok(())
}

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
    ensure_indexes(&db).await?;

    Ok(db)
}
