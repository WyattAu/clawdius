# Discord Adapter

## Overview

Connects Clawdius to Discord via a bot using the serenity SDK. Handles
incoming messages, file attachments, and response delivery including
streaming edits through Discord's REST API.

## Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DISCORD_BOT_TOKEN` | Yes | -- | Bot token from the Discord Developer Portal |

The token is read from the `api_token` field in the platform config or
from the `DISCORD_BOT_TOKEN` environment variable.

## Setup

1. Go to the [Discord Developer Portal](https://discord.com/developers/applications)
2. Click "New Application" and give it a name
3. Navigate to the "Bot" tab and click "Add Bot"
4. Copy the bot token
5. Enable the Privileged Gateway Intents you need (Message Content Intent
   is required for reading message text)
6. Invite the bot to your server using the OAuth2 URL generator with the
   `bot` scope and appropriate permissions
7. Enable the `discord` feature flag:
   ```bash
   cargo build --features discord
   ```
8. Set the token in your config:
   ```toml
   [gateways.discord]
   api_token = "MTk2Njg1.xxxxxx.xxxxxx"
   enabled = true
   ```

## Features

- Full message handling (text, replies, embeds)
- File and attachment download from Discord CDN
- Message editing for streaming responses
- Markdown formatting (Discord-flavored)
- Role-based admin detection (checks `MANAGE_MESSAGES` permission)
- Bot messages are automatically ignored

## Limitations

- The adapter uses Discord REST API v10 directly for send/edit operations
  rather than the serenity Client's gateway. The serenity Client is used
  for receiving events via `DiscordEventHandler`.
- Message edits require the message ID to be formatted as
  `channel_id:message_id` to resolve the target channel.
- No slash command support -- commands are parsed from plain text messages.

## Troubleshooting

**"DISCORD_BOT_TOKEN not set"**

Ensure the `api_token` field is set under `[gateways.discord]` or that
the `DISCORD_BOT_TOKEN` environment variable is exported.

**401 Unauthorized on send**

The bot token is invalid. Regenerate it in the Discord Developer Portal
under the Bot tab.

**403 Forbidden on send**

The bot lacks permissions in the target channel. Ensure it has been
invited with the correct permissions (Send Messages, Manage Messages).

**Bot not receiving messages**

Verify that the Message Content Intent is enabled in the Developer Portal
under your bot's Privileged Gateway Intents.
