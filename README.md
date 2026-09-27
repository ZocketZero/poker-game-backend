# 🃏 Poker Backend

A real-time No-Limit Texas Hold'em poker game server built in Rust, powered by the [`poker_engine`](https://github.com/ZocketZero/poker-engine) crate.

## Features

- **Real-time WebSocket** — Bidirectional communication for live poker gameplay
- **JWT Authentication** — Stateless token-based auth with bcrypt password hashing
- **MongoDB Persistence** — User accounts, chip balances, and hand history storage
- **Lobby System** — Create, list, and join tables dynamically
- **Full NLHE Rules** — Blinds, antes, side pots, all-in, showdown via `poker_engine`
- **Private Card Delivery** — Hole cards are sent only to the owning player
- **Podman Compose** — One-command MongoDB setup

## Architecture

```
                  ┌─────────────┐
                  │   Clients   │
                  │ (WebSocket) │
                  └──────┬──────┘
                         │
              ┌──────────┴──────────┐
              │   actix-web Server  │
              ├─────────┬───────────┤
              │ REST API│ WebSocket │
              │ /api/*  │ /ws      │
              └────┬────┴─────┬────┘
                   │          │
              ┌────┴────┐ ┌───┴────┐
              │ MongoDB │ │ Lobby  │
              │ (users, │ │  ├─Table 1 (poker_engine::Table)
              │ history)│ │  ├─Table 2
              └─────────┘ │  └─Table N
                          └────────┘
```

## Project Structure

```
poker-backend/
├── docker-compose.yml           # Podman Compose — MongoDB 7
├── .env                         # Environment configuration
├── Cargo.toml                   # Dependencies
├── docs/
│   └── artifact.md              # Implementation walkthrough
├── wiki/                        # GitHub Wiki API Documentation
│   ├── Home.md                  # Wiki landing page & index
│   ├── _Sidebar.md              # Wiki navigation sidebar
│   ├── _Footer.md               # Wiki footer
│   ├── REST-API.md              # REST endpoints reference
│   ├── WebSocket-Protocol.md    # WebSocket gateway & lifecycle
│   ├── WebSocket-Client-Messages.md # Client-to-server messages
│   ├── WebSocket-Server-Messages.md # Server-to-client messages
│   ├── Game-Flow-and-Rules.md   # NLHE engine rules & game flow
│   └── Database-and-Persistence.md # MongoDB schema & chip lifecycle
├── tests/                       # Automated integration tests
└── src/
    ├── main.rs                  # Entry point, HTTP server setup
    ├── config.rs                # Environment config loader
    ├── error.rs                 # Unified error types
    ├── auth/
    │   ├── mod.rs               # JWT create/validate utilities
    │   └── handlers.rs          # Register & Login REST endpoints
    ├── db/
    │   ├── mod.rs               # MongoDB connection init
    │   ├── models.rs            # Document schemas (User, HandHistory)
    │   └── repository.rs        # Data access layer (CRUD operations)
    ├── game/
    │   ├── mod.rs               # Game module exports
    │   ├── lobby.rs             # Table registry & player routing
    │   ├── table_actor.rs       # Async wrapper around poker_engine::Table
    │   └── messages.rs          # Client/Server WebSocket message protocol
    └── ws/
        ├── mod.rs               # WebSocket module exports
        └── handler.rs           # WebSocket upgrade & session management
```

## Prerequisites

- **Rust** 1.85+ (edition 2024)
- **Podman** & **podman-compose** (for MongoDB)
- **poker_engine** crate cloned at `../poker-engine`

## Quick Start

### 1. Start MongoDB

```bash
podman-compose up -d
```

This starts a MongoDB 7 container with:
- **Username:** `poker_admin`
- **Password:** `poker_secret`
- **Database:** `poker_db`
- **Port:** `27017`

### 2. Configure Environment

The default `.env` file works out of the box for local development:

```env
MONGODB_URI=mongodb://poker_admin:poker_secret@localhost:27017
DATABASE_NAME=poker_db
JWT_SECRET=change-me-to-a-secure-random-string
SERVER_HOST=127.0.0.1
SERVER_PORT=8080
RUST_LOG=info
```

> ⚠️ **Change `JWT_SECRET`** to a long random string before deploying to production.

### 3. Build & Run

```bash
cargo build
cargo run
```

You should see:

```
[INFO  poker_backend] Starting poker-backend on 127.0.0.1:8080
[INFO  poker_backend::db] Connected to MongoDB successfully
[INFO  poker_backend::db] Database indexes ensured
```

---

## API Documentation & Wiki

Full GitHub Wiki documentation is maintained in the [`wiki/`](wiki/) directory:
- [**Wiki Home**](wiki/Home.md)
- [**REST API Reference**](wiki/REST-API.md)
- [**WebSocket Protocol & Lifecycle**](wiki/WebSocket-Protocol.md)
- [**Client Messages (Client → Server)**](wiki/WebSocket-Client-Messages.md)
- [**Server Messages (Server → Client)**](wiki/WebSocket-Server-Messages.md)
- [**Game Flow & Rules**](wiki/Game-Flow-and-Rules.md)
- [**Database & Persistence**](wiki/Database-and-Persistence.md)

---

## API Reference

### REST Endpoints

#### `GET /health`

Health check.

```bash
curl http://localhost:8080/health
```

```json
{ "status": "ok", "service": "poker-backend" }
```

---

#### `POST /api/auth/register`

Create a new account. Returns a JWT token and starting chips (10,000).

**Request:**

```bash
curl -X POST http://localhost:8080/api/auth/register \
  -H 'Content-Type: application/json' \
  -d '{"username": "alice", "password": "test123"}'
```

**Response** (`201 Created`):

```json
{
  "token": "eyJhbGciOiJIUzI1NiJ9...",
  "username": "alice",
  "chips": 10000
}
```

---

#### `POST /api/auth/login`

Log in with existing credentials. Returns a JWT token.

**Request:**

```bash
curl -X POST http://localhost:8080/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username": "alice", "password": "test123"}'
```

**Response** (`200 OK`):

```json
{
  "token": "eyJhbGciOiJIUzI1NiJ9...",
  "username": "alice",
  "chips": 10000
}
```

---

### WebSocket Endpoint

#### `GET /ws?token=<JWT>`

Upgrade to WebSocket connection. The JWT token from registration/login must be passed as a query parameter.

```bash
# Using websocat
websocat "ws://localhost:8080/ws?token=YOUR_JWT_TOKEN"
```

All WebSocket messages are **JSON objects** with a `"type"` field for routing.

---

### WebSocket Messages — Client → Server

#### `ListTables`

List all available tables.

```json
{ "type": "ListTables" }
```

---

#### `CreateTable`

Create a new poker table.

```json
{
  "type": "CreateTable",
  "small_blind": 10,
  "big_blind": 20,
  "ante": 0,
  "max_players": 6
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `small_blind` | `u64` | required | Small blind amount |
| `big_blind` | `u64` | required | Big blind amount |
| `ante` | `u64` | `0` | Ante per player per hand |
| `max_players` | `usize` | `6` | Maximum seats (2–10) |

---

#### `JoinTable`

Join a table at a specific seat with a chip buy-in.

```json
{
  "type": "JoinTable",
  "table_id": "a1b2c3d4-...",
  "seat": 0,
  "buy_in": 1000
}
```

| Field | Type | Description |
|-------|------|-------------|
| `table_id` | `String` | UUID of the table |
| `seat` | `usize` | Seat index (0-based) |
| `buy_in` | `u64` | Chips to bring to the table |

---

#### `LeaveTable`

Leave a table.

```json
{
  "type": "LeaveTable",
  "table_id": "a1b2c3d4-..."
}
```

---

#### `StartHand`

Request to start a new hand. Requires at least 2 seated players with chips.

```json
{
  "type": "StartHand",
  "table_id": "a1b2c3d4-..."
}
```

---

#### `PlayerAction`

Submit a game action when it's your turn.

```json
{
  "type": "PlayerAction",
  "table_id": "a1b2c3d4-...",
  "action": { "action": "Raise", "amount": 100 }
}
```

**Available actions:**

| Action | Format | Description |
|--------|--------|-------------|
| Fold | `{ "action": "Fold" }` | Fold your hand |
| Check | `{ "action": "Check" }` | Check (no bet to call) |
| Call | `{ "action": "Call" }` | Call the current bet |
| Bet | `{ "action": "Bet", "amount": 50 }` | Open betting (no existing bet) |
| Raise | `{ "action": "Raise", "amount": 100 }` | Raise to a total of `amount` |
| AllIn | `{ "action": "AllIn" }` | Go all-in with remaining chips |

---

### WebSocket Messages — Server → Client

| Type | Description | Sent To |
|------|-------------|---------|
| `TableList` | List of all tables with metadata | Requester |
| `JoinedTable` | Confirmation of joining a table | Requester |
| `TableState` | Full table snapshot (seats, board, pot, stage) | Requester (on join) |
| `PlayerJoined` | A player joined the table | All at table |
| `PlayerLeft` | A player left the table | All at table |
| `HoleCards` | Your private hole cards | **Only you** |
| `YourTurn` | It's your turn with legal actions | **Only you** |
| `GameEvent` | Poker engine events (blinds, actions, board, showdown) | All at table |
| `Error` | Error message | Requester |

---

## Example Game Session

```
# Terminal 1: Alice
> {"type":"CreateTable","small_blind":10,"big_blind":20,"ante":0,"max_players":6}
< {"type":"TableList","tables":[{"id":"abc-123","name":"Table-abc","player_count":0,...}]}

> {"type":"JoinTable","table_id":"abc-123","seat":0,"buy_in":1000}
< {"type":"JoinedTable","table_id":"abc-123","seat":0}
< {"type":"TableState","table_id":"abc-123","seats":[...],...}

# Terminal 2: Bob joins same table at seat 1
> {"type":"JoinTable","table_id":"abc-123","seat":1,"buy_in":1000}

# Alice starts the hand
> {"type":"StartHand","table_id":"abc-123"}
< {"type":"HoleCards","table_id":"abc-123","cards":["A♠","K♠"]}
< {"type":"GameEvent","table_id":"abc-123","event":{"HandStarted":{...}}}
< {"type":"YourTurn","table_id":"abc-123","legal_actions":{...}}

# Alice raises
> {"type":"PlayerAction","table_id":"abc-123","action":{"action":"Raise","amount":60}}

# Bob calls
> {"type":"PlayerAction","table_id":"abc-123","action":{"action":"Call"}}

# ... flop, turn, river, showdown events are broadcast automatically
```

---

## MongoDB Collections

| Collection | Description |
|------------|-------------|
| `users` | User accounts (username, password hash, chip balance) |
| `hand_history` | Completed hand records with player snapshots and events |

**Inspect data:**

```bash
podman exec -it poker-mongo mongosh -u poker_admin -p poker_secret poker_db
> db.users.find()
> db.hand_history.find()
```

---

## Managing the Database

```bash
# Start MongoDB
podman-compose up -d

# Stop MongoDB (data persisted)
podman-compose down

# Stop and delete all data
podman-compose down -v

# View container logs
podman logs poker-mongo
```

---

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `MONGODB_URI` | `mongodb://localhost:27017` | MongoDB connection string |
| `DATABASE_NAME` | `poker_db` | Database name |
| `JWT_SECRET` | `dev-secret-change-me` | Secret key for JWT signing |
| `SERVER_HOST` | `127.0.0.1` | Server bind address |
| `SERVER_PORT` | `8080` | Server bind port |
| `RUST_LOG` | *(none)* | Log level (`info`, `debug`, `trace`) |

---

## Tech Stack

| Component | Technology |
|-----------|------------|
| Language | Rust 1.85+ (edition 2024) |
| Web Framework | actix-web 4 |
| WebSocket | actix-ws 0.3 |
| Database | MongoDB 7 (via `mongodb` 3 driver) |
| Auth | JWT (`jsonwebtoken` 9) + bcrypt |
| Game Engine | `poker_engine` (local crate) |
| Container | Podman + podman-compose |

## License

MIT
