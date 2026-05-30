# Webhook Adapter

## Overview

Provides a generic HTTP endpoint that accepts incoming messages via POST
requests and delivers responses via configured outgoing webhooks. Useful
for custom integrations, CI/CD notifications, ChatOps pipelines, and any
platform without a dedicated adapter.

## Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `WEBHOOK_URL` | Yes | -- | URL to POST outgoing messages to |
| `WEBHOOK_SECRET` | No | -- | Secret for HMAC-SHA256 signature verification |
| `WEBHOOK_LISTEN_PORT` | No | `8080` | Port for incoming webhook listener |
| `WEBHOOK_OUTGOING_HEADERS` | No | `{}` | Custom headers for outgoing requests (JSON object) |

The outgoing URL is read from `webhook_url`. The secret is read from
`webhook_secret`. The listen port is read from `settings.listen_port`.
Custom headers are read from `settings.outgoing_headers`.

## Setup

1. Configure the outgoing webhook URL where responses will be delivered:
   ```toml
   [gateways.webhook]
   webhook_url = "https://your-app.example.com/webhook"
   webhook_secret = "shared-secret-for-signing"
   enabled = true

   [gateways.webhook.settings]
   listen_port = 8080
   ```

2. (Optional) Configure a custom listen port:
   ```toml
   [gateways.webhook.settings]
   listen_port = 9090
   ```

3. (Optional) Add custom headers to outgoing requests:
   ```toml
   [gateways.webhook.settings]
   outgoing_headers = { "X-API-Key" = "your-api-key", "X-Source" = "clawdius" }
   ```

## Protocol

### Incoming Messages (POST to Clawdius)

```json
{
  "chat_id": "optional-channel-id",
  "user": {
    "id": "user-123",
    "name": "Alice",
    "username": "alice",
    "is_admin": false
  },
  "text": "message content",
  "reply_to": "optional-parent-id",
  "message_id": "optional-custom-id"
}
```

All fields except `text` are optional. If `chat_id` is omitted, it
defaults to `"default"`. If `user` is omitted, it defaults to
anonymous.

### Outgoing Messages (POST from Clawdius)

```json
{
  "chat_id": "...",
  "text": "response content",
  "reply_to": "optional-parent-id",
  "message_id": "wh_550e8400-e29b-41d4-a716-446655440000",
  "edit_message_id": "optional-original-id"
}
```

The `edit_message_id` field is present only for edit operations.

### HMAC Signatures

When `WEBHOOK_SECRET` is configured, outgoing requests include an
`X-Clawdius-Signature` header with the HMAC-SHA256 digest:
```
X-Clawdius-Signature: sha256=abcdef0123456789...
```

Verify the signature by computing HMAC-SHA256 of the raw request body
using your shared secret.

## Features

- Generic HTTP POST interface for any platform
- HMAC-SHA256 payload signing for outgoing messages
- Custom outgoing headers
- Configurable listen port for incoming webhooks
- Message ID generation (`wh_<uuid>`)
- Edit support via `edit_message_id` field
- Fallback for platforms without a dedicated adapter

## Limitations

- No built-in retry logic for failed outgoing webhooks
- The HTTP listener spawns a background task; the listener handle is not
  currently persisted for clean shutdown
- Edit messages do not carry the original `chat_id` -- the target URL
  must handle routing edits to the correct context
- No support for file uploads in outgoing webhooks

## Troubleshooting

**"WEBHOOK_URL not set"**

Set `webhook_url` under `[gateways.webhook]` to the URL where outgoing
messages should be POSTed.

**"failed to bind to 0.0.0.0:8080"**

The port is already in use. Change the listen port in
`settings.listen_port` or stop the conflicting service.

**Webhook target returns non-2xx**

Check the `chat_id` and ensure the target endpoint accepts the payload
format. Inspect the outgoing body and headers for debugging.

**Signature verification fails on receiver side**

Ensure both sides use the same secret. The signature is computed over
the raw JSON body bytes using HMAC-SHA256.
