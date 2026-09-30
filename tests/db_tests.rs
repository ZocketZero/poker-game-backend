use chrono::Utc;
use mongodb::bson::{from_document, oid::ObjectId, to_document};
use poker_backend::db::models::{HandHistoryDoc, HandPlayer, UserDoc};

#[test]
fn test_user_doc_bson_roundtrip() {
    let doc = UserDoc {
        id: Some(ObjectId::new()),
        username: "charlie".to_string(),
        password_hash: "hashedpassword".to_string(),
        chips: 5000,
        created_at: Utc::now(),
    };

    let bson_doc = to_document(&doc).expect("Serialization to BSON must succeed");
    let deserialized: UserDoc =
        from_document(bson_doc).expect("Deserialization from BSON must succeed");

    assert_eq!(doc.username, deserialized.username);
    assert_eq!(doc.chips, deserialized.chips);
    assert_eq!(doc.id, deserialized.id);
}

#[test]
fn test_hand_history_doc_bson_roundtrip() {
    let doc = HandHistoryDoc {
        id: Some(ObjectId::new()),
        table_id: "table-123".to_string(),
        hand_number: 1,
        players: vec![
            HandPlayer {
                user_id: "u1".to_string(),
                username: "alice".to_string(),
                seat: 0,
                starting_chips: 1000,
                ending_chips: 1050,
            },
            HandPlayer {
                user_id: "u2".to_string(),
                username: "bob".to_string(),
                seat: 1,
                starting_chips: 1000,
                ending_chips: 950,
            },
        ],
        events: vec![serde_json::json!({"test_event": "BlindPosted"})],
        created_at: Utc::now(),
    };

    let bson_doc = to_document(&doc).expect("Serialization to BSON must succeed");
    let deserialized: HandHistoryDoc =
        from_document(bson_doc).expect("Deserialization from BSON must succeed");

    assert_eq!(doc.table_id, deserialized.table_id);
    assert_eq!(doc.hand_number, deserialized.hand_number);
    assert_eq!(doc.players.len(), deserialized.players.len());
    assert_eq!(doc.players[0].username, deserialized.players[0].username);
}
