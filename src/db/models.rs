use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

/// User document stored in the "users" collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDoc {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub username: String,
    pub password_hash: String,
    pub chips: u64,
    pub created_at: DateTime<Utc>,
}

/// A player snapshot stored within hand history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandPlayer {
    pub user_id: String,
    pub username: String,
    pub seat: usize,
    pub starting_chips: u64,
    pub ending_chips: u64,
}

/// Hand history document stored in the "hand_history" collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandHistoryDoc {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub table_id: String,
    pub hand_number: u64,
    pub players: Vec<HandPlayer>,
    pub events: Vec<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}
