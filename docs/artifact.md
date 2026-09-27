# Poker Backend — Implementation Walkthrough

## Summary

Built a complete real-time poker game server backend in Rust that wraps the existing `poker_engine` crate. The server supports WebSocket-based game play, JWT authentication, and MongoDB persistence via Podman Compose.

## Changes Made

### Infrastructure (2 new files)
| File | Purpose |
|------|---------|
| [`docker-compose.yml`](file:///home/zero/Desktop/projects/rust/poker-backend/docker-compose.yml) | Podman Compose config for MongoDB 7 with auth and persistent volume |
| [`.env`](file:///home/zero/Desktop/projects/rust/poker-backend/.env) | Environment variables (MongoDB URI, JWT secret, server bind) |

### Dependencies (1 modified file)
| File | Changes |
|------|---------|
| [`Cargo.toml`](file:///home/zero/Desktop/projects/rust/poker-backend/Cargo.toml) | Removed `websocket 0.27.1`, added `actix-ws`, `mongodb`, `jsonwebtoken`, `bcrypt`, `uuid`, `chrono`, `dotenvy`, `thiserror`, etc. |

### Core Modules (2 new files)
| File | Purpose |
|------|---------|
| [`src/config.rs`](file:///home/zero/Desktop/projects/rust/poker-backend/src/config.rs) | Typed `Config` struct loaded from environment variables |
| [`src/error.rs`](file:///home/zero/Desktop/projects/rust/poker-backend/src/error.rs) | Unified `AppError` enum with actix-web `ResponseError` impl for automatic HTTP status mapping |

### Auth Module (2 new files)
| File | Purpose |
|------|---------|
| [`src/auth/mod.rs`](file:///home/zero/Desktop/projects/rust/poker-backend/src/auth/mod.rs) | JWT `create_token()` / `validate_token()` with 24h expiry |
| [`src/auth/handlers.rs`](file:///home/zero/Desktop/projects/rust/poker-backend/src/auth/handlers.rs) | `POST /api/auth/register` and `POST /api/auth/login` REST endpoints |

### Database Module (3 new files)
| File | Purpose |
|------|---------|
| [`src/db/mod.rs`](file:///home/zero/Desktop/projects/rust/poker-backend/src/db/mod.rs) | MongoDB connection init with ping check and index setup |
| [`src/db/models.rs`](file:///home/zero/Desktop/projects/rust/poker-backend/src/db/models.rs) | `UserDoc` and `HandHistoryDoc` document schemas |
| [`src/db/repository.rs`](file:///home/zero/Desktop/projects/rust/poker-backend/src/db/repository.rs) | CRUD: `create_user`, `find_user_by_username`, `update_chips`, `save_hand_history`, plus index creation |

### Game Module (4 new files)
| File | Purpose |
|------|---------|
| [`src/game/mod.rs`](file:///home/zero/Desktop/projects/rust/poker-backend/src/game/mod.rs) | Module re-exports |
| [`src/game/messages.rs`](file:///home/zero/Desktop/projects/rust/poker-backend/src/game/messages.rs) | `ClientMessage` and `ServerMessage` — tagged JSON WebSocket protocol |
| [`src/game/table_actor.rs`](file:///home/zero/Desktop/projects/rust/poker-backend/src/game/table_actor.rs) | `GameTable` — wraps `poker_engine::Table` with connected player tracking, event broadcasting (private hole cards, targeted YourTurn), and state snapshots |
| [`src/game/lobby.rs`](file:///home/zero/Desktop/projects/rust/poker-backend/src/game/lobby.rs) | `Lobby` — manages table creation/listing, player join/leave with broadcast, and dispatches game actions with turn validation |

### WebSocket Module (2 new files)
| File | Purpose |
|------|---------|
| [`src/ws/mod.rs`](file:///home/zero/Desktop/projects/rust/poker-backend/src/ws/mod.rs) | Module re-exports |
| [`src/ws/handler.rs`](file:///home/zero/Desktop/projects/rust/poker-backend/src/ws/handler.rs) | `GET /ws?token=<JWT>` — WebSocket upgrade, JWT auth, bidirectional message loop dispatching to lobby |

### Entry Point (1 modified file)
| File | Changes |
|------|---------|
| [`src/main.rs`](file:///home/zero/Desktop/projects/rust/poker-backend/src/main.rs) | Complete rewrite: initializes config, MongoDB, lobby, binds HTTP server with REST auth + WebSocket routes |

## What Was Tested

| Test | Result |
|------|--------|
| `cargo build` (poker-backend) | ✅ Compiles with only dead-code warnings for scaffolded functions |
| `cargo test` (poker-engine) | ✅ All 26 tests pass (8 unit + 18 integration) |

## How to Run

```bash
# 1. Start MongoDB
podman-compose up -d

# 2. Run the server
cargo run

# 3. Register a user
curl -X POST http://localhost:8080/api/auth/register \
  -H 'Content-Type: application/json' \
  -d '{"username":"alice","password":"test123"}'

# 4. Connect via WebSocket (using websocat or similar)
websocat "ws://localhost:8080/ws?token=<JWT_FROM_STEP_3>"

# 5. Create and join a table
# Send: {"type":"CreateTable","small_blind":10,"big_blind":20,"ante":0,"max_players":6}
# Send: {"type":"JoinTable","table_id":"<ID>","seat":0,"buy_in":1000}
```
