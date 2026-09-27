# REST API Reference

The REST API handles authentication, initial user onboarding, and service health checks.

---

## Base URL

```
http://127.0.0.1:8080
```

All JSON request and response bodies use the `application/json` content type.

---

## Endpoints

### 1. Health Check

Checks if the server instance is operational.

- **Method**: `GET`
- **Path**: `/health`
- **Authentication**: None

#### Request
```http
GET /health HTTP/1.1
Host: 127.0.0.1:8080
```

#### Response (`200 OK`)
```json
{
  "service": "poker-backend",
  "status": "ok"
}
```

---

### 2. User Registration

Registers a new user account with starting chips (default: 10,000).

- **Method**: `POST`
- **Path**: `/api/auth/register`
- **Authentication**: None

#### Request Body Schema

| Field | Type | Required | Constraints | Description |
|-------|------|----------|-------------|-------------|
| `username` | `string` | **Yes** | 1–32 characters, trimmed, unique | Account username |
| `password` | `string` | **Yes** | 4–72 characters | Plaintext password |

#### Request Example
```http
POST /api/auth/register HTTP/1.1
Host: 127.0.0.1:8080
Content-Type: application/json

{
  "username": "alice",
  "password": "secretPassword123"
}
```

#### Successful Response (`201 Created`)
```json
{
  "chips": 10000,
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "username": "alice"
}
```

#### Error Responses

- **`400 Bad Request`**: Validation failure (e.g. empty username, password shorter than 4 or longer than 72 chars):
  ```json
  {
    "error": "Bad request: Password must be at least 4 characters"
  }
  ```
- **`409 Conflict`**: Username is already taken:
  ```json
  {
    "error": "Conflict: Username 'alice' is already taken"
  }
  ```

---

### 3. User Login

Authenticates existing credentials and issues a fresh 24-hour JWT token.

- **Method**: `POST`
- **Path**: `/api/auth/login`
- **Authentication**: None

#### Request Body Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `username` | `string` | **Yes** | Account username |
| `password` | `string` | **Yes** | Plaintext account password |

#### Request Example
```http
POST /api/auth/login HTTP/1.1
Host: 127.0.0.1:8080
Content-Type: application/json

{
  "username": "alice",
  "password": "secretPassword123"
}
```

#### Successful Response (`200 OK`)
```json
{
  "chips": 10000,
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "username": "alice"
}
```

#### Error Responses

- **`401 Unauthorized`**: Invalid username or password:
  ```json
  {
    "error": "Authentication failed: Invalid username or password"
  }
  ```

---

## JWT Authentication Token

JWT tokens issued by `/api/auth/register` and `/api/auth/login` have a 24-hour expiration window.

### Claims Payload
```json
{
  "sub": "65b8f1a23c4d5e6f7a8b9c0d",
  "username": "alice",
  "exp": 1706500000
}
```

- `sub`: Hex string representation of the user's MongoDB `ObjectId`.
- `username`: User account username.
- `exp`: UNIX timestamp (in seconds) marking expiration (issued at + 24 hours).
