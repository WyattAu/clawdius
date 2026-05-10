#![allow(
    dead_code,
    missing_docs,
    unused_imports,
    unused_variables,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::clone_on_copy,
    clippy::doc_lazy_continuation,
    clippy::doc_markdown,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::format_collect,
    clippy::from_over_into,
    clippy::ignored_unit_patterns,
    clippy::items_after_statements,
    clippy::let_and_return,
    clippy::manual_is_multiple_of,
    clippy::manual_range_contains,
    clippy::match_single_binding,
    clippy::missing_const_for_fn,
    clippy::must_use_candidate,
    clippy::needless_return,
    clippy::panic,
    clippy::redundant_clone,
    clippy::return_self_not_must_use,
    clippy::similar_names,
    clippy::single_match_else,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    clippy::unreadable_literal,
    clippy::unwrap_used
)]
use clawdius_core::Error;

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
