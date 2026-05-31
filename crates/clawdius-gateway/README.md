# clawdius-gateway

Multi-platform AI messaging gateway for the Clawdius agent engine. Routes incoming
chat messages from Telegram, Discord, Slack, Matrix, Signal, Teams, WhatsApp,
Rocket.Chat, and generic webhooks to a shared LLM backend, then formats and delivers
responses back to the originating platform.

## Features

- 9 platform adapters with a unified `PlatformAdapter` trait
- Admin REST API with 13 endpoints for multi-tenant management
- Per-user, per-platform sliding-window rate limiting
- Automatic response chunking that respects platform message length limits
- Streaming support via `OutgoingMessage::chunk` and `edit_message`
- Multi-tenant billing, quota enforcement, and subscription management
- User allowlists and admin role support per platform
- Code-block-aware message splitting (chunks never break a fenced block)
- LLM response caching for identical request deduplication

## Platform Adapters

| Platform     | Feature Flag  | SDK / Transport                         | Status        |
|--------------|---------------|-----------------------------------------|---------------|
| Telegram     | `telegram`    | teloxide 0.14                           | Feature-gated  |
| Discord      | `discord`     | serenity 0.12 (rustls)                 | Feature-gated  |
| Slack        | `slack`       | slack-morphism 1                       | Feature-gated  |
| Matrix       | `matrix`      | matrix-sdk 0.10 (rustls-tls)           | Feature-gated  |
| Signal       | *(default)*   | signal-cli REST API                    | Always built   |
| Teams        | *(default)*   | Bot Framework REST API                 | Always built   |
| WhatsApp     | *(default)*   | Meta Cloud API                         | Always built   |
| Rocket.Chat  | *(default)*   | REST API                                | Always built   |
| Webhook      | *(default)*   | Generic HTTP (axum listener + POST)    | Always built   |

## Quick Start

Add `clawdius-gateway` to your `Cargo.toml` and enable the platforms you need:

```toml
[dependencies]
clawdius-gateway = { version = "1.0.0-rc.2", features = ["telegram", "discord"] }
```

Minimal usage:

```rust
use clawdius_gateway::{MessageGateway, ClawdiusHandler};
use clawdius_gateway::adapter::{Platform, PlatformConfig};

#[tokio::main]
async fn main() {
    let mut gateway = MessageGateway::new();

    let config = PlatformConfig::with_token(Platform::Telegram, "bot-token");
    // gateway.register_adapter(TelegramAdapter::new("bot-token"), config).await;

    gateway.set_handler(Box::new(ClawdiusHandler::new())).await;

    let gateway = std::sync::Arc::new(gateway);
    let results = gateway.start_all_arc().await;
    for (platform, result) in results {
        println!("{platform}: {result:?}");
    }
}
```

## API Overview

### MessageGateway

The central routing layer. Receives messages from adapters, enforces rate limits
and authorization, dispatches to the message handler, then formats and delivers
the response.

```rust
use clawdius_gateway::MessageGateway;
use clawdius_gateway::adapter::{Platform, PlatformConfig, IncomingMessage};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // Create with default rate limiter (20 req/min)
    let mut gateway = MessageGateway::new();

    // Or with a custom rate limiter
    let mut gateway = MessageGateway::with_rate_limiter(100, 60);

    // Register an adapter
    gateway.register_adapter(my_adapter, PlatformConfig::new(Platform::Discord)).await;

    // Set the message handler (connects to the LLM agent)
    gateway.set_handler(Box::new(my_handler)).await;

    // Process an incoming message
    let message = IncomingMessage { /* ... */ };
    gateway.handle_incoming(message).await?;

    // Proactive outbound message
    gateway.send_to_platform(Platform::Discord, "chat-id", "Alert!").await?;

    // Lifecycle
    let gw = Arc::new(gateway);
    gw.start_all_arc().await;
    gw.health_status().await;
    gw.stop_all().await;
}
```

### PlatformAdapter Trait

Every platform implements this trait. Key methods:

| Method               | Description                                           |
|----------------------|-------------------------------------------------------|
| `platform()`         | Returns the `Platform` enum variant                   |
| `set_message_callback(cb)` | Registers the callback for incoming messages    |
| `start()`            | Starts the adapter event loop                         |
| `stop()`             | Stops the adapter event loop                          |
| `send_message(msg)`  | Sends an `OutgoingMessage` to the platform           |
| `edit_message(id, text)` | Edits a previously sent message (for streaming) |
| `download_attachment(url)` | Downloads a file attachment to a temp path     |
| `is_running()`       | Whether the adapter is currently active               |
| `health()`           | Returns an `AdapterHealth` struct                     |

### IncomingMessage / OutgoingMessage

```rust
use clawdius_gateway::adapter::{IncomingMessage, OutgoingMessage, User, Attachment, Platform};
use std::collections::HashMap;

// Incoming
let incoming = IncomingMessage {
    id: "msg-001".into(),
    platform: Platform::Discord,
    chat_id: "ch-123".into(),
    user: User {
        id: "user-42".into(),
        name: "Alice".into(),
        username: Some("alice".into()),
        is_admin: false,
    },
    text: "Hello, Clawdius!".into(),
    reply_to: None,
    attachments: vec![],
    timestamp: chrono::Utc::now(),
    metadata: HashMap::new(),
};

// Outgoing (complete message)
let msg = OutgoingMessage::new(Platform::Discord, "ch-123", "Reply text")
    .with_reply_to("msg-001");

// Outgoing (streamed chunk)
let chunk = OutgoingMessage::chunk(Platform::Discord, "ch-123", "partial", 0);
```

## Admin API

The gateway ships a REST API (axum) for multi-tenant management. All endpoints are
prefixed with `/api/admin`.

| #  | Method | Endpoint                                            | Description              |
|----|--------|-----------------------------------------------------|--------------------------|
| 1  | POST   | `/api/admin/tenants`                                | Create a tenant          |
| 2  | GET    | `/api/admin/tenants`                                | List tenants (filtered)  |
| 3  | GET    | `/api/admin/tenants/{tenant_id}`                    | Get tenant details       |
| 4  | DELETE | `/api/admin/tenants/{tenant_id}`                    | Delete a tenant          |
| 5  | GET    | `/api/admin/tenants/{tenant_id}/usage`              | Get current usage        |
| 6  | POST   | `/api/admin/tenants/{tenant_id}/usage/reset`        | Reset usage counters     |
| 7  | GET    | `/api/admin/tenants/{tenant_id}/quota`              | Get quota limits         |
| 8  | PUT    | `/api/admin/tenants/{tenant_id}/quota`              | Set quota limits         |
| 9  | GET    | `/api/admin/tenants/{tenant_id}/subscription`       | Get subscription info    |
| 10 | PUT    | `/api/admin/tenants/{tenant_id}/subscription/plan`  | Change plan tier         |
| 11 | POST   | `/api/admin/tenants/{tenant_id}/subscription/cancel`| Cancel subscription      |
| 12 | GET    | `/api/admin/system/info`                            | System info & version    |
| 13 | GET    | `/api/admin/health`                                 | Health check             |

Additionally, `/api/gateway/health` returns per-adapter health status.

Authenticate with the `CLAWDIUS_ADMIN_API_KEY` environment variable (defaults to
`clawdius-admin`).

## Feature Flags

| Feature          | Enables          | Adds dependency             |
|------------------|------------------|-----------------------------|
| `telegram`       | Telegram adapter | teloxide 0.14               |
| `discord`        | Discord adapter  | serenity 0.12               |
| `slack`          | Slack adapter    | slack-morphism 1            |
| `matrix`         | Matrix adapter   | matrix-sdk 0.10             |
| `all-platforms`  | All four above   | All four dependencies        |

Signal, Teams, WhatsApp, Rocket.Chat, and Webhook adapters are always available and
require no feature flags.

## Configuration

### PlatformConfig

Each adapter is configured via `PlatformConfig`:

```rust
use clawdius_gateway::adapter::PlatformConfig;
use clawdius_gateway::adapter::Platform;

let mut config = PlatformConfig::with_token(Platform::Telegram, "bot-token");
config.enabled = true;
config.allowed_users = vec!["user-1".into(), "user-2".into()];
config.admin_users = vec!["admin-1".into()];
config.webhook_url = Some("https://example.com/hook".into());
config.webhook_secret = Some("secret123".into());
```

| Field           | Type                            | Default      | Description                        |
|-----------------|---------------------------------|--------------|------------------------------------|
| `platform`      | `Platform`                      | *(required)* | Platform enum variant               |
| `enabled`       | `bool`                          | `true`       | Whether the adapter is active       |
| `api_token`     | `Option<String>`                | `None`       | Bot / API token                     |
| `webhook_url`   | `Option<String>`                | `None`       | Outgoing webhook URL               |
| `webhook_secret`| `Option<String>`                | `None`       | HMAC signature verification secret |
| `allowed_users` | `Vec<String>`                   | `[]`         | Empty = allow all users             |
| `admin_users`   | `Vec<String>`                   | `[]`         | Admin user IDs                      |
| `settings`      | `HashMap<String, Value>`        | `{}`         | Platform-specific key-value pairs  |

### Environment Variables

| Variable                    | Description                    | Used By           |
|-----------------------------|--------------------------------|-------------------|
| `CLAWDIUS_CONFIG`           | Path to config file            | `ClawdiusHandler` |
| `CLAWDIUS_PROVIDER`         | LLM provider override           | `ClawdiusHandler` |
| `CLAWDIUS_MODEL`            | LLM model override             | `ClawdiusHandler` |
| `CLAWDIUS_ADMIN_API_KEY`    | Admin API authentication key   | Admin server      |
| `TELEGRAM_BOT_TOKEN`        | Telegram bot token             | Telegram adapter  |
| `DISCORD_BOT_TOKEN`         | Discord bot token              | Discord adapter   |
| `SLACK_BOT_TOKEN`           | Slack bot token                | Slack adapter     |
| `SLACK_APP_TOKEN`           | Slack app token (Socket Mode)  | Slack adapter     |
| `MATRIX_HOMESERVER`         | Matrix homeserver URL          | Matrix adapter    |
| `MATRIX_ACCESS_TOKEN`       | Matrix access token            | Matrix adapter    |
| `MATRIX_USER_ID`            | Matrix user ID                 | Matrix adapter    |
| `SIGNAL_ACCOUNT_NUMBER`     | Signal account number          | Signal adapter    |
| `SIGNAL_REST_URL`           | signal-cli REST URL            | Signal adapter    |
| `TEAMS_APP_ID`              | Teams App ID                   | Teams adapter     |
| `TEAMS_APP_PASSWORD`        | Teams App Password             | Teams adapter     |
| `TEAMS_SERVICE_URL`         | Teams Service URL              | Teams adapter     |
| `WHATSAPP_ACCESS_TOKEN`     | WhatsApp access token          | WhatsApp adapter  |
| `WHATSAPP_PHONE_NUMBER_ID`  | WhatsApp phone number ID       | WhatsApp adapter  |
| `ROCKETCHAT_URL`            | Rocket.Chat server URL         | RocketChat adapter|
| `ROCKETCHAT_TOKEN`          | Rocket.Chat auth token         | RocketChat adapter|
| `ROCKETCHAT_USER_ID`        | Rocket.Chat user ID            | RocketChat adapter|
| `WEBHOOK_URL`               | Outgoing webhook URL           | Webhook adapter   |
| `WEBHOOK_SECRET`            | Webhook HMAC secret            | Webhook adapter   |
| `RUST_LOG`                  | Log level (e.g. `info`, `debug`) | Logging          |

## Testing

Run the full test suite:

```bash
cargo test -p clawdius-gateway
```

Run with specific platform features enabled:

```bash
cargo test -p clawdius-gateway --features telegram,discord
```

Run only integration tests (outside the binary):

```bash
cargo test -p clawdius-gateway --tests
```

Run property-based tests (uses `proptest`):

```bash
cargo test -p clawdius-gateway proptest
```

Run the binary with a specific platform:

```bash
cargo run -p clawdius-gateway --features telegram \
    -- --platform telegram --telegram-bot-token "YOUR_TOKEN" --rate-limit 30
```
