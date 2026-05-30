# Matrix Adapter

## Overview

Connects Clawdius to a Matrix homeserver via a bot account using the
matrix-sdk. Handles incoming messages, file uploads/downloads, and
response delivery including edits via `m.replace` relations.

## Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `MATRIX_HOMESERVER_URL` | Yes | -- | Full URL of the Matrix homeserver |
| `MATRIX_ACCESS_TOKEN` | Yes | -- | Access token for the bot account |
| `MATRIX_USER_ID` | No | `@clawdius:matrix.org` | Full Matrix ID of the bot |

The homeserver URL is read from `settings.homeserver_url`. The access
token is read from `api_token`. The user ID is read from
`settings.user_id`.

## Setup

1. Create a bot account on your Matrix homeserver (e.g., register at
   `matrix.org` or use your own Synapse/Conduit server)
2. Obtain an access token by logging in:
   ```bash
   curl -X POST https://matrix.org/_matrix/client/v3/login \
     -d '{"type":"m.login.password","identifier":{"type":"m.id.user","user":"clawdius"},"password":"your-password"}'
   ```
3. Note the `access_token` from the response
4. Enable the `matrix` feature flag:
   ```bash
   cargo build --features matrix
   ```
5. Configure:
   ```toml
   [gateways.matrix]
   api_token = "syt_abc123_xxx"
   enabled = true

   [gateways.matrix.settings]
   homeserver_url = "https://matrix.org"
   user_id = "@clawdius:matrix.org"
   ```

## Features

- Message handling (text, replies, edits, formatted body)
- File upload and download (with `mxc://` URI resolution)
- Threaded replies via `m.in_reply_to` relations
- Message editing via `m.replace` relations
- Markdown to Matrix HTML formatting
- Idempotent sends using transaction IDs (`clawdius_<uuid>`)
- E2EE support when the `e2e-encryption` feature is enabled

## Limitations

- Message edits require the message ID to be formatted as
  `room_id:event_id` to resolve the target room
- E2EE requires a matching `rusqlite` version for the crypto store;
  enable with `--features matrix,e2e-encryption`
- The Matrix sync loop runs separately via the matrix-sdk Client; the
  adapter handles send/edit/download via the Client-Server API
- `mxc://` media URLs are converted to HTTP download URLs using the
  configured homeserver

## Troubleshooting

**"MATRIX_HOMESERVER_URL not set"**

Add `homeserver_url` to `[gateways.matrix.settings]`.

**"MATRIX_ACCESS_TOKEN not set"**

Add the `api_token` field under `[gateways.matrix]`.

**401 Unauthorized on API calls**

The access token has expired. Log in again to obtain a fresh token.
Consider using a long-lived token for bot accounts.

**Media download fails for `mxc://` URIs**

Verify the homeserver URL is correct and the homeserver's media endpoint
is accessible. The adapter constructs:
`{homeserver_url}/_matrix/media/v3/download/{server_name}/{media_id}`.
