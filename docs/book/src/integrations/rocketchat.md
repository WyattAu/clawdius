# Rocket.Chat Adapter

## Overview

Connects Clawdius to a Rocket.Chat server via its REST API. Handles
incoming messages from channels and direct messages, with support for
message editing and threaded replies.

## Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `ROCKETCHAT_URL` | Yes | -- | Base URL of the Rocket.Chat server |
| `ROCKETCHAT_TOKEN` | Yes | -- | Personal access token or API key |
| `ROCKETCHAT_USER_ID` | Yes | -- | Rocket.Chat user ID for the bot account |

The server URL is read from `settings.server_url`. The auth token is read
from `api_token`. The user ID is read from `settings.rc_user_id`.

## Setup

1. Log in to your Rocket.Chat server as an administrator
2. Navigate to **Administration > Integrations > Personal Access Tokens**
3. Create a new token for the bot user
4. Note the token and the bot user's user ID
5. Alternatively, create a dedicated bot user account and generate
   an API token for it
6. Configure:
   ```toml
   [gateways.rocketchat]
   api_token = "your-auth-token"
   enabled = true

   [gateways.rocketchat.settings]
   server_url = "https://rocketchat.example.com"
   rc_user_id = "bot-user-id"
   ```

## Features

- Text message sending and receiving
- Channel and direct message support
- Message editing for streaming responses (via `chat.update`)
- File attachment download (with auth headers)
- Threaded replies via `tmid` field
- Authentication via `X-Auth-Token` and `X-User-Id` headers

## Limitations

- Message edits require the message ID to be formatted as
  `room_id:msg_id` to resolve the target room and message
- Real-time updates via WebSocket (DDP protocol) are planned but not
  yet implemented; currently relies on REST API polling
- File upload support is planned but not yet implemented

## Troubleshooting

**"ROCKETCHAT_URL not set"**

Add `server_url` to `[gateways.rocketchat.settings]` with the full base
URL of your Rocket.Chat server (e.g., `https://rocketchat.example.com`).

**"ROCKETCHAT_TOKEN not set"**

Set `api_token` under `[gateways.rocketchat]` to your personal access
token.

**"ROCKETCHAT_USER_ID not set"**

Add `rc_user_id` to `[gateways.rocketchat.settings]` with the Rocket.Chat
user ID of the bot account.

**401 Unauthorized on API calls**

Verify that the auth token is valid and matches the user ID. Tokens can
expire; regenerate one from the Personal Access Tokens admin page.

**Bot not receiving messages**

Ensure the bot user has joined the target channels or groups. For direct
messages, the bot must be allowed to receive DMs.
