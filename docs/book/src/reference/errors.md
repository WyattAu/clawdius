# Error Codes

Reference for Clawdius error types and their meanings.

## Core Error Type

The `clawdius_core::Error` enum is the top-level error type. It is defined in `crates/clawdius-core/src/error/mod.rs`.

## Error Variants

### Configuration

| Variant | Message Template | Retryable | Description |
|---------|-----------------|-----------|-------------|
| `Config(msg)` | `Configuration error: {msg}` | No | Invalid or missing configuration |

### LLM

| Variant | Message Template | Retryable | Description |
|---------|-----------------|-----------|-------------|
| `Llm(msg)` | `LLM error: {msg}` | No | General LLM failure |
| `LlmProvider { message, provider }` | `LLM provider error from {provider}: {message}` | No | Provider-specific error |
| `RateLimited { retry_after_ms }` | `Rate limited. Retry after {retry_after_ms}ms` | Yes | HTTP 429 rate limit |
| `ContextLimit { current, limit }` | `Context limit exceeded: {current}/{limit} tokens` | No | Token budget exceeded |
| `RetryExhausted(n)` | `Retry exhausted after {n} attempts` | No | All retries failed |
| `CircuitBreakerOpen { service, last_error }` | `Circuit breaker open for {service}` | Yes | Service circuit open |
| `QuotaExceeded(msg)` | `Quota exceeded: {msg}` | No | API quota limit reached |

### Tool Execution

| Variant | Message Template | Retryable | Description |
|---------|-----------------|-----------|-------------|
| `ToolExecution { tool, reason }` | `Tool execution failed '{tool}': {reason}` | No | Tool failed |
| `Tool(msg)` | `Tool error: {msg}` | No | Tool configuration error |
| `Sandbox(msg)` | `Sandbox error: {msg}` | No | Sandbox violation |

### Session

| Variant | Message Template | Retryable | Description |
|---------|-----------------|-----------|-------------|
| `Session(msg)` | `Session error: {msg}` | No | General session error |
| `SessionNotFound { id }` | `Session not found: {id}` | No | Invalid session ID |

### I/O and Serialization

| Variant | Message Template | Retryable | Description |
|---------|-----------------|-----------|-------------|
| `Io(err)` | `IO error: {err}` | No | File system error |
| `Serialization(err)` | `Serialization error: {err}` | No | JSON error |
| `TomlDe(err)` | `TOML deserialization error: {err}` | No | Config parse error |
| `TomlSer(err)` | `TOML serialization error: {err}` | No | Config write error |
| `Database(err)` | `Database error: {err}` | No | SQLite error |
| `Network(msg)` | `Network error: {msg}` | No | Network failure |

### System

| Variant | Message Template | Retryable | Description |
|---------|-----------------|-----------|-------------|
| `Timeout(duration)` | `Operation timed out after {duration:?}` | Yes | Operation timeout |
| `Cancelled` | `Operation cancelled` | No | User cancelled |
| `Internal(msg)` | `Internal error: {msg}` | No | Bug in Clawdius |
| `NotFound(msg)` | `Not found: {msg}` | No | Resource not found |
| `InvalidInput(msg)` | `Invalid input: {msg}` | No | Bad user input |
| `ParseError(msg)` | `Parse error: {msg}` | No | Parse failure |
| `UnsupportedLanguage(msg)` | `Unsupported language: {msg}` | No | Language not supported |

### Additional

| Variant | Message Template | Retryable | Description |
|---------|-----------------|-----------|-------------|
| `Auth(msg)` | `Authentication error: {msg}` | No | Auth failure |
| `Checkpoint(msg)` | `Checkpoint error: {msg}` | No | Checkpoint failure |
| `Brain(msg)` | `Brain runtime error: {msg}` | No | WASM runtime error |
| `Rpc(msg)` | `RPC error: {msg}` | No | JSON-RPC error |
| `Model(msg)` | `Model error: {msg}` | No | Model loading error |
| `Processing(msg)` | `Processing error: {msg}` | No | Data processing error |
| `Sprint(msg)` | `Sprint error: {msg}` | No | Sprint execution error |

## Error Helper Methods

```rust
use clawdius_core::Error;

let error = Error::RateLimited { retry_after_ms: 5000 };

error.is_retryable();       // true
error.retry_after_ms();      // Some(5000)
error.is_rate_limited();     // true
error.is_timeout();          // false
error.is_circuit_breaker();  // false
error.user_message();        // Human-friendly message
```

## Retry Logic

Errors are retryable when `is_retryable()` returns `true`. The retry system uses exponential backoff:

```
delay = min(initial_delay * base^attempt + jitter, max_delay)
```

Default values: `initial_delay = 1000ms`, `base = 2.0`, `max_delay = 30000ms`.

## Enhanced Errors

The `EnhancedError` type provides rich, actionable error messages with context, suggestions, and documentation links:

```rust
use clawdius_core::error::ErrorHelpers;

let error = ErrorHelpers::file_not_found("/path/to/file");
println!("{}", error.format_pretty());
```
