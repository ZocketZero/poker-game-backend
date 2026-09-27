# 🃏 Poker Backend Wiki

Welcome to the **Poker Backend** documentation wiki!

This wiki provides comprehensive documentation for integrating with the Poker Backend server, covering REST API endpoints, real-time WebSocket protocol messages, game state transitions, chip persistence, and MongoDB storage schemas.

---

## 📚 Table of Contents

1. [**REST API Reference**](REST-API)  
   User registration, authentication, JWT tokens, and health checks.
2. [**WebSocket Protocol**](WebSocket-Protocol)  
   Connection establishment, handshake, ping/pong heartbeats, reconnection, and disconnect handling.
3. [**Client Messages (Client → Server)**](WebSocket-Client-Messages)  
   Complete specifications for messages sent from WebSocket clients (`ListTables`, `CreateTable`, `JoinTable`, `LeaveTable`, `StartHand`, `PlayerAction`).
4. [**Server Messages (Server → Client)**](WebSocket-Server-Messages)  
   Complete specifications for messages sent from the server (`TableList`, `JoinedTable`, `TableState`, `PlayerJoined`, `PlayerLeft`, `HoleCards`, `YourTurn`, `GameEvent`, `Error`).
5. [**Game Flow & Rules**](Game-Flow-and-Rules)  
   No-Limit Texas Hold'em lifecycle, dealer button movement, side pots, disconnect auto-fold, and end-to-end example session.
6. [**Database & Persistence**](Database-and-Persistence)  
   MongoDB schemas (`users`, `hand_history`), token claims structure, and chip conservation lifecycle.

---

## 🚀 Quick Overview

The server combines HTTP REST endpoints for account setup and a bidirectional WebSocket connection for low-latency, real-time poker gameplay:

```
                  ┌──────────────┐
                  │    Client    │
                  └──────┬───────┘
                         │
             ┌───────────┴───────────┐
             │   actix-web Server    │
             ├───────────┬───────────┤
             │ REST API  │ WebSocket │
             │  /api/*   │   /ws     │
             └─────┬─────┴─────┬─────┘
                   │           │
             ┌─────┴─────┐ ┌───┴────┐
             │  MongoDB  │ │ Lobby  │
             │  (users,  │ │  ├─Table 1
             │  history) │ │  └─Table 2
             └───────────┘ └────────┘
```

### Server Default Endpoints

| Service | Endpoint | Protocol | Auth Required |
|---------|----------|----------|---------------|
| Health Check | `GET /health` | HTTP | No |
| Register Account | `POST /api/auth/register` | HTTP | No |
| Login | `POST /api/auth/login` | HTTP | No |
| Game Gateway | `GET /ws?token=<JWT>` | WebSocket | Yes (JWT query param) |

---

## 🛠️ Local Development & Quick Start

### 1. Start MongoDB
```bash
podman-compose up -d
# or: docker compose up -d
```

### 2. Run the Server
```bash
cargo run
```

### 3. Register a Player
```bash
curl -X POST http://127.0.0.1:8080/api/auth/register \
  -H 'Content-Type: application/json' \
  -d '{"username": "alice", "password": "securePass123"}'
```

### 4. Connect via WebSocket
```bash
websocat "ws://127.0.0.1:8080/ws?token=<JWT_TOKEN_FROM_STEP_3>"
```
