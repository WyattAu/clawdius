# Migration Guide

This document provides migration guides for upgrading between Clawdius versions.

---

## Upgrading to 1.0.0

Clawdius 1.0.0 is the first stable release. There is no prior stable version to migrate from.

### Pre-release (rc.x) Users

If you were using an rc.x release, the following changes apply for 1.0.0:

#### Gateway Crate Lint Tightening

The `clawdius-gateway` crate no longer permits blanket `unwrap()`/`expect()` in production code. The crate-level `#![allow(clippy::unwrap_used)]` and `#![allow(clippy::expect_used)]` attributes have been removed.

- **What changed:** All `unwrap()` and `expect()` calls in non-test code must either use per-function `#[allow]` annotations with documented justification, or use proper error propagation (`?`, `map_err`).
- **Impact:** If you depend on `clawdius-gateway` internals, any code relying on panicking behavior may see compilation warnings with `-D clippy::unwrap_used`.
- **Action:** Review downstream code that calls gateway internals. Use `Result` return types where possible.

#### Signal Handler Error Handling

The gateway binary (`clawdius-gateway`) now handles signal handler registration failures gracefully instead of panicking.

- **What changed:** `tokio::signal::ctrl_c()` and `tokio::signal::unix::signal()` failures no longer panic. A warning is logged and the alternative signal mechanism is used.
- **Impact:** None for normal operation. Containers with restricted signal access will no longer crash at startup.

---

## Public API Surface Reference

### clawdius-core (library)

The primary library crate. All public types below are covered by the SemVer stability guarantee.

| Module | Key Types |
|--------|-----------|
| `config` | `Config` |
| `llm` | `LlmClient`, `LlmConfig`, `LlmResponse`, `ChatMessage`, `ChatRole`, `LlmTokenUsage`, `LlmResponseCache` |
| `session` | `Session`, `SessionManager`, `SessionStore` |
| `tools` | Tool traits and execution types |
| `context` | `Context`, `ContextCompactor`, `ContextWindowManager`, `Mention`, `MentionResolver` |
| `agentic` | `AgenticSystem`, `TaskRequest`, `TaskResult`, `GenerationOptions` |
| `analysis` | `DriftDetector`, `DebtAnalyzer`, `DriftReport`, `DebtReport` |
| `checkpoint` | `TimelineManager`, `CheckpointId` |
| `memory` | `ProjectMemory`, `MemoryEntry` |
| `skills` | `Skill`, `SkillRegistry`, `SkillContext` |
| `storage` | `StorageBackend`, `SqliteBackend`, `InMemoryBackend`, `GraphRepository` |
| `retry` | `CircuitBreaker`, `with_retry_and_circuit` |
| `telemetry` | `TelemetryConfig`, `CrashReporter` |
| `output` | `OutputFormat` |
| `error` | `Error`, `EnhancedError`, `Result` |

### clawdius-gateway (messaging)

| Type | Description |
|------|-------------|
| `MessageGateway` | Central message routing and adapter management |
| `ClawdiusHandler` | Bridges messaging gateway to the agent engine |
| `PlatformAdapter` | Trait for platform-specific adapters |
| `IncomingMessage` / `OutgoingMessage` | Message types for platform communication |
| `Platform` | Enum of supported platforms |
| `PlatformConfig` | Configuration for platform adapters |
| `RateLimiter` | Per-user, per-platform sliding window rate limiter |
| `ResponseFormatter` | Formats LLM responses for platform delivery |
| `GatewayError` | Gateway error types |

### clawdius-mcp (MCP server)

| Function | Description |
|----------|-------------|
| `parse_request(raw: &str)` | Parse raw JSON into `McpRequest` |
| `format_response(response: &McpResponse)` | Serialize `McpResponse` to JSON |

### clawdius-code (editor helper)

| Function | Description |
|----------|-------------|
| `parse_request(raw: &str)` | Parse raw JSON into JSON-RPC `Request` |
| `format_response(response: &Response)` | Serialize JSON-RPC `Response` to JSON |

### clawdius (CLI)

The CLI binary exposes its argument parser for testing via `clawdius::cli::{Cli, Commands, OutputFormat}`. The CLI interface (command names, flags, output formats) is stable per the API Stability Guarantee.

---

## Configuration Changes

### Config File Location

Clawdius searches for configuration in this order:

1. `--config /path/to/config.toml` (explicit path)
2. `CLAWDIUS_CONFIG` environment variable
3. `~/.config/clawdius/config.toml` (user default)
4. `.clawdius/config.toml` (project local)

### Config File Schema

The configuration schema is stable. See [API_STABILITY.md](./API_STABILITY.md) for details.

---

## Breaking Changes Policy

Breaking changes are only introduced in major versions (2.0.0, 3.0.0, etc.). When a breaking change is planned:

1. The old API is marked `#[deprecated]` with a migration suggestion
2. The deprecated API remains functional for at least 2 minor releases
3. A migration guide section is added to this document
4. The breaking change ships in the next major release

See [API_STABILITY.md](./API_STABILITY.md) for the full deprecation policy.
