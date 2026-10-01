# AI Agent Integration & Client Guide

This document is optimized for **AI Models and Autonomous Agents** interacting with the Poker Backend service.

---

## 1. System Overview for Agents

The server operates a hybrid architecture:
1. **HTTP REST**: Authentication & identity issuance (`/api/auth/*`).
2. **WebSocket**: State synchronization, turn prompts, and action execution (`/ws?token=<JWT>`).

### Base URLs
- REST Base: `http://127.0.0.1:8080`
- WebSocket Base: `ws://127.0.0.1:8080/ws`

---

## 2. Agent Execution Pipeline

An AI poker agent follows this standard lifecycle:

```
[1. POST /api/auth/login or register]
               │
               ▼
[2. Extract JWT token & initial chips]
               │
               ▼
[3. Connect WebSocket ws://...?token=<JWT>]
               │
               ▼
[4. Query Tables: {"type": "ListTables"}]
               │
               ▼
[5. Join or Create Room: {"type": "JoinTable", ...}]
               │
               ▼
┌──────────────┴──────────────────────────────┐
│  State Machine & Event Ingestion Loop       │
│                                             │
│  - On HoleCards: record private 2 cards     │
│  - On TableState: parse board & seats       │
│  - On GameEvent: update pot & active turns  │
│  - On YourTurn: decide & emit PlayerAction  │
└─────────────────────────────────────────────┘
```

---

## 3. Decision-Making & Legal Action Invariants

When the server sends `YourTurn`, the payload provides exact boolean flags and boundaries in `legal_actions`:

```json
{
  "type": "YourTurn",
  "table_id": "uuid-string",
  "legal_actions": {
    "can_fold": true,
    "can_check": false,
    "can_call": true,
    "call_amount": 20,
    "can_bet": false,
    "min_bet": 0,
    "max_bet": 0,
    "can_raise": true,
    "min_raise": 60,
    "max_raise": 1000,
    "can_all_in": true,
    "all_in_cost": 1000
  },
  "time_limit_secs": 15
}
```

> **Decision Time Limit**: Players are allotted `time_limit_secs` (default 15 seconds) to submit an action. If no valid action is received before this window expires, the server automatically performs a **Check** (if legal) or **Fold**.

### Action Formulation Rules for LLMs

To guarantee zero rejected actions:

1. **Fold**:
   - Only if `legal_actions.can_fold == true`.
   - Payload: `{"type": "PlayerAction", "table_id": "...", "action": {"action": "Fold"}}`
2. **Check**:
   - Only if `legal_actions.can_check == true`. Never send Check if `can_check == false`!
   - Payload: `{"type": "PlayerAction", "table_id": "...", "action": {"action": "Check"}}`
3. **Call**:
   - Only if `legal_actions.can_call == true`.
   - Payload: `{"type": "PlayerAction", "table_id": "...", "action": {"action": "Call"}}`
4. **Bet**:
   - Only if `legal_actions.can_bet == true`.
   - Value constraint: `legal_actions.min_bet <= amount <= legal_actions.max_bet`.
   - Payload: `{"type": "PlayerAction", "table_id": "...", "action": {"action": "Bet", "amount": <amount>}}`
5. **Raise**:
   - Only if `legal_actions.can_raise == true`.
   - Value constraint: `legal_actions.min_raise <= amount <= legal_actions.max_raise`.
   - *Note: `amount` is the cumulative total bet level (not the incremental difference).*
   - Payload: `{"type": "PlayerAction", "table_id": "...", "action": {"action": "Raise", "amount": <amount>}}`
6. **AllIn**:
   - Only if `legal_actions.can_all_in == true`.
   - Payload: `{"type": "PlayerAction", "table_id": "...", "action": {"action": "AllIn"}}`

---

## 4. Tournament Mode Rules for AI Agents

When interacting with tournament tables (`game_mode: "Tournament"`):
1. **Pre-Game Registration**: Join the table while `is_started: false`.
2. **Equal Starting Chips**: All participants receive identical starting stacks (`starting_chips`), deducted automatically from MongoDB wallet.
3. **Late Registration Closure**: Once `is_started: true`, any `JoinTable` requests from unseated players are rejected.
4. **Elimination**: If an agent's chip count reaches 0, the server broadcasts `PlayerEliminated` and removes the agent.
5. **Winner Payout**: The last surviving agent wins the entire accumulated prize pool via `TournamentEnded`.

---

## 5. Python Reference Agent Implementation

```python
import asyncio
import json
import websockets
import httpx

API_BASE = "http://127.0.0.1:8080"
WS_BASE = "ws://127.0.0.1:8080/ws"

async def run_poker_agent(username: str, password: str):
    # 1. Authenticate / Register
    async with httpx.AsyncClient() as client:
        resp = await client.post(f"{API_BASE}/api/auth/login", json={"username": username, "password": password})
        if resp.status_code != 200:
            resp = await client.post(f"{API_BASE}/api/auth/register", json={"username": username, "password": password})
        auth_data = resp.json()
        token = auth_data["token"]

    # 2. Connect WebSocket
    async with websockets.connect(f"{WS_BASE}?token={token}") as ws:
        # List tables
        await ws.send(json.dumps({"type": "ListTables"}))

        hole_cards = []
        table_id = None

        async for raw_msg in ws:
            msg = json.loads(raw_msg)
            msg_type = msg.get("type")

            if msg_type == "TableList":
                tables = msg.get("tables", [])
                if tables:
                    table_id = tables[0]["id"]
                    # Join seat 0 if open
                    await ws.send(json.dumps({
                        "type": "JoinTable",
                        "table_id": table_id,
                        "seat": 0,
                        "buy_in": 1000
                    }))

            elif msg_type == "JoinedTable":
                table_id = msg["table_id"]
                # Start hand if ready
                await ws.send(json.dumps({"type": "StartHand", "table_id": table_id}))

            elif msg_type == "HoleCards":
                hole_cards = msg["cards"]
                print(f"[{username}] Received Hole Cards: {hole_cards}")

            elif msg_type == "YourTurn":
                legal = msg["legal_actions"]
                # Default AI policy: Check if possible, else Call, else Fold
                if legal.get("can_check"):
                    action = {"action": "Check"}
                elif legal.get("can_call"):
                    action = {"action": "Call"}
                else:
                    action = {"action": "Fold"}

                await ws.send(json.dumps({
                    "type": "PlayerAction",
                    "table_id": table_id,
                    "action": action
                }))

            elif msg_type == "PlayerEliminated":
                print(f"Player eliminated: {msg}")

            elif msg_type == "TournamentEnded":
                print(f"Tournament concluded! Winner: {msg['winner_username']}, Prize: {msg['prize']}")

if __name__ == "__main__":
    asyncio.run(run_poker_agent("ai_bot_1", "securePass123"))
```
