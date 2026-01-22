# SSControl Signaling Worker

Cloudflare Worker-based signaling server for SSControl WebRTC connections.

## Setup

### 1. Install dependencies

```bash
npm install
```

### 2. Create KV Namespace

```bash
# Create production KV namespace
wrangler kv:namespace create SESSIONS

# Create preview KV namespace (for development)
wrangler kv:namespace create SESSIONS --preview
```

Update `wrangler.toml` with the returned namespace IDs.

### 3. Development

```bash
npm run dev
```

### 4. Deploy

```bash
npm run deploy
```

## API

### Create Session (Host)

```bash
POST /api/session
Content-Type: application/json

{
  "session_id": "abc123",
  "offer": "<SDP offer>",
  "candidates": [{"candidate": "...", "sdpMid": "0"}],
  "public_key": "<optional public key>",
  "pin_hash": "<optional PIN hash>",
  "ttl": 300
}
```

### Get Session (Client)

```bash
GET /api/session/{session_id}
```

Returns:
```json
{
  "offer": "<SDP offer>",
  "candidates": [...],
  "public_key": "...",
  "expires_at": 1234567890,
  "status": "waiting",
  "answer": null,
  "client_candidates": null
}
```

### Post Answer (Client)

```bash
POST /api/session/{session_id}/answer
Content-Type: application/json

{
  "answer": "<SDP answer>",
  "candidates": [...]
}
```

### Add ICE Candidate

```bash
POST /api/session/{session_id}/ice
Content-Type: application/json

{
  "role": "host" | "client",
  "candidate": {"candidate": "...", "sdpMid": "0"}
}
```

### Delete Session

```bash
DELETE /api/session/{session_id}
```

## Configuration

Environment variables in `wrangler.toml`:

- `MAX_SESSION_TTL`: Maximum session lifetime in seconds (default: 300)
- `MAX_ATTEMPTS`: Maximum GET attempts before session invalidation (default: 3)
