# Slack Adapter

## Overview

Connects Clawdius to Slack via a Slack App using the Slack Web API. Handles
incoming messages via Socket Mode or the Events API, file downloads, and
response delivery including threaded replies.

## Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `SLACK_BOT_TOKEN` | Yes | -- | Bot OAuth token (starts with `xoxb-`) |
| `SLACK_APP_TOKEN` | No | -- | App-Level token for Socket Mode (starts with `xapp-`) |

The bot token is read from the `api_token` field. The app token is read
from `settings.app_token`.

## Setup

1. Go to the [Slack App settings](https://api.slack.com/apps)
2. Click "Create New App" and choose "From scratch"
3. Under **OAuth & Permissions**, add these Bot Token Scopes:
   - `chat:write`
   - `channels:history`
   - `files:read`
   - `groups:history`
   - `im:history`
   - `mpim:history`
4. Install the app to your workspace and copy the Bot User OAuth Token
   (`xoxb-...`)
5. For Socket Mode (recommended for self-hosted):
   - Go to **Settings > Basic Information > App-Level Tokens**
   - Generate a new token with the `connections:write` scope
   - Copy the App-Level Token (`xapp-...`)
6. Enable the `slack` feature flag:
   ```bash
   cargo build --features slack
   ```
7. Configure:
   ```toml
   [gateways.slack]
   api_token = "xoxb-your-bot-token"
   enabled = true

   [gateways.slack.settings]
   app_token = "xapp-your-app-token"
   ```

## Features

- Message handling (text, threaded replies, bot mentions)
- File and attachment download (with Bearer auth for private files)
- Threaded conversations using Slack's `thread_ts` field
- Markdown formatting (mrkdwn)
- Message editing via `chat.update`

## Limitations

- Block Kit support is planned but not yet implemented
- Socket Mode listener runs separately from the adapter; the adapter
  itself handles send/edit/download via the Web API
- Message edits require the message ID to be formatted as
  `channel_id:timestamp`
- Without Socket Mode, you must configure an Events API request URL
  pointing to the gateway

## Troubleshooting

**"SLACK_BOT_TOKEN not set"**

Ensure the `api_token` field is set under `[gateways.slack]`. The token
must start with `xoxb-`.

**"missing_scope" error from Slack API**

Re-check your Bot Token Scopes under OAuth & Permissions. The most common
missing scope is `chat:write`.

**"channel_not_found" when sending**

The bot has not been added to the channel. Invite it with `/invite @botname`.

**Socket Mode not connecting**

Verify the app token starts with `xapp-` and has the `connections:write`
scope.
