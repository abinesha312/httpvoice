# httpvoice

HTTP in, SIP out. A tiny programmable-voice API you host.

**Not Twilio. Not a phone company.** You bring a SIP trunk for real calls.

## What it does

httpvoice is a minimal Twilio-like programmable voice API that you run yourself. It translates HTTP REST calls into SIP signaling. Think of it as the API layer that sits in front of your own SIP infrastructure.

## What it doesn't do

- **No phone numbers** — You manage your own DID inventory through your trunk provider.
- **No PSTN carrier services** — You need a SIP trunk (Twilio, Bandwidth, Telnyx, etc.) for actual phone calls.
- **No STIR/SHAKEN** — That's handled at the trunk/carrier level.
- **No SMS** — Voice only.
- **No global infrastructure** — Single instance you deploy.
- **No robocalling** — This is a simple API tool, not a blast-dial system.

## Quick Start

### Prerequisites

- Rust 2021 edition
- Optional: A SIP trunk provider account for E.164 (phone number) calls

### Installation

```bash
git clone https://github.com/abinesha312/httpvoice
cd httpvoice
cargo build --release
```

### Configuration

Set environment variables:

```bash
# Required: The "from" identity on your SIP trunk (anti-spoofing protection)
export SIP_FROM="sip:your-id@your-trunk.example.com"

# Optional: Your SIP trunk server for E.164 phone calls
export SIP_SERVER="trunk.example.com"

# Optional: Bearer token for API authentication
export HTTVOICE_TOKEN="your-secret-token"
```

### Run

```bash
cargo run --release
```

Server starts on `http://0.0.0.0:8080`

## API Reference

### Create Call

**POST** `/v1/calls`

```bash
curl -X POST http://localhost:8080/v1/calls \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-secret-token" \
  -d '{
    "to": "sip:bob@sip.example.com",
    "from": "sip:your-id@your-trunk.example.com",
    "url": "https://your-server.com/twiml"
  }'
```

**Request:**
```json
{
  "to": "sip:user@host OR +14155551234",
  "from": "sip:your-id@your-trunk.example.com",
  "url": "https://optional-webhook-url.com"
}
```

**Response:** `201 Created`
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "queued"
}
```

**Notes:**
- `to` can be a SIP URI (`sip:user@host`) or E.164 phone number (`+14155551234`)
- `from` **must** match your `SIP_FROM` env variable (anti-spoofing)
- `url` is optional; if provided, receives call status webhooks and can return TwiML

**Status values:** `queued`, `ringing`, `in-progress`, `completed`, `failed`

### Get Call Status

**GET** `/v1/calls/{id}`

```bash
curl http://localhost:8080/v1/calls/550e8400-e29b-41d4-a716-446655440000 \
  -H "Authorization: Bearer your-secret-token"
```

**Response:** `200 OK`
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "to": "sip:bob@sip.example.com",
  "from": "sip:alice@trunk.example.com",
  "status": "in-progress"
}
```

### Hangup Call

**POST** `/v1/calls/{id}/hangup`

```bash
curl -X POST http://localhost:8080/v1/calls/550e8400-e29b-41d4-a716-446655440000/hangup \
  -H "Authorization: Bearer your-secret-token"
```

**Response:** `204 No Content`

## Webhooks (Optional)

If you provide a `url` when creating a call, httpvoice will POST form-encoded call status updates:

```
POST https://your-server.com/webhook
Content-Type: application/x-www-form-urlencoded

CallSid=550e8400-e29b-41d4-a716-446655440000
To=sip:bob@example.com
From=sip:alice@trunk.example.com
CallStatus=ringing
```

## TwiML-lite (Optional Stub)

If your webhook URL returns XML, httpvoice parses basic TwiML:

```xml
<Response>
  <Say>Hello world</Say>
  <Hangup/>
</Response>
```

**Important:** The `<Say>` verb is a **stub**. It logs intent but plays only a tone/silence placeholder (no full TTS). This is documented as a proof-of-concept. If you need real TTS, integrate a separate service or extend this codebase.

## SIP Behavior

### Direct SIP URIs

When `to` is a SIP URI like `sip:user@host.com`, httpvoice sends a direct SIP INVITE to that host on port 5060.

### E.164 Phone Numbers

When `to` is an E.164 number like `+14155551234`:

1. httpvoice sends a SIP REGISTER to the server configured in `SIP_SERVER`
2. Then sends an INVITE through that trunk
3. Your trunk provider routes the call to the PSTN

### Media Handling

The implementation includes basic G.711 PCMU (ulaw) SDP signaling. For production use with live audio, you'd extend this with RTP media handling. Current version is **signaling-focused** with mock media paths — sufficient for testing API flows and SIP connectivity.

## Authentication

If `HTTVOICE_TOKEN` is set, all API requests must include:

```
Authorization: Bearer your-secret-token
```

If `HTTVOICE_TOKEN` is not set, no authentication is required (localhost dev mode).

## Anti-Spoofing

The `from` field in every call request **must** match the `SIP_FROM` environment variable. This prevents callers from spoofing arbitrary caller IDs through your trunk.

```bash
# This will be rejected with 403 Forbidden:
curl -X POST http://localhost:8080/v1/calls \
  -H "Content-Type: application/json" \
  -d '{
    "to": "sip:victim@example.com",
    "from": "sip:fake-identity@evil.com"
  }'
```

## Testing

```bash
# Run all tests
cargo test

# Run with logs
RUST_LOG=debug cargo test -- --nocapture

# Run integration tests serially (avoids env var conflicts)
cargo test -- --test-threads=1
```

Tests cover:
- SIP URI and E.164 phone number parsing
- Anti-spoofing (`from` field validation)
- Call state management and hangup
- TwiML-lite parsing
- Localhost SIP invite
- Authentication with bearer tokens

**No real PSTN calls are made during testing.**

## Deployment Notes

- This is a **single-process API server**. For production, put it behind a reverse proxy (nginx, Caddy) with TLS.
- Use systemd, Docker, or your preferred process manager.
- For distributed setups, you'd need to add shared state (Redis, PostgreSQL) instead of the in-memory call store.
- RTP media handling is stubbed — extend `sip.rs` if you need live audio processing.

## License

MIT

## Contributing

This is a minimal proof-of-concept. Pull requests welcome for:
- Full RTP media support
- More TwiML verbs
- Better SIP trunk compatibility
- Observability (metrics, structured logs)

**Not accepting:** Features that turn this into a carrier/PSTN service or number management system. Those belong at the trunk provider level.

## Legal

This software is for building your own voice API infrastructure. You are responsible for:
- Compliance with telecom regulations in your jurisdiction
- Your SIP trunk provider's terms of service  
- Not using this for spam, harassment, or illegal purposes

The authors are not liable for how you use this software.

