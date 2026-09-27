use poker_backend::game::messages::{ActionPayload, ClientMessage, ServerMessage, TableInfo};
use poker_engine::Action;

#[test]
fn test_client_message_deserialization() {
    let json_list = r#"{"type":"ListTables"}"#;
    let msg: ClientMessage = serde_json::from_str(json_list).unwrap();
    assert!(matches!(msg, ClientMessage::ListTables));

    let json_create = r#"{"type":"CreateTable","small_blind":10,"big_blind":20,"ante":5,"max_players":6}"#;
    let msg: ClientMessage = serde_json::from_str(json_create).unwrap();
    match msg {
        ClientMessage::CreateTable {
            small_blind,
            big_blind,
            ante,
            max_players,
        } => {
            assert_eq!(small_blind, 10);
            assert_eq!(big_blind, 20);
            assert_eq!(ante, 5);
            assert_eq!(max_players, 6);
        }
        _ => panic!("Expected CreateTable"),
    }

    let json_join = r#"{"type":"JoinTable","table_id":"tbl-1","seat":2,"buy_in":1000}"#;
    let msg: ClientMessage = serde_json::from_str(json_join).unwrap();
    match msg {
        ClientMessage::JoinTable {
            table_id,
            seat,
            buy_in,
        } => {
            assert_eq!(table_id, "tbl-1");
            assert_eq!(seat, 2);
            assert_eq!(buy_in, 1000);
        }
        _ => panic!("Expected JoinTable"),
    }

    let json_leave = r#"{"type":"LeaveTable","table_id":"tbl-1"}"#;
    let msg: ClientMessage = serde_json::from_str(json_leave).unwrap();
    match msg {
        ClientMessage::LeaveTable { table_id } => assert_eq!(table_id, "tbl-1"),
        _ => panic!("Expected LeaveTable"),
    }

    let json_action = r#"{"type":"PlayerAction","table_id":"tbl-1","action":{"action":"Raise","amount":100}}"#;
    let msg: ClientMessage = serde_json::from_str(json_action).unwrap();
    match msg {
        ClientMessage::PlayerAction { table_id, action } => {
            assert_eq!(table_id, "tbl-1");
            assert!(matches!(action, ActionPayload::Raise { amount: 100 }));
        }
        _ => panic!("Expected PlayerAction"),
    }
}

#[test]
fn test_action_payload_conversion() {
    assert_eq!(Action::from(ActionPayload::Fold), Action::Fold);
    assert_eq!(Action::from(ActionPayload::Check), Action::Check);
    assert_eq!(Action::from(ActionPayload::Call), Action::Call);
    assert_eq!(Action::from(ActionPayload::Bet { amount: 50 }), Action::Bet(50));
    assert_eq!(Action::from(ActionPayload::Raise { amount: 100 }), Action::Raise(100));
    assert_eq!(Action::from(ActionPayload::AllIn), Action::AllIn);
}

#[test]
fn test_server_message_serialization() {
    let msg = ServerMessage::JoinedTable {
        table_id: "t1".to_string(),
        seat: 0,
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""type":"JoinedTable""#));
    assert!(json.contains(r#""seat":0"#));

    let msg = ServerMessage::TableList {
        tables: vec![TableInfo {
            id: "t1".to_string(),
            name: "Table-1".to_string(),
            player_count: 2,
            max_players: 6,
            small_blind: 10,
            big_blind: 20,
            stage: "PreFlop".to_string(),
        }],
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""type":"TableList""#));
    assert!(json.contains(r#""player_count":2"#));
}
