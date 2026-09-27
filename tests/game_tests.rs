use poker_backend::game::messages::ServerMessage;
use poker_backend::game::table_actor::GameTable;
use poker_engine::table::TableConfig;
use poker_engine::Action;
use tokio::sync::mpsc;

fn test_config() -> TableConfig {
    TableConfig {
        small_blind: 10,
        big_blind: 20,
        ante: 0,
        max_players: 6,
    }
}

#[tokio::test]
async fn test_sit_and_remove_player() {
    let mut table = GameTable::new("tbl-1".to_string(), "Table 1".to_string(), test_config(), None);
    let (tx1, _rx1) = mpsc::unbounded_channel();
    let (tx2, _rx2) = mpsc::unbounded_channel();

    // Sit player 1 at seat 0
    assert!(table.sit_player(0, "u1".to_string(), "Alice".to_string(), 1000, tx1).is_ok());
    assert_eq!(table.player_count(), 1);

    // Cannot sit at out-of-bounds seat
    let (tx_bad, _rx_bad) = mpsc::unbounded_channel();
    assert!(table.sit_player(10, "u_bad".to_string(), "Bad".to_string(), 1000, tx_bad).is_err());

    // Cannot sit at occupied seat
    assert!(table.sit_player(0, "u2".to_string(), "Bob".to_string(), 1000, tx2.clone()).is_err());

    // Sit player 2 at seat 1
    assert!(table.sit_player(1, "u2".to_string(), "Bob".to_string(), 1000, tx2).is_ok());
    assert_eq!(table.player_count(), 2);

    // Remove player 0
    let (removed, chips) = table.remove_player(0);
    assert!(removed.is_some());
    assert_eq!(chips, 1000);
    assert_eq!(table.player_count(), 1);
}

#[tokio::test]
async fn test_reconnect_reattaches_sender() {
    let mut table = GameTable::new("tbl-1".to_string(), "Table 1".to_string(), test_config(), None);
    let (tx1, rx1) = mpsc::unbounded_channel();

    table.sit_player(0, "u1".to_string(), "Alice".to_string(), 1000, tx1).unwrap();

    // Drop rx1 so tx1 becomes closed (simulating disconnect)
    drop(rx1);

    // New connection for same user at same seat
    let (tx1_new, _rx1_new) = mpsc::unbounded_channel();
    let res = table.sit_player(0, "u1".to_string(), "Alice".to_string(), 1000, tx1_new);
    assert!(res.is_ok(), "Reconnecting same user at their seat should succeed");
}

#[tokio::test]
async fn test_no_duplicate_events_on_subsequent_actions() {
    let mut table = GameTable::new("tbl-1".to_string(), "Table 1".to_string(), test_config(), None);
    let (tx0, mut rx0) = mpsc::unbounded_channel();
    let (tx1, mut rx1) = mpsc::unbounded_channel();

    table.sit_player(0, "u0".to_string(), "Alice".to_string(), 1000, tx0).unwrap();
    table.sit_player(1, "u1".to_string(), "Bob".to_string(), 1000, tx1).unwrap();

    // Start hand
    table.start_hand().unwrap();

    // Count messages received on start_hand
    let mut start_msgs_0 = Vec::new();
    while let Ok(msg) = rx0.try_recv() {
        start_msgs_0.push(msg);
    }
    assert!(!start_msgs_0.is_empty());

    // Drain rx1
    while rx1.try_recv().is_ok() {}

    // Verify HoleCards was sent to Alice and not duplicated
    let hole_cards_count = start_msgs_0
        .iter()
        .filter(|m| matches!(m, ServerMessage::HoleCards { .. }))
        .count();
    assert_eq!(hole_cards_count, 1, "Alice should receive HoleCards exactly once");

    // Alice makes an action
    // In heads-up, button is small blind and acts first preflop (seat 0)
    let action = Action::Call;
    assert!(table.apply_action(action).is_ok());

    // Messages received by Alice after this action
    let mut action_msgs_0 = Vec::new();
    while let Ok(msg) = rx0.try_recv() {
        action_msgs_0.push(msg);
    }

    // CRITICAL BUG TEST: Ensure events are NOT re-broadcast from the start of the hand!
    let re_sent_hole_cards = action_msgs_0
        .iter()
        .filter(|m| matches!(m, ServerMessage::HoleCards { .. }))
        .count();
    assert_eq!(
        re_sent_hole_cards, 0,
        "Subsequent action must NOT re-broadcast HoleCards from hand start"
    );

    let re_sent_hand_started = action_msgs_0
        .iter()
        .filter(|m| {
            if let ServerMessage::GameEvent { event, .. } = m {
                event.get("HandStarted").is_some()
            } else {
                false
            }
        })
        .count();
    assert_eq!(
        re_sent_hand_started, 0,
        "Subsequent action must NOT re-broadcast HandStarted event"
    );
}

#[tokio::test]
async fn test_hole_cards_are_private() {
    let mut table = GameTable::new("tbl-1".to_string(), "Table 1".to_string(), test_config(), None);
    let (tx0, mut rx0) = mpsc::unbounded_channel();
    let (tx1, mut rx1) = mpsc::unbounded_channel();

    table.sit_player(0, "u0".to_string(), "Alice".to_string(), 1000, tx0).unwrap();
    table.sit_player(1, "u1".to_string(), "Bob".to_string(), 1000, tx1).unwrap();

    table.start_hand().unwrap();

    let mut alice_got_hole = false;
    while let Ok(msg) = rx0.try_recv() {
        if let ServerMessage::HoleCards { .. } = msg {
            alice_got_hole = true;
        }
        // Ensure no GameEvent broadcast has HoleCardsDealt
        if let ServerMessage::GameEvent { event, .. } = msg {
            assert!(event.get("HoleCardsDealt").is_none());
        }
    }
    assert!(alice_got_hole);

    let mut bob_got_hole = false;
    while let Ok(msg) = rx1.try_recv() {
        if let ServerMessage::HoleCards { .. } = msg {
            bob_got_hole = true;
        }
        if let ServerMessage::GameEvent { event, .. } = msg {
            assert!(event.get("HoleCardsDealt").is_none());
        }
    }
    assert!(bob_got_hole);
}

#[tokio::test]
async fn test_auto_fold_disconnected_player() {
    let mut table = GameTable::new("tbl-1".to_string(), "Table 1".to_string(), test_config(), None);
    let (tx0, rx0) = mpsc::unbounded_channel();
    let (tx1, _rx1) = mpsc::unbounded_channel();

    table.sit_player(0, "u0".to_string(), "Alice".to_string(), 1000, tx0).unwrap();
    table.sit_player(1, "u1".to_string(), "Bob".to_string(), 1000, tx1).unwrap();

    // Alice disconnects before hand starts
    drop(rx0);

    // Hand starts: Alice is small blind / button in heads up and first to act preflop.
    // Facing big blind, Alice cannot check, so auto-action should fold her!
    table.start_hand().unwrap();

    // Hand should end because Alice auto-folded as a disconnected player!
    assert_eq!(
        table.engine.stage,
        poker_engine::events::Stage::HandEnded,
        "Alice should have auto-folded, ending the hand"
    );

    // Bob should have won the pot
    let bob = table.engine.player(1).unwrap();
    assert!(bob.chips > 1000, "Bob should have won the pot");
}
