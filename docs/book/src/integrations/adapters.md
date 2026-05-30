# Messaging Adapters

Clawdius Gateway connects to messaging platforms through **adapters** -- pluggable
modules that translate between each platform's API and Clawdius's internal
message protocol.

## Available Adapters

| Adapter | Transport | Feature Flag | Status |
|---------|-----------|--------------|--------|
| [Telegram](telegram.md) | Long-polling (teloxide) | `telegram` | Stable |
| [Discord](discord.md) | Gateway (serenity) | `discord` | Stable |
| [Slack](slack.md) | Socket Mode / Event API | `slack` | Stable |
| [Matrix](matrix.md) | Client-Server sync (matrix-sdk) | `matrix` | Stable |
| [Signal](signal.md) | REST API (signal-cli) | -- | Stable |
| [Teams](teams.md) | Bot Framework REST API | -- | Stable |
| [WhatsApp](whatsapp.md) | Meta Cloud API | -- | Stable |
| [Rocket.Chat](rocketchat.md) | REST / Real-Time API | -- | Stable |
| [Webhook](webhook.md) | HTTP POST | -- | Stable |

## Common Configuration Pattern

All adapters share the same `PlatformConfig` structure. Required fields are
resolved from environment variables or the TOML config file:

```toml
[gateways.telegram]
api_token = "..."          # Maps to TELEGRAM_BOT_TOKEN
enabled = true

[gateways.telegram.settings]
# Adapter-specific key-value pairs
```

Each adapter reads `api_token` as its primary credential and uses the
`settings` map for additional configuration.

## Message Flow

```
Platform API --> Adapter --> Gateway (MessageHandler) --> LLM
                                                       |
Platform API <-- Adapter <-- Gateway (OutgoingMessage) <+
```

All adapters implement the `PlatformAdapter` trait which provides:

- `start()` / `stop()` -- lifecycle management
- `send_message()` -- deliver responses
- `edit_message()` -- streaming edits (where supported)
- `download_attachment()` -- file retrieval

## Health Checking

Every adapter exposes a `health()` method returning:

| Field | Type | Description |
|-------|------|-------------|
| `healthy` | `bool` | Whether the adapter is active |
| `message` | `string` | Status summary (e.g. "polling", "syncing") |
| `messages_processed` | `u64` | Total messages handled |
| `errors` | `u64` | Total errors encountered |

## Feature Gate Caveat

Telegram, Discord, Slack, and Matrix adapters are behind Cargo feature flags.
Enable them at build time:

```bash
cargo build --features telegram,discord,slack,matrix
```

Signal, Teams, WhatsApp, Rocket.Chat, and Webhook adapters are always available.
