use poker_backend::game::lobby::Lobby;
use poker_backend::game::messages::{GameMode, ServerMessage};
use poker_engine::table::TableConfig;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_lobby_table_config_validation() {
    let mut lobby = Lobby::new(None);

    // Invalid max_players (less than 2)
    let res = lobby.create_table(
        "admin".to_string(),
        "Admin".to_string(),
        TableConfig {
            small_blind: 10,
            big_blind: 20,
            ante: 0,
            max_players: 1,
        },
        GameMode::Cash,
        None,
    );
    assert!(res.is_err());

    // Invalid max_players (more than 10)
    let res = lobby.create_table(
        "admin".to_string(),
        "Admin".to_string(),
        TableConfig {
            small_blind: 10,
            big_blind: 20,
            ante: 0,
            max_players: 11,
        },
        GameMode::Cash,
        None,
    );
    assert!(res.is_err());

    // Invalid big blind (0)
    let res = lobby.create_table(
        "admin".to_string(),
        "Admin".to_string(),
        TableConfig {
            small_blind: 10,
            big_blind: 0,
            ante: 0,
            max_players: 6,
        },
        GameMode::Cash,
        None,
    );
    assert!(res.is_err());

    // Invalid: small_blind > big_blind
    let res = lobby.create_table(
        "admin".to_string(),
        "Admin".to_string(),
        TableConfig {
            small_blind: 50,
            big_blind: 20,
            ante: 0,
            max_players: 6,
        },
        GameMode::Cash,
        None,
    );
    assert!(res.is_err());

    // Valid table
    let res = lobby.create_table(
        "admin".to_string(),
        "Admin".to_string(),
        TableConfig {
            small_blind: 10,
            big_blind: 20,
            ante: 0,
            max_players: 6,
        },
        GameMode::Cash,
        None,
    );
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_join_and_leave_table() {
    let mut lobby = Lobby::new(None);
    let table_id = lobby
        .create_table(
            "admin".to_string(),
            "Admin".to_string(),
            TableConfig {
                small_blind: 10,
                big_blind: 20,
                ante: 0,
                max_players: 6,
            },
            GameMode::Cash,
            None,
        )
        .unwrap();

    let (tx1, mut rx1) = mpsc::unbounded_channel();
    let (tx2, _rx2) = mpsc::unbounded_channel();

    // Alice joins at seat 0
    lobby
        .join_table(&table_id, 0, "u1".to_string(), "Alice".to_string(), 1000, tx1)
        .await
        .unwrap();

    // Bob joins at seat 1
    lobby
        .join_table(&table_id, 1, "u2".to_string(), "Bob".to_string(), 1000, tx2)
        .await
        .unwrap();

    // Alice should receive PlayerJoined for Bob
    let msg = rx1.recv().await.unwrap();
    match msg {
        ServerMessage::PlayerJoined { username, seat, .. } => {
            assert_eq!(seat, 1);
            assert_eq!(username, "Bob");
        }
        _ => panic!("Expected PlayerJoined"),
    }

    // Bob leaves
    lobby.leave_table(&table_id, "u2").await.unwrap();

    // Alice should receive PlayerLeft for Bob
    let msg = rx1.recv().await.unwrap();
    match msg {
        ServerMessage::PlayerLeft { username, seat, .. } => {
            assert_eq!(seat, 1);
            assert_eq!(username, "Bob");
        }
        _ => panic!("Expected PlayerLeft"),
    }
}

#[tokio::test]
async fn test_leave_all_tables_on_disconnect() {
    let mut lobby = Lobby::new(None);
    let table_id = lobby
        .create_table(
            "admin".to_string(),
            "Admin".to_string(),
            TableConfig {
                small_blind: 10,
                big_blind: 20,
                ante: 0,
                max_players: 6,
            },
            GameMode::Cash,
            None,
        )
        .unwrap();

    let (tx1, _rx1) = mpsc::unbounded_channel();
    lobby
        .join_table(&table_id, 0, "u1".to_string(), "Alice".to_string(), 1000, tx1)
        .await
        .unwrap();

    let refunded = lobby.leave_all_tables("u1").await;
    assert_eq!(refunded.len(), 1);
    assert_eq!(refunded[0].0, table_id);
    assert_eq!(refunded[0].1, 1000);
}

#[tokio::test]
async fn test_tournament_host_start_and_late_join_restriction() {
    let mut lobby = Lobby::new(None);
    let table_id = lobby
        .create_table(
            "u1".to_string(),
            "Alice".to_string(),
            TableConfig {
                small_blind: 10,
                big_blind: 20,
                ante: 0,
                max_players: 6,
            },
            GameMode::Tournament,
            Some(1500),
        )
        .unwrap();

    let (tx1, _rx1) = mpsc::unbounded_channel();
    let (tx2, _rx2) = mpsc::unbounded_channel();
    let (tx3, _rx3) = mpsc::unbounded_channel();

    // Alice (creator) joins before start with equal starting chips (1500)
    let chips_charged_1 = lobby
        .join_table(&table_id, 0, "u1".to_string(), "Alice".to_string(), 500, tx1)
        .await
        .unwrap();
    assert_eq!(chips_charged_1, 1500);

    // Bob joins before start
    let chips_charged_2 = lobby
        .join_table(&table_id, 1, "u2".to_string(), "Bob".to_string(), 9999, tx2)
        .await
        .unwrap();
    assert_eq!(chips_charged_2, 1500);

    // Both players have exactly 1500 chips
    {
        let table_lock = lobby.get_table(&table_id).unwrap();
        let table = table_lock.read().await;
        assert_eq!(table.engine.player(0).unwrap().chips, 1500);
        assert_eq!(table.engine.player(1).unwrap().chips, 1500);
        assert_eq!(table.prize_pool, 3000);
        assert!(!table.is_started);
    }

    // Even though 2 players joined, tournament must NOT auto-start!
    tokio::time::sleep(std::time::Duration::from_millis(1600)).await;
    {
        let table_lock = lobby.get_table(&table_id).unwrap();
        let table = table_lock.read().await;
        assert!(!table.is_started, "Tournament must not auto start before host starts");
        assert_eq!(table.engine.stage, poker_engine::events::Stage::HandEnded);
    }

    // Bob (not creator) tries to start -> Rejected!
    let bob_start = lobby.start_hand(&table_id, "u2").await;
    assert!(bob_start.is_err());
    assert_eq!(
        bob_start.unwrap_err(),
        "Only the room creator can start the tournament"
    );

    // Alice (creator) starts the tournament -> Success!
    lobby.start_hand(&table_id, "u1").await.unwrap();

    // Tournament is now started!
    {
        let table_lock = lobby.get_table(&table_id).unwrap();
        let table = table_lock.read().await;
        assert!(table.is_started);
        assert_eq!(table.engine.stage, poker_engine::events::Stage::PreFlop);
    }

    // Charlie tries to join AFTER tournament has begun -> Rejected!
    let join_res = lobby
        .join_table(&table_id, 2, "u3".to_string(), "Charlie".to_string(), 1500, tx3)
        .await;

    assert!(join_res.is_err(), "Late joiner must be rejected after game has begun");
    assert!(join_res.unwrap_err().contains("tournament has already started"));
}

#[tokio::test]
async fn test_cash_table_auto_start_on_two_players() {
    let mut lobby = Lobby::new(None);
    let table_id = lobby
        .create_table(
            "admin".to_string(),
            "Admin".to_string(),
            TableConfig {
                small_blind: 10,
                big_blind: 20,
                ante: 0,
                max_players: 6,
            },
            GameMode::Cash,
            None,
        )
        .unwrap();

    let (tx1, _rx1) = mpsc::unbounded_channel();
    let (tx2, _rx2) = mpsc::unbounded_channel();

    // 1 player joins: should not start yet
    lobby
        .join_table(&table_id, 0, "u1".to_string(), "Alice".to_string(), 1000, tx1)
        .await
        .unwrap();

    {
        let table_lock = lobby.get_table(&table_id).unwrap();
        let table = table_lock.read().await;
        assert_eq!(table.engine.stage, poker_engine::events::Stage::HandEnded);
    }

    // 2nd player joins: triggers auto-start
    lobby
        .join_table(&table_id, 1, "u2".to_string(), "Bob".to_string(), 1000, tx2)
        .await
        .unwrap();

    // Wait for the auto-start delay (1.5s for initial start)
    tokio::time::sleep(std::time::Duration::from_millis(1700)).await;

    {
        let table_lock = lobby.get_table(&table_id).unwrap();
        let table = table_lock.read().await;
        assert_eq!(
            table.engine.stage,
            poker_engine::events::Stage::PreFlop,
            "Cash game should auto-start when 2 players join"
        );
        assert_eq!(table.engine.hand_count, 1);
    }
}

#[tokio::test]
async fn test_auto_start_next_hand_after_hand_ended() {
    let mut lobby = Lobby::new(None);
    let table_id = lobby
        .create_table(
            "admin".to_string(),
            "Admin".to_string(),
            TableConfig {
                small_blind: 10,
                big_blind: 20,
                ante: 0,
                max_players: 6,
            },
            GameMode::Cash,
            None,
        )
        .unwrap();

    let (tx1, _rx1) = mpsc::unbounded_channel();
    let (tx2, _rx2) = mpsc::unbounded_channel();

    // Alice joins seat 0, Bob joins seat 1
    lobby
        .join_table(&table_id, 0, "u1".to_string(), "Alice".to_string(), 1000, tx1)
        .await
        .unwrap();
    lobby
        .join_table(&table_id, 1, "u2".to_string(), "Bob".to_string(), 1000, tx2)
        .await
        .unwrap();

    // Wait for Hand 1 to auto-start
    tokio::time::sleep(std::time::Duration::from_millis(1700)).await;

    // Find current acting player and fold to end hand 1
    let acting_user = {
        let table_lock = lobby.get_table(&table_id).unwrap();
        let table = table_lock.read().await;
        assert_eq!(table.engine.hand_count, 1);
        let curr_seat = table.engine.current_player.unwrap();
        if curr_seat == 0 { "u1" } else { "u2" }
    };

    lobby
        .player_action(&table_id, acting_user, poker_engine::Action::Fold)
        .await
        .unwrap();

    // Hand 1 has ended
    {
        let table_lock = lobby.get_table(&table_id).unwrap();
        let table = table_lock.read().await;
        assert_eq!(table.engine.stage, poker_engine::events::Stage::HandEnded);
    }

    // Wait for the 3.0s between-hand delay to elapse
    tokio::time::sleep(std::time::Duration::from_millis(3200)).await;

    // Hand 2 should now have auto-started!
    {
        let table_lock = lobby.get_table(&table_id).unwrap();
        let table = table_lock.read().await;
        assert_eq!(table.engine.hand_count, 2, "Hand 2 should have auto-started");
        assert_eq!(table.engine.stage, poker_engine::events::Stage::PreFlop);
    }
}
