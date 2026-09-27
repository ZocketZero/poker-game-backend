# WebSocket Server Messages (Server → Client)

These messages are sent from the server to clients over the WebSocket connection.

---

## Summary of Message Types

| Message Type | Scope | Description |
|--------------|-------|-------------|
| [`TableList`](#1-tablelist) | Requesting client | List of active tables and their status |
| [`JoinedTable`](#2-joinedtable) | Requesting client | Confirmation of successful table join |
| [`TableState`](#3-tablestate) | Requesting client | Comprehensive table state snapshot |
| [`PlayerJoined`](#4-playerjoined) | All seated players | Notification that a player joined the table |
| [`PlayerLeft`](#5-playerleft) | All seated players | Notification that a player left the table |
| [`PlayerEliminated`](#6-playereliminated) | All seated players | Notification that a tournament player was eliminated |
| [`TournamentEnded`](#7-tournamentended) | All seated players | Notification that a tournament finished and winner was crowned |
| [`HoleCards`](#8-holecards) | **Direct / Private** | Private hole cards dealt to the owning player |
| [`YourTurn`](#9-yourturn) | **Direct / Private** | Turn notification with legal action parameters |
| [`GameEvent`](#10-gameevent) | All seated players | Public poker engine events (streets, blinds, showdown) |
| [`Error`](#11-error) | Requesting client | Operation failure or invalid message details |

---

## 1. `TableList`

Sent in response to a `ListTables`, `CreateTable`, or `CreateTournament` request.

### Message Payload
```json
{
  "type": "TableList",
  "tables": [
    {
      "id": "8b51d8b7-6cb5-4f46-95fa-d4b533cb18df",
      "name": "Tournament-8b51d8b7",
      "player_count": 3,
      "max_players": 6,
      "small_blind": 10,
      "big_blind": 20,
      "stage": "PreFlop",
      "game_mode": "Tournament",
      "is_started": false,
      "starting_chips": 1000
    },
    {
      "id": "4a12c9e1-1db4-4f32-82ea-b1c422da10ab",
      "name": "Table-4a12c9e1",
      "player_count": 2,
      "max_players": 6,
      "small_blind": 5,
      "big_blind": 10,
      "stage": "HandEnded",
      "game_mode": "Cash",
      "is_started": true,
      "starting_chips": null
    }
  ]
}
```

---

## 2. `JoinedTable`

Sent directly to the client who requested `JoinTable` confirming their seat assignment.

### Message Payload
```json
{
  "type": "JoinedTable",
  "table_id": "8b51d8b7-6cb5-4f46-95fa-d4b533cb18df",
  "seat": 0
}
```

---

## 3. `TableState`

Sent to a joining player immediately after `JoinedTable` to provide a full snapshot of the table.

### Message Payload
```json
{
  "type": "TableState",
  "table_id": "8b51d8b7-6cb5-4f46-95fa-d4b533cb18df",
  "seats": [
    {
      "seat": 0,
      "username": "alice",
      "chips": 1000,
      "status": "Active",
      "current_bet": 10
    },
    {
      "seat": 1,
      "username": "bob",
      "chips": 1000,
      "status": "Active",
      "current_bet": 20
    },
    {
      "seat": 2,
      "username": null,
      "chips": null,
      "status": null,
      "current_bet": null
    }
  ],
  "stage": "PreFlop",
  "board": [],
  "pot": 30,
  "current_player": 0,
  "game_mode": "Tournament",
  "is_started": false
}
```

#### Fields Description
- `seats`: Array of seat slots from `0` to `max_players - 1`. Empty seats have `username: null`.
- `stage`: Current stage of the hand (`PreFlop`, `Flop`, `Turn`, `River`, `Showdown`, or `HandEnded`).
- `board`: Community cards visible on the board (e.g. `["A♠", "K♦", "2♣"]`).
- `pot`: Total chips currently in the pot across all betting rounds.
- `current_player`: Seat index of the player whose turn it is to act (`null` if no active turn).
- `game_mode`: `"Cash"` or `"Tournament"`.
- `is_started`: Whether the game has begun (in tournament mode, prevents new players from joining).

---

## 4. `PlayerJoined`

Broadcast to other seated players when a new player sits down.

### Message Payload
```json
{
  "type": "PlayerJoined",
  "table_id": "8b51d8b7-6cb5-4f46-95fa-d4b533cb18df",
  "seat": 1,
  "username": "bob",
  "chips": 1000
}
```

---

## 5. `PlayerLeft`

Broadcast to remaining players when a seated player leaves or disconnects.

### Message Payload
```json
{
  "type": "PlayerLeft",
  "table_id": "8b51d8b7-6cb5-4f46-95fa-d4b533cb18df",
  "seat": 1,
  "username": "bob"
}
```

---

## 6. `PlayerEliminated`

Broadcast in **Tournament Mode** when a player's chip stack reaches `0` at the end of a hand.

### Message Payload
```json
{
  "type": "PlayerEliminated",
  "table_id": "8b51d8b7-6cb5-4f46-95fa-d4b533cb18df",
  "seat": 1,
  "username": "bob",
  "rank": 2
}
```

- `rank`: Finishing position in the tournament (e.g. `2` for 2nd place runner-up).

---

## 7. `TournamentEnded`

Broadcast in **Tournament Mode** when only one player remains with chips, concluding the tournament.

### Message Payload
```json
{
  "type": "TournamentEnded",
  "table_id": "8b51d8b7-6cb5-4f46-95fa-d4b533cb18df",
  "winner_username": "alice",
  "prize": 2000
}
```

- `prize`: Total accumulated tournament prize pool credited directly to the winner's MongoDB account.

---

## 8. `HoleCards`

**Private Message**: Sent strictly to the owner of the cards when a hand begins. It is never broadcast in public events.

### Message Payload
```json
{
  "type": "HoleCards",
  "table_id": "8b51d8b7-6cb5-4f46-95fa-d4b533cb18df",
  "cards": [
    { "code": 16908587 },
    { "code": 16908843 }
  ]
}
```

---

## 9. `YourTurn`

**Private Message**: Sent strictly to the player who must act, specifying exact legal actions and bet bounds.

### Message Payload
```json
{
  "type": "YourTurn",
  "table_id": "8b51d8b7-6cb5-4f46-95fa-d4b533cb18df",
  "legal_actions": {
    "can_fold": true,
    "can_check": false,
    "can_call": true,
    "call_amount": 10,
    "can_bet": false,
    "min_bet": 0,
    "max_bet": 0,
    "can_raise": true,
    "min_raise": 40,
    "max_raise": 1000,
    "can_all_in": true,
    "all_in_cost": 1000
  }
}
```

---

## 10. `GameEvent`

Broadcast to all seated players at the table whenever an engine event occurs.

### Event Variants

#### Hand Started
```json
{
  "type": "GameEvent",
  "table_id": "8b51d8b7-...",
  "event": {
    "HandStarted": {
      "hand_id": 1,
      "button": 0,
      "small_blind": 10,
      "big_blind": 20
    }
  }
}
```

#### Blind Posted
```json
{
  "type": "GameEvent",
  "table_id": "8b51d8b7-...",
  "event": {
    "BlindPosted": {
      "player_id": 0,
      "amount": 10,
      "is_small_blind": true
    }
  }
}
```

#### Player Acted
```json
{
  "type": "GameEvent",
  "table_id": "8b51d8b7-...",
  "event": {
    "PlayerActed": {
      "player_id": 0,
      "action": { "Raise": 60 },
      "chips_committed": 50
    }
  }
}
```

#### Street Started (Flop / Turn / River)
```json
{
  "type": "GameEvent",
  "table_id": "8b51d8b7-...",
  "event": {
    "StreetStarted": {
      "stage": "Flop",
      "board": [
        { "code": 16908587 },
        { "code": 268566811 },
        { "code": 67174679 }
      ]
    }
  }
}
```

#### Public Player Turn Notification
```json
{
  "type": "GameEvent",
  "table_id": "8b51d8b7-...",
  "event": {
    "PlayerTurn": {
      "player_id": 1
    }
  }
}
```

#### Showdown
```json
{
  "type": "GameEvent",
  "table_id": "8b51d8b7-...",
  "event": {
    "Showdown": {
      "players": [
        [0, [{ "code": 16908587 }, { "code": 16908843 }], { "rank_type": "Pair", "score": 3450 }],
        [1, [{ "code": 268566811 }, { "code": 67174679 }], { "rank_type": "HighCard", "score": 6200 }]
      ]
    }
  }
}
```

#### Pot Awarded
```json
{
  "type": "GameEvent",
  "table_id": "8b51d8b7-...",
  "event": {
    "PotAwarded": {
      "pot_index": 0,
      "player_id": 0,
      "amount": 120,
      "hand_rank": { "rank_type": "Pair", "score": 3450 }
    }
  }
}
```

#### Hand Ended
```json
{
  "type": "GameEvent",
  "table_id": "8b51d8b7-...",
  "event": "HandEnded"
}
```

---

## 11. `Error`

Sent directly to a client when their request cannot be processed.

### Message Payload
```json
{
  "type": "Error",
  "message": "Cannot join room: tournament has already started"
}
```
