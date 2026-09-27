use poker_backend::game::lobby::Lobby;
use poker_backend::game::messages::{GameMode, ServerMessage};
use poker_engine::table::TableConfig;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_lobby_table_config_validation() {
    let mut lobby = Lobby::new(None);

    // Invalid max_players (less than 2)
    let res = lobby.create_table(
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

    // Alice should NOT receive PlayerJoined for herself
    assert!(rx1.try_recv().is_err());

    // Bob joins at seat 1
    lobby
        .join_table(&table_id, 1, "u2".to_string(), "Bob".to_string(), 1000, tx2)
        .await
        .unwrap();

    // Alice should receive PlayerJoined for Bob
    let msg = rx1.try_recv().unwrap();
    match msg {
        ServerMessage::PlayerJoined { seat, username, chips, .. } => {
            assert_eq!(seat, 1);
            assert_eq!(username, "Bob");
            assert_eq!(chips, 1000);
        }
        _ => panic!("Expected PlayerJoined"),
    }

    // Bob leaves
    let chips = lobby.leave_table(&table_id, "u2").await.unwrap();
    assert_eq!(chips, 1000);

    // Alice should receive PlayerLeft for Bob
    let msg = rx1.try_recv().unwrap();
    match msg {
        ServerMessage::PlayerLeft { seat, username, .. } => {
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
async fn test_tournament_cannot_join_after_game_has_begun() {
    let mut lobby = Lobby::new(None);
    let table_id = lobby
        .create_table(
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

    // Alice joins before start with equal starting chips (1500)
    let chips_charged_1 = lobby
        .join_table(&table_id, 0, "u1".to_string(), "Alice".to_string(), 500, tx1)
        .await
        .unwrap();
    assert_eq!(chips_charged_1, 1500, "Should charge equal starting chips in tournament");

    // Bob joins before start
    let chips_charged_2 = lobby
        .join_table(&table_id, 1, "u2".to_string(), "Bob".to_string(), 9999, tx2)
        .await
        .unwrap();
    assert_eq!(chips_charged_2, 1500, "Should charge equal starting chips in tournament");

    // Both players have exactly 1500 chips
    {
        let table_lock = lobby.get_table(&table_id).unwrap();
        let table = table_lock.read().await;
        assert_eq!(table.engine.player(0).unwrap().chips, 1500);
        assert_eq!(table.engine.player(1).unwrap().chips, 1500);
        assert_eq!(table.prize_pool, 3000);
        assert!(!table.is_started);
    }

    // Alice starts the tournament
    lobby.start_hand(&table_id).await.unwrap();

    // Tournament is now started!
    {
        let table_lock = lobby.get_table(&table_id).unwrap();
        let table = table_lock.read().await;
        assert!(table.is_started);
    }

    // Charlie tries to join AFTER tournament has begun
    let join_res = lobby
        .join_table(&table_id, 2, "u3".to_string(), "Charlie".to_string(), 1500, tx3)
        .await;

    assert!(join_res.is_err(), "Late joiner must be rejected after game has begun");
    assert!(join_res.unwrap_err().contains("tournament has already started"));
}
