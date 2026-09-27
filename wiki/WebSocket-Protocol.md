# WebSocket Protocol

The WebSocket gateway enables real-time, low-latency, bidirectional communication between poker clients and the game server.

---

## Connection Endpoint

```
ws://127.0.0.1:8080/ws?token=<JWT_TOKEN>
```

Authentication is performed during the HTTP upgrade handshake via the `token` query parameter containing a valid JWT obtained from `/api/auth/register` or `/api/auth/login`.

- If the token is missing, expired, or has an invalid signature, the server rejects the handshake with HTTP status **`401 Unauthorized`**.
- Upon successful validation, the connection is upgraded to WebSocket and a dedicated session is initialized.

---

## Message Framing

All messages exchanged over the WebSocket connection (both incoming and outgoing) are UTF-8 JSON text frames.

Every JSON message contains a `"type"` discriminator string:

```json
{
  "type": "MessageType",
  ...
}
```

---

## Heartbeats (Ping / Pong)

- Clients can send WebSocket `Ping` frames at any interval (recommended every 15–30 seconds) to maintain connection liveness and prevent NAT/proxy timeouts.
- The server responds with standard WebSocket `Pong` frames carrying the identical binary payload.

---

## Connection Lifecycle & State Management

```
┌──────────┐                     ┌──────────┐                     ┌──────────┐
│  Client  │                     │  Server  │                     │ MongoDB  │
└────┬─────┘                     └────┬─────┘                     └────┬─────┘
     │   GET /ws?token=<JWT>          │                                │
     ├───────────────────────────────>│  Verify JWT                    │
     │   101 Switching Protocols      │                                │
     │<───────────────────────────────┤                                │
     │                                │                                │
     │   {"type":"JoinTable",...}     │                                │
     ├───────────────────────────────>│  Deduct buy-in                 │
     │                                ├───────────────────────────────>│
     │   {"type":"JoinedTable",...}   │                                │
     │<───────────────────────────────┤                                │
     │   {"type":"TableState",...}    │                                │
     │<───────────────────────────────┤                                │
     │                                │                                │
     │   (Gameplay via PlayerAction)  │                                │
     │<==============================>│                                │
     │                                │                                │
     │   Connection Closes / Drops    │                                │
     │- - - - - - - - - - - - - - - ->│  Detect disconnect             │
     │                                │  Auto-check/fold if turn       │
     │                                │  Credit remaining chips        │
     │                                ├───────────────────────────────>│
     │                                │  Broadcast PlayerLeft          │
```

### 1. Connection Drop / Disconnect Handling
When a WebSocket client drops or closes connection:
- If the player is seated at a table and **not** involved in an active hand, they are immediately removed from the seat, their remaining table chips are safely credited back to MongoDB, and remaining players receive `PlayerLeft`.
- If the player is in an active hand and it is **their turn**, the server automatically checks (if legal) or folds on their behalf, advances the hand, and refunds their remaining chips to MongoDB.
- If the player is in an active hand but it is **not their turn yet**, their seat remains reserved. When their turn arrives, the server checks if their connection is closed and automatically checks or folds. Once the hand completes (`HandEnded`), their ending chips are refunded to MongoDB and they are cleanly removed from the table.

### 2. Seamless Reconnection
If a player experiences a brief network interruption and reconnects with a new WebSocket:
- Sending `JoinTable` for their existing seat reattaches the new WebSocket sender without charging a second buy-in.
- The player immediately receives `JoinedTable` and `TableState` snapshots, restoring full gameplay visibility.
