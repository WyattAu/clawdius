# Error Handling and Structured Logging Guide

This document describes the enhanced error handling and structured logging system in Clawdius v0.6.0.

## Enhanced Error Types

### New Error Variants

The `Error` enum in `clawdius-core/src/error.rs` has been enhanced with more specific error types:

#### LLM Provider Errors
```rust
Error::LlmProvider {
    message: String,
    provider: String,
}
```
Use this for LLM API-specific errors with provider context.

#### Rate Limiting
```rust
Error::RateLimited {
    retry_after_ms: u64,
}
```
Indicates rate limiting with retry delay information.

#### Context Limit
```rust
Error::ContextLimit {
    current: usize,
    limit: usize,
}
```
For token limit errors with detailed metrics.

#### Tool Execution
```rust
Error::ToolExecution {
    tool: String,
    reason: String,
}
```
For tool execution failures with context.

#### Session Not Found
```rust
Error::SessionNotFound {
    id: String,
}
```
Specific error for missing sessions.

#### Circuit Breaker
```rust
Error::CircuitBreakerOpen {
    service: String,
    last_error: String,
}
```
For circuit breaker pattern implementation.

### Error Helper Methods

```rust
impl Error {
    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool;
    
    /// Get retry delay in milliseconds
    pub fn retry_after_ms(&self) -> Option<u64>;
    
    /// Check if rate limited
    pub fn is_rate_limited(&self) -> bool;
    
    /// Check if timeout
    pub fn is_timeout(&self) -> bool;
    
    /// Check if circuit breaker
    pub fn is_circuit_breaker(&self) -> bool;
}
```

### Usage Example

```rust
use clawdius_core::{Error, Result};

async fn call_llm() -> Result<String> {
    match llm_client.chat(request).await {
        Ok(response) => Ok(response.content),
        Err(e) if e.is_rate_limited() => {
            if let Some(delay) = e.retry_after_ms() {
                tokio::time::sleep(Duration::from_millis(delay)).await;
                // Retry
            }
            Err(e)
        }
        Err(e) => Err(e),
    }
}
```

## Structured Logging

### Configuration

Logging is configured in `clawdius.toml`:

```toml
[telemetry.logging]
level = "info"  # trace, debug, info, warn, error
json_format = false
show_target = true
show_thread_ids = false
show_thread_names = false
compact = true
```

### Initialization

```rust
use clawdius_core::{LoggingConfig, init_logging};

let config = LoggingConfig {
    level: "debug".to_string(),
    json_format: true,
    ..Default::default()
};

init_logging(&config);
```

### Using Tracing

#### Basic Logging

```rust
use tracing::{info, warn, error, debug};

info!("Processing request");
warn!("Rate limit approaching");
error!("Failed to process message");
debug!("Internal state: {:?}", state);
```

#### Structured Fields

```rust
use tracing::{info, warn, error};

info!(
    user_id = %user.id,
    action = "login",
    "User logged in"
);

warn!(
    provider = %provider,
    remaining = rate_limit.remaining,
    "Rate limit approaching"
);

error!(
    error = %e,
    provider = "anthropic",
    model = "claude-3-5-sonnet",
    "LLM call failed"
);
```

#### Instrumentation with Spans

```rust
use tracing::instrument;

#[instrument(skip_all)]
pub async fn chat(&self, message: &str) -> Result<String> {
    info!("Processing chat message");
    
    let response = self.llm_client.chat(message).await?;
    
    info!(
        tokens_used = response.usage.total_tokens,
        "Chat completed"
    );
    
    Ok(response.content)
}

#[instrument(skip(self, request))]
pub async fn execute_tool(
    &self,
    tool: &str,
    request: &ToolRequest,
) -> Result<ToolResponse> {
    debug!(tool = %tool, "Executing tool");
    
    let result = self.run_tool(tool, request).await?;
    
    info!(
        tool = %tool,
        duration_ms = result.duration.as_millis(),
        "Tool execution completed"
    );
    
    Ok(result)
}
```

#### Creating Custom Spans

```rust
use tracing::{info, span, Level};

let span = span!(Level::INFO, "request_processing", request_id = %id);
let _enter = span.enter();

info!("Processing started");
// ... work ...
info!("Processing completed");
```

## Error Recovery

### Retry Logic

The `retry` module provides retry logic with exponential backoff:

```rust
use clawdius_core::{with_retry_and_circuit, CircuitBreaker};

let circuit_breaker = CircuitBreaker::new(
    "llm-provider",
    5,      // failure threshold
    30000,  // recovery timeout (ms)
);

let result = with_retry_and_circuit(
    3,      // max retries
    1000,   // initial delay (ms)
    30000,  // max delay (ms)
    2.0,    // exponential base
    Some(&circuit_breaker),
    || async {
        llm_client.chat(request).await
    },
).await?;
```

### Circuit Breaker Pattern

```rust
use clawdius_core::{CircuitBreaker, CircuitState};

let cb = CircuitBreaker::new("external-api", 3, 60000);

// Check if requests are allowed
if cb.is_allowed().await.is_ok() {
    match make_request().await {
        Ok(response) => {
            cb.record_success().await;
            Ok(response)
        }
        Err(e) if e.is_retryable() => {
            cb.record_failure(&e).await;
            Err(e)
        }
        Err(e) => Err(e),
    }
} else {
    Err(Error::CircuitBreakerOpen {
        service: "external-api".to_string(),
        last_error: "Service unavailable".to_string(),
    })
}
```

### Fallback Responses

```rust
use clawdius_core::{Error, Result};

async fn get_with_fallback<T>(
    primary: impl Future<Output = Result<T>>,
    fallback: impl Future<Output = Result<T>>,
) -> Result<T> {
    match primary.await {
        Ok(result) => Ok(result),
        Err(e) if e.is_retryable() => {
            warn!(error = %e, "Primary failed, using fallback");
            fallback.await
        }
        Err(e) => Err(e),
    }
}
```

## Best Practices

1. **Use Specific Error Types**: Always use the most specific error variant available.

2. **Include Context**: Add relevant context to errors using structured fields.

3. **Log at Appropriate Levels**:
   - `trace`: Very detailed internal state
   - `debug`: Detailed debugging information
   - `info`: Important business events
   - `warn`: Recoverable issues
   - `error`: Failures that affect operations

4. **Use Instrumentation**: Add `#[instrument]` to public functions for automatic span creation.

5. **Handle Retries Appropriately**: Use the retry module for transient errors, not for configuration or authentication errors.

6. **Implement Circuit Breakers**: Use circuit breakers for external service calls to prevent cascading failures.

7. **Provide Fallbacks**: Where possible, implement fallback responses for degraded service.

## Configuration Example

Complete logging configuration:

```toml
[telemetry]
crash_reporting = false
sentry_dsn = ""

[telemetry.logging]
level = "info"
json_format = false
show_target = true
show_thread_ids = false
show_thread_names = false
compact = true
```

For production JSON logging:

```toml
[telemetry.logging]
level = "info"
json_format = true
show_target = true
show_thread_ids = true
compact = false
```

For development:

```toml
[telemetry.logging]
level = "debug"
json_format = false
show_target = true
show_thread_ids = false
compact = true
```
