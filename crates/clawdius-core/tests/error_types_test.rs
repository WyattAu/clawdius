use clawdius_core::{Error, Result};

#[test]
fn test_retryable_errors() {
    let rate_limited = Error::RateLimited {
        retry_after_ms: 1000,
    };
    assert!(rate_limited.is_retryable());
    assert!(rate_limited.is_rate_limited());
    assert_eq!(rate_limited.retry_after_ms(), Some(1000));

    let timeout = Error::Timeout(std::time::Duration::from_secs(5));
    assert!(timeout.is_retryable());
    assert!(timeout.is_timeout());
    assert_eq!(timeout.retry_after_ms(), Some(5000));

    let circuit_open = Error::CircuitBreakerOpen {
        service: "test".to_string(),
        last_error: "error".to_string(),
    };
    assert!(circuit_open.is_retryable());
    assert!(circuit_open.is_circuit_breaker());

    let config_error = Error::Config("test".to_string());
    assert!(!config_error.is_retryable());
    assert_eq!(config_error.retry_after_ms(), None);
}

#[test]
fn test_new_error_types() {
    let llm_provider = Error::LlmProvider {
        message: "API error".to_string(),
        provider: "anthropic".to_string(),
    };
    assert!(llm_provider.to_string().contains("anthropic"));
    assert!(llm_provider.to_string().contains("API error"));

    let context_limit = Error::ContextLimit {
        current: 10000,
        limit: 8000,
    };
    assert!(context_limit.to_string().contains("10000"));
    assert!(context_limit.to_string().contains("8000"));

    let tool_exec = Error::ToolExecution {
        tool: "bash".to_string(),
        reason: "timeout".to_string(),
    };
    assert!(tool_exec.to_string().contains("bash"));
    assert!(tool_exec.to_string().contains("timeout"));

    let session_not_found = Error::SessionNotFound {
        id: "session-123".to_string(),
    };
    assert!(session_not_found.to_string().contains("session-123"));
}
