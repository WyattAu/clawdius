# Deploying Clawdius Server

## Quick Start

```bash
# Build and run with Docker
docker compose up -d

# Or build locally
cargo build --release --bin clawdius-server
./target/release/clawdius-server
```

## Configuration

The server reads configuration from (in order of precedence):

1. `clawdius.toml` in the current directory
2. `.clawdius/config.toml`
3. CLI flags override file values

### Minimal Config

```toml
[messaging]
host = "0.0.0.0"
port = 8080

[messaging.platforms.telegram]
bot_token = "YOUR_BOT_TOKEN"
```

### CLI Flags

| Flag | Env Var | Default | Description |
|------|---------|---------|-------------|
| `-c, --config` | | | Path to TOML config file |
| `--host` | | `0.0.0.0` | Bind address |
| `-p, --port` | | `8080` | Bind port |
| `--cors-origins` | | | Comma-separated allowed origins |
| `--db-path` | `CLAWDIUS_DB_PATH` | | Session DB path (in-memory if omitted) |
| `--max-request-size` | | `1000000` | Max request body size (bytes) |
| `--mock-channels` | | `false` | Use mock channels (no real platform calls) |
| `--json-logs` | `CLAWDIUS_JSON_LOGS` | `false` | Structured JSON log output |

### Config File Reference

```toml
[messaging]
host = "0.0.0.0"               # Bind address
port = 8080                     # Bind port
cors_origins = ["*"]            # CORS: ["*"] for permissive
rate_limit_per_minute = 60      # Per-platform rate limit
max_request_size_bytes = 1000000 # Max webhook body size
global_api_keys = ["master-key"] # Keys accepted by all platforms
key_default_expiry_secs = 0     # New key expiry (0 = never)
key_grace_period_secs = 0       # Grace period after expiry

# Per-platform API keys
[messaging.api_keys]
telegram = ["tg-key-1", "tg-key-2"]
discord = ["discord-key"]

# Per-platform webhook credentials
[messaging.platforms.telegram]
bot_token = "123456:ABC-DEF..."
secret_token = "webhook_secret"

[messaging.platforms.discord]
public_key_pem = "-----BEGIN PUBLIC KEY-----..."
discord_bot_token = "BOT_TOKEN_HERE"

[messaging.platforms.slack]
signing_secret = "xoxb-..."
slack_bot_token = "xoxb-..."

[messaging.platforms.matrix]
access_token = "syt_..."
homeserver_base_url = "https://matrix.org"

[messaging.platforms.signal]
verification_token = "verify_token"
signal_api_url = "http://localhost:8080"
signal_number = "+1234567890"

[messaging.platforms.whatsapp]
verify_token = "verify_token"
app_secret = "app_secret"
phone_number_id = "12345678"
whatsapp_access_token = "EAAG..."

# IP allowlist (empty = accept all)
[messaging.ip_allowlist]
# ip_allowlist = ["10.0.0.0/8", "192.168.1.0/24"]

# State store
[messaging.state_store]
backend = "sqlite"              # "memory" or "sqlite"
sqlite_path = "messaging_state.db"
encryption_key = ""              # 64-hex-char AES-256 key

# Multi-tenancy
[messaging.tenants]
enabled = false                 # Enable tenant API
default_max_sessions_per_user = 100
db_path = "tenants.db"

# Retry queue
[messaging.retry]
max_retries = 5
initial_delay_ms = 1000
max_delay_ms = 300000
exponential_base = 2.0
jitter_factor = 0.1
max_queue_size = 10000
dead_letter_enabled = true

# Audit logging
[messaging.audit]
backend = "file"                # "file", "sqlite", "memory"
path = "audit"
retention_days = 90
flush_interval_secs = 5

# PII redaction in logs
[messaging.pii_redaction]
redact_field_names = true
redact_value_patterns = true
replacement = "[REDACTED]"
```

## API Endpoints

### Health & Readiness

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/health` | None | Liveness probe (returns uptime) |
| GET | `/ready` | None | Readiness probe (checks state store) |
| GET | `/metrics` | None | Prometheus metrics scrape |

### Tenant Management (requires `tenants.enabled = true`)

All tenant endpoints require `Authorization: Bearer <api-key>` header where the key was added to the server's API key store.

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/tenants` | List all tenants |
| POST | `/api/v1/tenants` | Create a tenant |
| GET | `/api/v1/tenants/{id}` | Get tenant details |
| PUT | `/api/v1/tenants/{id}` | Update a tenant |
| DELETE | `/api/v1/tenants/{id}` | Delete a tenant |
| GET | `/api/v1/tenants/{id}/usage` | Usage summary for tenant |
| GET | `/api/v1/usage` | Global usage overview |

### Webhook Endpoints

| Method | Path | Description |
|--------|------|-------------|
| ANY | `/webhook/{platform}` | Incoming webhook from platform |

Supported platforms: `telegram`, `discord`, `matrix`, `slack`, `signal`, `whatsapp`, `rocketchat`

## Docker

```bash
# Build
docker compose build

# Run (requires clawdius.toml in same directory)
docker compose up -d

# View logs (JSON format in container)
docker compose logs -f clawdius

# Stop
docker compose down
```

Data persists in the `clawdius-data` Docker volume at `/app/data`.

## Encryption at Rest

To enable AES-256-GCM encryption for the state store:

1. Generate a 32-byte key (64 hex characters):
   ```bash
   openssl rand -hex 32
   ```

2. Set in config:
   ```toml
   [messaging.state_store]
   encryption_key = "a1b2c3...64-char-hex-key"
   ```

   Or via environment variable (not recommended for production):
   ```bash
   CLAWDIUS_ENCRYPTION_KEY="a1b2c3..." ./clawdius-server
   ```

3. Build with the encryption feature:
   ```bash
   cargo build --release --bin clawdius-server --features encryption
   ```

## Prometheus Monitoring

The server exposes metrics at `/metrics`. Key metrics:

- `clawdius_http_requests_total` - HTTP request counter
- `clawdius_http_request_duration_seconds` - Request latency histogram
- `clawdius_usage_messages_total` - Message processing counter by tenant/platform/outcome
- `clawdius_usage_message_duration_ms` - Message processing latency by tenant/platform
- `clawdius_usage_active_tenants` - Gauge of distinct active tenants

A Grafana dashboard and Prometheus alert rules are available in `crates/clawdius-server/monitoring/`.
