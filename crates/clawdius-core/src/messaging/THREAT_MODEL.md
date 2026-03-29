# STRIDE Threat Model: Clawdius Messaging Gateway

**Version:** 1.0
**Date:** 2026-03-28
**Author:** Security Review
**Scope:** `clawdius-core::messaging` + `clawdius-server`

---

## 1. System Overview

The Clawdius Messaging Gateway is a multi-platform webhook server that receives messages from 7 messaging platforms (Telegram, Discord, Matrix, Signal, WhatsApp, Rocket.Chat, Slack), authenticates each request through a dual-layer scheme (global/per-platform API keys via `ApiAuthenticator` + platform-specific cryptographic signature verification via `WebhookReceiver`), normalizes them into `NormalizedMessage` objects, parses commands via `CommandParser`, routes through `MessagingGateway` to typed `MessageHandler` implementations, and sends responses back through platform-specific `MessagingChannel` adapters.

### Data Flow

```
External Platform
  │
  ▼
WebhookServer::process_webhook()
  │  1. ApiAuthenticator::validate()   — global + per-platform API key
  │  2. WebhookReceiver::verify_signature() — platform-specific crypto
  │  3. WebhookReceiver::parse_webhook_body() — deserialize → NormalizedMessage
  │
  ▼
MessagingGateway::process_message()
  │  4. TenantContext (optional) — tenant isolation check
  │  5. RateLimiter::check_rate_limit() — token bucket per user
  │  6. CommandParser::parse() — prefix + category routing
  │  7. SessionBinder::bind_session() — user session lookup/creation
  │  8. PermissionSet check — per-category authorization
  │  9. MessageHandler::handle() — business logic
  │ 10. MessagingChannel::send_message() / edit_message() — response delivery
  │ 11. RetryQueue::enqueue() — failed delivery retry
  │ 12. MessagingAuditLogger::log_event() — audit trail
  │
  ▼
Platform API
```

### Key Components

| Component | Source File | Responsibility |
|-----------|-------------|----------------|
| `WebhookServer` | `server.rs` | HTTP routing, auth orchestration |
| `WebhookReceiver` | `webhook_receiver.rs` | Platform registration, signature verification, body parsing |
| `ApiAuthenticator` | `auth/mod.rs` | API key validation (global + per-platform) |
| `MessagingGateway` | `gateway.rs` | Central message routing, rate limiting, session binding |
| `CommandParser` | `command_parser.rs` | Message → `ParsedCommand` with `CommandCategory` |
| `SessionBinder` | `session_binder.rs` | Per-user session lifecycle with SQLite persistence |
| `RateLimiter` | `rate_limiter.rs` | Token bucket rate limiting per user |
| `PiiRedactionLayer` | `pii_redaction.rs` | `tracing-subscriber::Layer` for log PII scrubbing |
| `MessagingAuditLogger` | `audit.rs` | Bridges `MessagingAuditEvent` → enterprise `AuditEvent` |
| `TenantManager` / `TenantResolver` / `TenantContext` | `tenant.rs` | Multi-tenant isolation, API key → tenant mapping |
| `RetryQueue` | `retry_queue.rs` | Exponential backoff retry with dead letter queue |
| `OAuthTokenStore` / `PlatformOAuthClient` | `oauth.rs` | OAuth 2.0 flows for Slack and Discord |
| `StateStore` (trait) | `state_store.rs` | Pluggable storage backend (memory / SQLite) |
| `PermissionSet` | `types.rs` | Per-session capability flags |
| `MessagingChannel` (trait) | `channels/mod.rs` | Platform send/edit abstraction |

---

## 2. Trust Boundaries

| # | Boundary | Inside | Outside | Crossing Mechanism |
|---|----------|--------|---------|-------------------|
| TB-1 | Internet → Server | `clawdius-server` | External platforms, attackers | HTTPS, webhook endpoints |
| TB-2 | Server → Platform APIs | `clawdius-core::messaging` | Telegram/Discord/Matrix/etc. APIs | Outbound HTTPS (`reqwest`) |
| TB-3 | Server → LLM Provider | `clawdius-core::messaging` | LLM provider API | Outbound HTTPS |
| TB-4 | Multi-tenant | Tenant A resources | Tenant B resources | `TenantManager` + `TenantContext` |
| TB-5 | Process → SQLite | `clawdius-core` | Filesystem | `rusqlite` (direct file access) |
| TB-6 | User → Command | `CommandParser` | Shell / OS | No shell execution (mitigated) |

---

## 3. STRIDE Analysis

### 3.1 Spoofing

| # | Threat | Affected Component | Likelihood | Impact | Mitigation | Status |
|---|--------|-------------------|------------|--------|------------|--------|
| S-1 | Forged webhook — attacker sends fake Telegram update with replayed body | `WebhookReceiver::verify_signature()` via `verify_telegram()` | High | High | `TelegramWebhookConfig.secret_token` compared against `X-Telegram-Bot-Api-Secret-Token` header or `secret_token` query param | Implemented |
| S-2 | Stolen API key — attacker replays a captured API key from a compromised client | `ApiAuthenticator::validate()` in `WebhookServer::process_webhook()` | Medium | High | Dual auth: API key + platform signature both required; per-platform key scoping; HTTPS only | Partial (no key rotation or expiry) |
| S-3 | Slack request signing bypass — forged `X-Slack-Signature` header | `WebhookReceiver::verify_slack()` | Medium | High | `blake3::keyed_hash` with `SlackWebhookConfig.signing_secret` over `v0:{timestamp}:{body}` | Implemented |
| S-4 | Discord webhook signature bypass — forged `X-Signature-Ed25519` header | `WebhookReceiver::verify_discord()` | Medium | High | Checks for presence of `X-Signature-Ed25519` + `X-Signature-Timestamp` headers; `DiscordWebhookConfig.public_key_pem` stored for verification | Partial (header check present, Ed25519 crypto validation not yet implemented — currently returns `InvalidSignature` unconditionally) |
| S-5 | Matrix webhook token replay | `WebhookReceiver::verify_matrix()` | Low | Medium | Static `Bearer` token comparison against `MatrixWebhookConfig.access_token` | Implemented |
| S-6 | Rocket.Chat token replay | `WebhookReceiver::verify_rocketchat()` | Low | Medium | Static token comparison via `X-Rocket-Chat-Token` header or `token` query param | Implemented |
| S-7 | Signal verification token replay | `WebhookReceiver::verify_signal()` | Low | Medium | Static `X-Signal-Token` header comparison against `SignalWebhookConfig.verification_token` | Implemented |
| S-8 | WhatsApp verify token replay | `WebhookReceiver::verify_whatsapp()` | Low | Medium | `hub.verify_token` + `hub.mode=subscribe` query param check | Implemented |
| S-9 | Fake OAuth tokens injected into `OAuthTokenStore` | `OAuthTokenStore` | Low | High | Tokens stored in SQLite with `parking_lot::Mutex` access; validated on use via `is_expired()` | Implemented |

### 3.2 Tampering

| # | Threat | Affected Component | Likelihood | Impact | Mitigation | Status |
|---|--------|-------------------|------------|--------|------------|--------|
| T-1 | Modify webhook body in transit (MITM) | All platform `verify_*()` functions in `WebhookReceiver` | Medium | High | Platform signature verification runs before `parse_webhook_body()`; body integrity checked | Implemented |
| T-2 | Modify outbound response before delivery to platform | `MessagingChannel::send_message()` / `edit_message()` | Low | Medium | HTTPS enforced via `reqwest::Client` | Implemented |
| T-3 | Modify configuration at rest | `WebhookServerConfig`, `TenantConfig`, `ChannelConfig` | Low | High | File permissions, `serde` deserialization (no runtime config hot-reload exposed) | Partial |
| T-4 | SQLite database tampering (sessions, rate limits, retry queue, tenants, OAuth tokens) | `SessionBinder`, `RateLimiter`, `RetryQueue`, `TenantManager`, `OAuthTokenStore` | Low | High | File permissions; WAL journal mode; no encryption at rest | Known gap |
| T-5 | Modify in-memory state during runtime | `SessionBinder.sessions`, `RateLimiter.buckets`, `RetryQueue.tasks` | Low | High | `tokio::sync::RwLock` / `parking_lot::RwLock` / `parking_lot::Mutex` for interior mutability | Implemented |
| T-6 | Tenant config tampering to elevate permissions | `TenantConfig.default_permissions`, `TenantConfig.command_whitelist` | Low | High | `TenantManager` CRUD gated by Rust ownership; SQLite persistence with `Mutex` | Implemented |

### 3.3 Repudiation

| # | Threat | Affected Component | Likelihood | Impact | Mitigation | Status |
|---|--------|-------------------|------------|--------|------------|--------|
| R-1 | User denies sending a command | `MessagingAuditLogger` | Medium | Medium | `MessagingAuditEvent::MessageReceived`, `CommandExecuted`, `ResponseSent` logged with platform, user_id, command, latency, outcome | Implemented |
| R-2 | Admin denies configuration change | `TenantManager` operations | Low | Medium | `TenantConfig.updated_at` timestamp; enterprise audit trail via `AuditEvent` | Partial |
| R-3 | Platform denies webhook was received | `RetryQueue` | Low | Low | `RetryTask.attempt`, `next_retry_at`, `last_error` tracked; dead letter queue for exhausted tasks | Implemented |
| R-4 | Attacker denies rate limit violations | `MessagingAuditLogger` | Medium | Low | `MessagingAuditEvent::RateLimitHit` logged with platform, user_id, limit, remaining | Implemented |
| R-5 | Permission denied events not attributable | `MessagingAuditLogger` | Low | Medium | `MessagingAuditEvent::PermissionDenied` logged with user_id, command, required_permission | Implemented |

### 3.4 Information Disclosure

| # | Threat | Affected Component | Likelihood | Impact | Mitigation | Status |
|---|--------|-------------------|------------|--------|------------|--------|
| I-1 | API keys / secrets in log output | `PiiRedactionLayer` | Medium | High | `SENSITIVE_FIELD_NAMES` includes `token`, `api_key`, `secret`, `password`, `authorization`, `signing_secret`, `app_secret`, `verify_token`; field-level redaction | Implemented |
| I-2 | User message content in logs | `PiiRedactionLayer` | High | Medium | `content` field mapped to `[REDACTED:USER_CONTENT]` via `replacement_for_field()` | Implemented |
| I-3 | User IDs and usernames in logs | `PiiRedactionLayer` | Medium | Medium | `user_id` and `username` fields mapped to `[REDACTED:ID]` | Implemented |
| I-4 | Bot tokens exposed in error responses | `WebhookServer::process_webhook()` | Low | High | Error responses return generic JSON (`{"error":"..."}`) without tokens; `MessagingError::AuthenticationFailed` message does not include credentials | Implemented |
| I-5 | Source IP addresses in logs | `PiiRedactionLayer` | Low | Medium | `ip_address` and `source_ip` fields mapped to `[REDACTED:IP]` | Implemented |
| I-6 | OAuth tokens in logs | `PiiRedactionLayer` | Medium | High | `access_token`, `refresh_token`, `client_secret`, `session_token` in `SENSITIVE_FIELD_NAMES`; Bearer token pattern (`Bearer [A-Za-z0-9\-._~+/]+=*`) redacted in values | Implemented |
| I-7 | Cross-tenant data leakage | `TenantManager`, `TenantContext` | Medium | High | `TenantContext::is_platform_allowed()` and `is_command_allowed()` enforce isolation; per-tenant `PermissionSet` | Implemented |
| I-8 | Session data in error traces | `SessionBinder` | Low | Medium | Session IDs logged for correlation; session content not included in error messages | Implemented |
| I-9 | Phone numbers and emails in log values | `PiiRedactionLayer` | Medium | Medium | Regex-based value pattern redaction: phone (`+[1-9]\d{6,14}`), email, Bearer tokens, long API-key-like strings (`[A-Za-z0-9]{32,}`) | Implemented |

### 3.5 Denial of Service

| # | Threat | Affected Component | Likelihood | Impact | Mitigation | Status |
|---|--------|-------------------|------------|--------|------------|--------|
| D-1 | Webhook flood — rate limit bypass across many user IDs | `RateLimiter` | High | Medium | Per-user token bucket (`TokenBucket::try_consume`); configurable `RateLimitConfig.requests_per_minute` (default 20) and `burst_capacity` (default 10) | Implemented |
| D-2 | Large request body causing memory exhaustion | `WebhookServerConfig.max_request_size_bytes` | Medium | Medium | Default `max_request_size_bytes: 1_000_000` (1 MB); enforced at HTTP layer | Implemented |
| D-3 | Slow LLM responses blocking handler threads | `MessageHandler::handle()` | Medium | Low | Async handlers via `#[async_trait]`; `MessagingGateway` is non-blocking | Partial (no explicit timeout on handler calls) |
| D-4 | Retry queue exhaustion | `RetryQueue` | Low | Medium | `RetryConfig.max_queue_size` (default 10,000); `RetryConfig.max_retries` (default 5); dead letter queue for exhausted tasks; `purge()` method | Implemented |
| D-5 | SQLite lock contention under concurrent writes | `SessionBinder`, `RateLimiter`, `RetryQueue` | Low | Medium | `spawn_blocking` for SQLite I/O; `parking_lot::Mutex` / `tokio::sync::RwLock`; WAL journal mode; `StateStore` trait for future backend swap | Partial |
| D-6 | Regex DoS via crafted command patterns | `CommandParser` | Low | Medium | Pre-compiled `Regex` patterns; no user-controlled regex input | Implemented |
| D-7 | Rate limit state explosion (unbounded bucket creation) | `RateLimiter` | Low | Low | `cleanup_inactive()` method removes stale buckets; in-memory bounded by OS | Implemented |

### 3.6 Elevation of Privilege

| # | Threat | Affected Component | Likelihood | Impact | Mitigation | Status |
|---|--------|-------------------|------------|--------|------------|--------|
| E-1 | Command injection via message content | `CommandParser::parse()` | Medium | High | Strict prefix matching (`Platform::command_prefix()`); regex-based category routing; no shell execution from message content; `ParsedCommand.args` and `ParsedCommand.flags` are plain strings | Implemented |
| E-2 | Privilege escalation via session manipulation | `SessionBinder::bind_session()` | Low | High | `PermissionSet` checked per `CommandCategory` in `MessagingGateway::check_permissions()`; permissions stored in SQLite and loaded on bind | Implemented |
| E-3 | Non-admin user invoking admin commands | `MessagingGateway::check_permissions()` | Low | High | `session.permissions.can_admin` checked before `CommandCategory::Admin` handlers execute | Implemented |
| E-4 | Cross-tenant privilege escalation | `TenantManager`, `TenantContext` | Low | High | `TenantContext::is_command_allowed()` filters by `command_whitelist`; `is_platform_allowed()` restricts platform access; `default_permissions` scoped per tenant | Implemented |
| E-5 | OAuth scope escalation | `PlatformOAuthClient` | Low | High | Minimal required scopes: Slack (`chat:write`, `channels:history`, `app_mentions:read`, `commands`), Discord (`bot`, `messages.read`); `DiscordOAuthConfig.permissions` bitmask | Implemented |
| E-6 | Generate/Analyze commands by unauthorized user | `MessagingGateway::check_permissions()` | Low | Medium | `session.permissions.can_generate` / `can_analyze` checked; default `PermissionSet::new()` allows both; `read_only()` restricts | Implemented |

---

## 4. Risk Summary

### Critical (Immediate Action)

None — all critical threats have implemented mitigations.

### High (Near-term)

| Risk | Threat ID | Recommendation |
|------|-----------|---------------|
| Discord Ed25519 verification not fully implemented | S-4 | `verify_discord()` currently checks header presence but does not perform actual Ed25519 signature validation against `public_key_pem`. Implement full cryptographic verification. |
| No API key rotation or expiry | S-2 | Add key creation timestamp, TTL, and rotation API to `ApiAuthenticator`. Consider `expiring-api-keys` table in SQLite. |
| SQLite encryption at rest | T-4 | Database files contain sessions, rate limits, OAuth tokens, tenant configs. Consider SQLCipher or filesystem-level encryption (LUKS, ecryptfs) for production. |

### Medium (Planned)

| Risk | Threat ID | Recommendation |
|------|-----------|---------------|
| No LLM call timeout | D-3 | Add configurable `tokio::time::timeout` around `MessageHandler::handle()` calls. Consider circuit breaker pattern. |
| SQLite contention under load | D-5 | Current `spawn_blocking` + `parking_lot::Mutex` works for single-server. For horizontal scaling, implement Redis or PostgreSQL `StateStore` backend. |
| Static token verification (Matrix, Rocket.Chat, Signal) | S-5, S-6, S-7 | These platforms use simple token comparison (no HMAC). Acceptable per platform docs but monitor for replay attacks. |
| Config tampering at rest | T-3 | Add config file integrity verification (hash check) or store sensitive config in keyring. |

### Low (Acceptable)

All other threats have adequate mitigations for the current use case (internal tool / single-tenant deployment).

---

## 5. Security Controls Inventory

| Control | Implementation | Component | Standard |
|---------|---------------|-----------|----------|
| Webhook signature verification | `WebhookReceiver::verify_signature()` dispatches to per-platform `verify_*()` | `webhook_receiver.rs` | OWASP API5 |
| API key authentication | `ApiAuthenticator::validate()` with global + per-platform `HashSet<String>` keys | `auth/mod.rs` | OWASP API2 |
| Rate limiting | `RateLimiter` with `TokenBucket` (O(1) check/consume), configurable `RateLimitConfig` | `rate_limiter.rs` | OWASP API4 |
| PII redaction | `PiiRedactionLayer` (tracing `Layer`): field-name + value-pattern redaction | `pii_redaction.rs` | GDPR Art. 25 |
| Audit logging | `MessagingAuditLogger::log_event()` → enterprise `AuditLogger` via `MessagingAuditEvent` → `AuditEvent` | `audit.rs` | NIST AU-2 |
| Session isolation | `SessionBinder::bind_session()` keyed by `PlatformUserId::composite_key()` (`{platform}:{user_id}`) | `session_binder.rs` | NIST AC-3 |
| Tenant isolation | `TenantContext::is_platform_allowed()` + `is_command_allowed()` + per-tenant `PermissionSet` | `tenant.rs` | NIST SC-7 |
| Permission enforcement | `MessagingGateway::check_permissions()` maps `CommandCategory` → `PermissionSet` flags | `gateway.rs` | NIST AC-1 |
| Request size limiting | `WebhookServerConfig.max_request_size_bytes` (default 1 MB) | `server.rs` | OWASP API4 |
| HTTPS enforcement | `reqwest::Client` for outbound; TLS termination at server layer | `oauth.rs`, channels | NIST SC-8 |
| Retry with backoff | `RetryQueue` with exponential backoff + jitter (`RetryConfig`), dead letter queue | `retry_queue.rs` | Resilience |
| OAuth token management | `OAuthTokenStore` with `is_expired()`, `refresh_token()`, `revoke_token()`; `PlatformOAuthClient` for Slack/Discord | `oauth.rs` | NIST IA-5 |
| Pluggable state storage | `StateStore` trait → `InMemoryStateStore` / `SqliteStateStore` via `StateStoreFactory` | `state_store.rs` | NIST SC-28 |
| Timestamp replay protection | Slack `X-Slack-Request-Timestamp` included in signature base string | `webhook_receiver.rs` | OWASP API8 |

---

## 6. Data Classification

| Data Type | Location | Sensitivity | Encryption | Redaction |
|-----------|----------|-------------|------------|-----------|
| Platform webhook secrets | `WebhookConfig` variants | High | No (config file / env) | Yes (field name) |
| API keys | `ApiAuthenticator.keys`, `TenantResolver.api_keys` | High | No (in-memory / SQLite) | Yes (field name + value pattern) |
| OAuth tokens | `OAuthTokenStore` (SQLite) | Critical | No | Yes (field name) |
| User messages | `NormalizedMessage.content` | Medium | No | Yes (`[REDACTED:USER_CONTENT]`) |
| User IDs | `AuthenticatedUser.platform_user_id` | Medium | No | Yes (`[REDACTED:ID]`) |
| Session state | `SessionBinder.sessions` (SQLite) | Medium | No | Partial (IDs only) |
| Audit logs | `MessagingAuditLogger` → `AuditStorage` | Medium | No | No (by design — audit trail must be complete) |
| Rate limit state | `RateLimiter.buckets` | Low | No | N/A |
| Retry queue | `RetryQueue.tasks` | Low | No | N/A |

---

## 7. Recommendations

### Short-term

1. **Implement full Discord Ed25519 verification** in `verify_discord()` — currently only validates header presence, not the cryptographic signature against `public_key_pem`.
2. **Add API key rotation** — extend `ApiAuthenticator` with creation timestamps, TTL, and a `rotate_key()` method.
3. **Add LLM call timeout** — wrap `MessageHandler::handle()` in `tokio::time::timeout` with configurable duration.
4. **Add Slack timestamp replay window** — reject requests where `X-Slack-Request-Timestamp` is older than 5 minutes.

### Medium-term

5. **Database encryption at rest** — evaluate SQLCipher integration for `SessionBinder`, `RateLimiter`, `RetryQueue`, `OAuthTokenStore`, and `TenantManager` SQLite files.
6. **Webhook IP allowlisting** — add configurable IP ranges for known platform webhook IPs (Telegram, Slack, Discord publish their IP ranges).
7. **Secret management** — integrate with OS keyring (e.g., `keyring` crate) for `WebhookConfig` secrets instead of plaintext config files.
8. **Add Slack HMAC-SHA256 as alternative** — the current `blake3::keyed_hash` implementation works but deviates from Slack's documented HMAC-SHA256 spec.

### Long-term

9. **Horizontal scaling backend** — implement Redis or PostgreSQL `StateStore` backend to replace SQLite for multi-instance deployments.
10. **Formal penetration testing** — engage a security firm for a focused assessment before any SaaS/external-facing deployment.
11. **CSP and CORS hardening** — review `WebhookServerConfig.cors_origins` defaults; ensure no wildcard origins in production.

---

## 8. Assumptions

- The server runs behind a reverse proxy (nginx/Caddy) that handles TLS termination.
- SQLite database files are protected by OS file permissions (owner read/write only).
- Platform API credentials (bot tokens, OAuth secrets) are injected via environment variables or a secrets manager, not committed to version control.
- The `WebhookServerConfig.max_request_size_bytes` limit is enforced by the HTTP framework layer before body parsing.
- Multi-tenant isolation assumes tenants do not share API keys (each tenant maps to distinct keys via `TenantResolver`).
- The `blake3::keyed_hash` used for Slack verification provides equivalent integrity guarantees to HMAC-SHA256 for this use case (single-tenant, no length-extension concerns).

---

## 9. Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-03-28 | Security Review | Initial STRIDE analysis |
