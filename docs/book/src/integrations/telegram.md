# Telegram Adapter

## Overview

Connects Clawdius to Telegram via a bot using the teloxide SDK. Handles
incoming messages, file downloads, and response delivery including
streaming edits through Telegram's message editing API.

## Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `TELEGRAM_BOT_TOKEN` | Yes | -- | Bot token from [@BotFather](https://t.me/BotFather) |

The token is read from the `api_token` field in the platform config or
from the `TELEGRAM_BOT_TOKEN` environment variable.

## Setup

1. Open Telegram and search for [@BotFather](https://t.me/BotFather)
2. Send `/newbot` and follow the prompts to create your bot
3. Copy the bot token (format: `123456789:ABCdefGHIjklMNOpqrSTUvwxYZ`)
4. Enable the `telegram` feature flag:
   ```bash
   cargo build --features telegram
   ```
5. Set the token in your config:
   ```toml
   [gateways.telegram]
   api_token = "123456789:ABCdefGHIjklMNOpqrSTUvwxYZ"
   enabled = true
   ```
   Or export the environment variable:
   ```bash
   export TELEGRAM_BOT_TOKEN="123456789:ABCdefGHIjklMNOpqrSTUvwxYZ"
   ```

## Features

- Full message handling (text, commands, replies, forwards)
- File and attachment download
- Message editing for streaming responses
- Markdown v2 formatting in outgoing messages
- Long-polling with 35-second timeout per request
- Rate limit awareness with automatic retry-after handling
- Graceful shutdown via `tokio::sync::Notify`

## Limitations

- Only text messages are processed; stickers, photos without captions,
  and other non-text content are silently dropped
- Long-polling only -- no webhook mode is supported in the current implementation
- Message edits use `edit_message_text` which requires the original
  message to still be accessible by the bot

## Troubleshooting

**"TELEGRAM_BOT_TOKEN not set"**

Ensure the `api_token` field is set under `[gateways.telegram]` or that
the `TELEGRAM_BOT_TOKEN` environment variable is exported.

**"failed to verify bot token"**

The token is invalid or expired. Regenerate it through @BotFather with
`/revoke`.

**Rate limit warnings in logs**

The adapter automatically handles Telegram's `Retry-After` responses by
sleeping for the requested duration. These are informational warnings.

**Network errors**

The adapter retries after 2 seconds on network errors and 5 seconds on
other errors. Check your network connectivity if errors persist.
