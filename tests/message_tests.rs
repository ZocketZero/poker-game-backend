use poker_backend::game::messages::{ActionPayload, ClientMessage, GameMode, ServerMessage, TableInfo};
use poker_engine::Action;

#[test]
fn test_client_message_deserialization() {
    let json_list = r#"{"type":"ListTables"}"#;
    let msg: ClientMessage = serde_json::from_str(json_list).unwrap();
    assert!(matches!(msg, ClientMessage::ListTables));

    let json_create = r#"{"type":"CreateTable","small_blind":10,"big_blind":20,"ante":5,"max_players":6,"game_mode":"tournament","starting_chips":1500}"#;
    let msg: ClientMessage = serde_json::from_str(json_create).unwrap();
    match msg {
        ClientMessage::CreateTable {
            small_blind,
            big_blind,
            ante,
            max_players,
            game_mode,
            starting_chips,
        } => {
            assert_eq!(small_blind, 10);
            assert_eq!(big_blind, 20);
            assert_eq!(ante, 5);
            assert_eq!(max_players, 6);
            assert_eq!(game_mode, GameMode::Tournament);
            assert_eq!(starting_chips, Some(1500));
        }
        _ => panic!("Expected CreateTable"),
    }

    let json_create_tourney = r#"{"type":"CreateTournament","small_blind":10,"big_blind":20,"ante":0,"max_players":6,"starting_chips":2000}"#;
    let msg: ClientMessage = serde_json::from_str(json_create_tourney).unwrap();
    match msg {
        ClientMessage::CreateTournament {
            small_blind,
            big_blind,
            ante,
            max_players,
            starting_chips,
        } => {
            assert_eq!(small_blind, 10);
            assert_eq!(big_blind, 20);
            assert_eq!(ante, 0);
            assert_eq!(max_players, 6);
            assert_eq!(starting_chips, 2000);
        }
        _ => panic!("Expected CreateTournament"),
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
            game_mode: GameMode::Tournament,
            is_started: false,
            starting_chips: Some(1000),
        }],
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""type":"TableList""#));
    assert!(json.contains(r#""player_count":2"#));
    assert!(json.contains(r#""game_mode":"Tournament""#));

    let msg_elim = ServerMessage::PlayerEliminated {
        table_id: "t1".to_string(),
        seat: 2,
        username: "dave".to_string(),
        rank: 3,
    };
    let json_elim = serde_json::to_string(&msg_elim).unwrap();
    assert!(json_elim.contains(r#""type":"PlayerEliminated""#));
    assert!(json_elim.contains(r#""rank":3"#));

    let msg_end = ServerMessage::TournamentEnded {
        table_id: "t1".to_string(),
        winner_username: "alice".to_string(),
        prize: 3000,
    };
    let json_end = serde_json::to_string(&msg_end).unwrap();
    assert!(json_end.contains(r#""type":"TournamentEnded""#));
    assert!(json_end.contains(r#""prize":3000"#));
}
