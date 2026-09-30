use chrono::Utc;
use mongodb::Database;
use mongodb::bson::doc;

use crate::db::collections::USERS_COLLECTION;
use crate::db::models::{HandHistoryDoc, UserDoc};
use crate::error::AppError;

const HAND_HISTORY_COLLECTION: &str = "hand_history";

/// Create a new user. Returns error if username already exists.
pub async fn create_user(
    db: &Database,
    username: &str,
    password_hash: &str,
    starting_chips: u64,
) -> Result<UserDoc, AppError> {
    let users = db.collection::<UserDoc>(USERS_COLLECTION);

    // Check if username already taken
    if users
        .find_one(doc! { "username": username })
        .await?
        .is_some()
    {
        return Err(AppError::Conflict(format!(
            "Username '{}' is already taken",
            username
        )));
    }

    let user = UserDoc {
        id: None,
        username: username.to_string(),
        password_hash: password_hash.to_string(),
        chips: starting_chips,
        created_at: Utc::now(),
    };

    let result = users.insert_one(&user).await?;

    Ok(UserDoc {
        id: result.inserted_id.as_object_id(),
        ..user
    })
}

/// Find a user by username.
pub async fn find_user_by_username(
    db: &Database,
    username: &str,
) -> Result<Option<UserDoc>, AppError> {
    let users = db.collection::<UserDoc>(USERS_COLLECTION);
    let user = users.find_one(doc! { "username": username }).await?;
    Ok(user)
}

/// Update a user's chip balance.
pub async fn update_chips(db: &Database, username: &str, new_balance: u64) -> Result<(), AppError> {
    let users = db.collection::<UserDoc>(USERS_COLLECTION);
    users
        .update_one(
            doc! { "username": username },
            doc! { "$set": { "chips": new_balance as i64 } },
        )
        .await?;
    Ok(())
}

/// Save a hand history record.
pub async fn save_hand_history(db: &Database, doc: &HandHistoryDoc) -> Result<(), AppError> {
    let collection = db.collection::<HandHistoryDoc>(HAND_HISTORY_COLLECTION);
    collection.insert_one(doc).await?;
    Ok(())
}
