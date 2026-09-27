# WebSocket Client Messages (Client → Server)

These messages are sent from the client to the server over the WebSocket connection.

---

## 1. `ListTables`

Requests a list of all active tables currently running on the server along with their metadata.

### Payload Schema
```json
{
  "type": "ListTables"
}
```

### Server Response
The server responds with a [`TableList`](WebSocket-Server-Messages#1-tablelist) message to the requesting client.

---

## 2. `CreateTable`

Creates a new No-Limit Texas Hold'em table with custom configurations.

### Payload Schema

| Field | Type | Required | Constraints | Default | Description |
|-------|------|----------|-------------|---------|-------------|
| `small_blind` | `u64` | **Yes** | `> 0`, `small_blind <= big_blind` | — | Small blind chip amount |
| `big_blind` | `u64` | **Yes** | `> 0` | — | Big blind chip amount |
| `ante` | `u64` | No | `≥ 0` | `0` | Optional ante collected from each player per hand |
| `max_players` | `usize` | No | `2..=10` | `6` | Maximum seats on the table |

### Payload Example
```json
{
  "type": "CreateTable",
  "small_blind": 10,
  "big_blind": 20,
  "ante": 0,
  "max_players": 6
}
```

### Server Response
Upon success, the server responds with an updated [`TableList`](WebSocket-Server-Messages#1-tablelist) message containing the newly created table.

---

## 3. `JoinTable`

Joins an existing table at a specific seat index with a chip buy-in.

### Payload Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `table_id` | `string` | **Yes** | Table UUID |
| `seat` | `usize` | **Yes** | 0-indexed seat number (`0..max_players`) |
| `buy_in` | `u64` | **Yes** | Amount of chips to bring to the table (`> 0`) |

### Payload Example
```json
{
  "type": "JoinTable",
  "table_id": "8b51d8b7-6cb5-4f46-95fa-d4b533cb18df",
  "seat": 0,
  "buy_in": 1000
}
```

### Rules & Validation
1. `buy_in` must be greater than 0.
2. The user's MongoDB account must have at least `buy_in` chips. If sufficient, `buy_in` is deducted from their account.
3. If the seat is already occupied or the seat number exceeds table bounds, the buy-in is immediately refunded and an [`Error`](WebSocket-Server-Messages#9-error) message is returned.
4. If the player is reconnecting with an existing seat whose previous connection dropped, the connection is reattached.

### Server Response
- To the joiner: [`JoinedTable`](WebSocket-Server-Messages#2-joinedtable) followed immediately by [`TableState`](WebSocket-Server-Messages#3-tablestate).
- To all other seated players at the table: [`PlayerJoined`](WebSocket-Server-Messages#4-playerjoined).

---

## 4. `LeaveTable`

Leaves a table and returns all remaining table chips back to the user's MongoDB balance.

### Payload Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `table_id` | `string` | **Yes** | Table UUID |

### Payload Example
```json
{
  "type": "LeaveTable",
  "table_id": "8b51d8b7-6cb5-4f46-95fa-d4b533cb18df"
}
```

### Rules & Constraints
- If a hand is in progress and the player is actively involved in the hand:
  - If it is currently their turn, the server folds their hand automatically and processes their departure.
  - If it is **not** their turn, the server rejects the request with an error message: `"Cannot leave table while involved in an active hand. Please fold on your turn first."`
- The player's remaining table chips are safely added back to their account in MongoDB.
- All other players at the table receive [`PlayerLeft`](WebSocket-Server-Messages#5-playerleft).

---

## 5. `StartHand`

Initiates a new hand on the specified table.

### Payload Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `table_id` | `string` | **Yes** | Table UUID |

### Payload Example
```json
{
  "type": "StartHand",
  "table_id": "8b51d8b7-6cb5-4f46-95fa-d4b533cb18df"
}
```

### Rules & Constraints
- Requires at least 2 seated players with `chips > 0`.
- The table must currently be in the `HandEnded` stage.

### Server Response
- Each seated player receives their private [`HoleCards`](WebSocket-Server-Messages#6-holecards).
- All seated players receive [`GameEvent`](WebSocket-Server-Messages#8-gameevent) for `HandStarted` and blinds posted.
- The player first to act receives their private [`YourTurn`](WebSocket-Server-Messages#7-yourturn).

---

## 6. `PlayerAction`

Submits a game action when it is the player's turn to act.

### Payload Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `table_id` | `string` | **Yes** | Table UUID |
| `action` | `object` | **Yes** | Action payload object |

### Action Types

#### Fold
Folds the player's hand, relinquishing any claim to the pot.
```json
{
  "type": "PlayerAction",
  "table_id": "8b51d8b7-6cb5-4f46-95fa-d4b533cb18df",
  "action": {
    "action": "Fold"
  }
}
```

#### Check
Passes the action to the next player when there is no current bet to call.
```json
{
  "type": "PlayerAction",
  "table_id": "8b51d8b7-6cb5-4f46-95fa-d4b533cb18df",
  "action": {
    "action": "Check"
  }
}
```

#### Call
Matches the current highest bet on the table.
```json
{
  "type": "PlayerAction",
  "table_id": "8b51d8b7-6cb5-4f46-95fa-d4b533cb18df",
  "action": {
    "action": "Call"
  }
}
```

#### Bet
Opens the betting when no previous bet has been made in the round.
```json
{
  "type": "PlayerAction",
  "table_id": "8b51d8b7-6cb5-4f46-95fa-d4b533cb18df",
  "action": {
    "action": "Bet",
    "amount": 50
  }
}
```
*`amount` must be between `min_bet` and `max_bet` as reported in `YourTurn`.*

#### Raise
Raises the bet level when facing an existing bet.
```json
{
  "type": "PlayerAction",
  "table_id": "8b51d8b7-6cb5-4f46-95fa-d4b533cb18df",
  "action": {
    "action": "Raise",
    "amount": 120
  }
}
```
*`amount` represents the **total bet size** the player is raising to (not the delta), and must be between `min_raise` and `max_raise`.*

#### AllIn
Commits all remaining chips to the pot.
```json
{
  "type": "PlayerAction",
  "table_id": "8b51d8b7-6cb5-4f46-95fa-d4b533cb18df",
  "action": {
    "action": "AllIn"
  }
}
```
