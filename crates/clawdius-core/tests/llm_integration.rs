//! Integration tests for LLM providers
//!
//! Tests provider configuration, retry logic, and error handling.

use clawdius_core::{with_retry_and_circuit, CircuitBreaker, CircuitState, Error};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

mod provider_config_tests {
    use super::*;

    fn create_chat_message(
        role: clawdius_core::llm::ChatRole,
        content: &str,
    ) -> clawdius_core::llm::ChatMessage {
        clawdius_core::llm::ChatMessage {
            role,
            content: content.to_string(),
        }
    }

    #[test]
    fn test_anthropic_config_from_values() {
        let config = clawdius_core::llm::LlmConfig {
            provider: "anthropic".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            api_key: Some("test-api-key".to_string()),
            base_url: None,
            max_tokens: 4096,
        };

        let provider = clawdius_core::llm::create_provider(&config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_anthropic_config_custom_model() {
        let config = clawdius_core::llm::LlmConfig {
            provider: "anthropic".to_string(),
            model: "claude-3-opus".to_string(),
            api_key: Some("test-api-key".to_string()),
            base_url: None,
            max_tokens: 8192,
        };

        let provider = clawdius_core::llm::create_provider(&config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_anthropic_config_missing_api_key() {
        let config = clawdius_core::llm::LlmConfig {
            provider: "anthropic".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            api_key: None,
            base_url: None,
            max_tokens: 4096,
        };

        let result = clawdius_core::llm::create_provider(&config);
        assert!(result.is_err());
        if let Err(err) = result {
            assert!(matches!(err, Error::Config(_)));
        }
    }

    #[test]
    fn test_openai_config_from_values() {
        let config = clawdius_core::llm::LlmConfig {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            api_key: Some("test-api-key".to_string()),
            base_url: None,
            max_tokens: 4096,
        };

        let provider = clawdius_core::llm::create_provider(&config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_openai_config_custom_model() {
        let config = clawdius_core::llm::LlmConfig {
            provider: "openai".to_string(),
            model: "gpt-4-turbo".to_string(),
            api_key: Some("test-api-key".to_string()),
            base_url: None,
            max_tokens: 8192,
        };

        let provider = clawdius_core::llm::create_provider(&config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_openai_config_missing_api_key() {
        let config = clawdius_core::llm::LlmConfig {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            api_key: None,
            base_url: None,
            max_tokens: 4096,
        };

        let result = clawdius_core::llm::create_provider(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_ollama_config_from_values() {
        let config = clawdius_core::llm::LlmConfig {
            provider: "ollama".to_string(),
            model: "llama3.2".to_string(),
            api_key: None,
            base_url: Some("http://localhost:11434".to_string()),
            max_tokens: 4096,
        };

        let provider = clawdius_core::llm::create_provider(&config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_ollama_config_custom_model() {
        let config = clawdius_core::llm::LlmConfig {
            provider: "ollama".to_string(),
            model: "llama2".to_string(),
            api_key: None,
            base_url: Some("http://localhost:11434".to_string()),
            max_tokens: 4096,
        };

        let provider = clawdius_core::llm::create_provider(&config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_ollama_config_default_base_url() {
        let config = clawdius_core::llm::LlmConfig {
            provider: "ollama".to_string(),
            model: "llama3.2".to_string(),
            api_key: None,
            base_url: None,
            max_tokens: 4096,
        };

        let provider = clawdius_core::llm::create_provider(&config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_unknown_provider_error() {
        let config = clawdius_core::llm::LlmConfig {
            provider: "unknown-provider".to_string(),
            model: "model".to_string(),
            api_key: Some("key".to_string()),
            base_url: None,
            max_tokens: 4096,
        };

        let result = clawdius_core::llm::create_provider(&config);
        assert!(result.is_err());
        if let Err(err) = result {
            assert!(matches!(err, Error::Config(_)));
        }
    }

    #[test]
    fn test_chat_message_creation() {
        let msg = create_chat_message(clawdius_core::llm::ChatRole::User, "Hello");
        assert_eq!(msg.content, "Hello");
        assert_eq!(msg.role, clawdius_core::llm::ChatRole::User);
    }

    #[test]
    fn test_chat_message_serialization() {
        let msg = create_chat_message(clawdius_core::llm::ChatRole::System, "You are helpful");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("system"));
        assert!(json.contains("You are helpful"));
    }

    #[test]
    fn test_provider_token_counting() {
        let config = clawdius_core::llm::LlmConfig {
            provider: "anthropic".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            api_key: Some("test-key".to_string()),
            base_url: None,
            max_tokens: 4096,
        };

        let provider = clawdius_core::llm::create_provider(&config).unwrap();
        assert_eq!(provider.count_tokens("hello world"), 2);
        assert_eq!(provider.count_tokens("one two three four five"), 5);
    }

    #[test]
    fn test_provider_enum_serialization() {
        let provider = clawdius_core::llm::Provider::Anthropic;
        let json = serde_json::to_string(&provider).unwrap();
        assert_eq!(json, "\"anthropic\"");

        let provider = clawdius_core::llm::Provider::OpenAI;
        let json = serde_json::to_string(&provider).unwrap();
        assert_eq!(json, "\"openai\"");

        let provider = clawdius_core::llm::Provider::Ollama;
        let json = serde_json::to_string(&provider).unwrap();
        assert_eq!(json, "\"ollama\"");
    }
}

mod retry_logic_tests {
    use super::*;

    #[tokio::test]
    async fn test_retry_success_on_first_attempt() {
        let call_count = Arc::new(AtomicU32::new(0));
        let count_clone = call_count.clone();

        let result = with_retry_and_circuit(3, 10, 100, 2.0, None, || {
            let count = count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Ok::<String, Error>("success".to_string())
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_on_rate_limit() {
        let call_count = Arc::new(AtomicU32::new(0));
        let count_clone = call_count.clone();

        let result = with_retry_and_circuit(3, 1, 10, 2.0, None, || {
            let count = count_clone.clone();
            async move {
                let c = count.fetch_add(1, Ordering::SeqCst);
                if c < 2 {
                    Err(Error::RateLimited { retry_after_ms: 1 })
                } else {
                    Ok::<String, Error>("success after retry".to_string())
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success after retry");
        assert!(call_count.load(Ordering::SeqCst) >= 2);
    }

    #[tokio::test]
    async fn test_retry_on_timeout() {
        let call_count = Arc::new(AtomicU32::new(0));
        let count_clone = call_count.clone();

        let result = with_retry_and_circuit(3, 1, 10, 2.0, None, || {
            let count = count_clone.clone();
            async move {
                let c = count.fetch_add(1, Ordering::SeqCst);
                if c < 2 {
                    Err(Error::Timeout(Duration::from_millis(1)))
                } else {
                    Ok::<String, Error>("success after timeout".to_string())
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert!(call_count.load(Ordering::SeqCst) >= 2);
    }

    #[tokio::test]
    async fn test_max_retries_exceeded() {
        let call_count = Arc::new(AtomicU32::new(0));
        let count_clone = call_count.clone();

        let result: Result<String, Error> = with_retry_and_circuit(2, 1, 10, 2.0, None, || {
            let count = count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Err(Error::RateLimited { retry_after_ms: 1 })
            }
        })
        .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::RateLimited { .. }));
        assert!(call_count.load(Ordering::SeqCst) > 2);
    }

    #[tokio::test]
    async fn test_non_retryable_error_no_retry() {
        let call_count = Arc::new(AtomicU32::new(0));
        let count_clone = call_count.clone();

        let result: Result<String, Error> = with_retry_and_circuit(3, 10, 100, 2.0, None, || {
            let count = count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Err(Error::Config("Invalid configuration".to_string()))
            }
        })
        .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Config { .. }));
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_with_circuit_breaker() {
        let circuit_breaker = CircuitBreaker::new("test-service", 3, 1000);
        let call_count = Arc::new(AtomicU32::new(0));
        let count_clone = call_count.clone();

        let result = with_retry_and_circuit(3, 1, 10, 2.0, Some(&circuit_breaker), || {
            let count = count_clone.clone();
            async move {
                let c = count.fetch_add(1, Ordering::SeqCst);
                if c < 1 {
                    Err(Error::RateLimited { retry_after_ms: 1 })
                } else {
                    Ok::<String, Error>("success".to_string())
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(circuit_breaker.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_exponential_backoff() {
        let call_count = Arc::new(AtomicU32::new(0));
        let count_clone = call_count.clone();
        let start = std::time::Instant::now();

        let result: Result<String, Error> = with_retry_and_circuit(3, 5, 100, 2.0, None, || {
            let count = count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Err(Error::RateLimited { retry_after_ms: 5 })
            }
        })
        .await;

        let elapsed = start.elapsed();
        assert!(result.is_err());
        assert!(elapsed.as_millis() >= 15);
    }

    #[tokio::test]
    async fn test_llm_with_retry_function() {
        use clawdius_core::config::RetryConfig;
        use clawdius_core::llm::{with_retry, RetryCondition};

        let call_count = Arc::new(AtomicU32::new(0));
        let count_clone = call_count.clone();

        let retry_config = RetryConfig {
            max_retries: 3,
            initial_delay_ms: 1,
            max_delay_ms: 10,
            exponential_base: 2.0,
            retry_on: vec![RetryCondition::RateLimit, RetryCondition::Timeout],
        };

        let result = with_retry(&retry_config, || {
            let count = count_clone.clone();
            async move {
                let c = count.fetch_add(1, Ordering::SeqCst);
                if c < 2 {
                    Err(Error::RateLimited { retry_after_ms: 1 })
                } else {
                    Ok::<String, Error>("success".to_string())
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert!(call_count.load(Ordering::SeqCst) >= 2);
    }
}

mod circuit_breaker_tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_starts_closed() {
        let cb = CircuitBreaker::new("test-service", 3, 1000);
        assert_eq!(cb.state().await, CircuitState::Closed);
        assert!(cb.is_allowed().await.is_ok());
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_after_threshold() {
        let cb = CircuitBreaker::new("test-service", 2, 1000);

        cb.record_failure(&Error::Timeout(Duration::from_secs(1)))
            .await;
        assert_eq!(cb.state().await, CircuitState::Closed);

        cb.record_failure(&Error::Timeout(Duration::from_secs(1)))
            .await;
        assert_eq!(cb.state().await, CircuitState::Open);

        let result = cb.is_allowed().await;
        assert!(matches!(result, Err(Error::CircuitBreakerOpen { .. })));
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_recovery() {
        let cb = CircuitBreaker::new("test-service", 1, 50);

        cb.record_failure(&Error::Timeout(Duration::from_secs(1)))
            .await;
        assert_eq!(cb.state().await, CircuitState::Open);

        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(cb.is_allowed().await.is_ok());
        assert_eq!(cb.state().await, CircuitState::HalfOpen);

        cb.record_success().await;
        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_resets_on_success() {
        let cb = CircuitBreaker::new("test-service", 3, 1000);

        cb.record_failure(&Error::Timeout(Duration::from_secs(1)))
            .await;
        cb.record_failure(&Error::Timeout(Duration::from_secs(1)))
            .await;
        assert_eq!(cb.state().await, CircuitState::Closed);

        cb.record_success().await;
        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_blocks_requests_when_open() {
        let cb = CircuitBreaker::new("test-service", 1, 500);

        cb.record_failure(&Error::Timeout(Duration::from_secs(1)))
            .await;
        assert_eq!(cb.state().await, CircuitState::Open);

        let result = cb.is_allowed().await;
        assert!(result.is_err());
        if let Err(Error::CircuitBreakerOpen { service, .. }) = result {
            assert_eq!(service, "test-service");
        } else {
            panic!("Expected CircuitBreakerOpen error");
        }
    }
}

mod error_handling_tests {
    use super::*;

    #[test]
    fn test_invalid_api_key_error() {
        let error = Error::Config("ANTHROPIC_API_KEY not set".to_string());
        assert!(!error.is_retryable());
        assert!(error.to_string().contains("Configuration error"));
    }

    #[test]
    fn test_llm_provider_error() {
        let error = Error::LlmProvider {
            message: "Invalid API key".to_string(),
            provider: "anthropic".to_string(),
        };
        assert!(!error.is_retryable());
        assert!(error.to_string().contains("anthropic"));
        assert!(error.to_string().contains("Invalid API key"));
    }

    #[test]
    fn test_network_error_handling() {
        let timeout_error = Error::Timeout(Duration::from_secs(30));
        assert!(timeout_error.is_retryable());
        assert!(timeout_error.is_timeout());
        assert_eq!(timeout_error.retry_after_ms(), Some(30000));
    }

    #[test]
    fn test_rate_limit_error() {
        let error = Error::RateLimited {
            retry_after_ms: 5000,
        };
        assert!(error.is_retryable());
        assert!(error.is_rate_limited());
        assert_eq!(error.retry_after_ms(), Some(5000));
    }

    #[test]
    fn test_context_limit_error() {
        let error = Error::ContextLimit {
            current: 100000,
            limit: 80000,
        };
        assert!(!error.is_retryable());
        assert!(error.to_string().contains("100000"));
        assert!(error.to_string().contains("80000"));
    }

    #[test]
    fn test_circuit_breaker_error() {
        let error = Error::CircuitBreakerOpen {
            service: "llm-provider".to_string(),
            last_error: "Connection refused".to_string(),
        };
        assert!(error.is_retryable());
        assert!(error.is_circuit_breaker());
    }

    #[test]
    fn test_user_message_for_api_key_error() {
        let error = Error::Config("ANTHROPIC_API_KEY not set".to_string());
        let message = error.user_message();
        assert!(message.contains("ANTHROPIC_API_KEY"));
        assert!(message.contains("export"));
    }

    #[test]
    fn test_user_message_for_openai_key_error() {
        let error = Error::Config("OPENAI_API_KEY not set".to_string());
        let message = error.user_message();
        assert!(message.contains("OPENAI_API_KEY"));
        assert!(message.contains("export"));
    }

    #[test]
    fn test_user_message_for_rate_limit() {
        let error = Error::RateLimited {
            retry_after_ms: 5000,
        };
        let message = error.user_message();
        assert!(message.contains("5 seconds"));
    }

    #[test]
    fn test_user_message_for_context_limit() {
        let error = Error::ContextLimit {
            current: 100000,
            limit: 80000,
        };
        let message = error.user_message();
        assert!(message.contains("compact"));
    }

    #[test]
    fn test_user_message_for_session_not_found() {
        let error = Error::SessionNotFound {
            id: "session-123".to_string(),
        };
        let message = error.user_message();
        assert!(message.contains("session-123"));
        assert!(message.contains("clawdius sessions"));
    }

    #[test]
    fn test_error_from_io() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error: Error = io_error.into();
        assert!(matches!(error, Error::Io(_)));
    }

    #[test]
    fn test_error_from_serde_json() {
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json");
        let error: Error = json_error.unwrap_err().into();
        assert!(matches!(error, Error::Serialization(_)));
    }

    #[test]
    fn test_retry_exhausted_error() {
        let error = Error::RetryExhausted(5);
        assert!(error.to_string().contains('5'));
        assert!(error.to_string().contains("Retry exhausted"));
    }

    #[test]
    fn test_enhanced_error_conversion() {
        let error = Error::SessionNotFound {
            id: "test".to_string(),
        };
        let enhanced = error.into_enhanced();
        assert!(!enhanced.message().is_empty());
    }
}

mod mock_llm_client_tests {
    use super::*;
    use async_trait::async_trait;
    use clawdius_core::llm::{ChatMessage, ChatRole, LlmClient};
    use std::sync::Arc;
    use tokio::sync::mpsc;

    enum MockResponse {
        Success(String),
        RateLimited { retry_after_ms: u64 },
        Timeout(Duration),
        ConfigError(String),
    }

    struct MockLlmClient {
        response: MockResponse,
        token_count: usize,
    }

    impl MockLlmClient {
        fn new(response: MockResponse) -> Self {
            Self {
                response,
                token_count: 0,
            }
        }

        fn with_token_count(mut self, count: usize) -> Self {
            self.token_count = count;
            self
        }

        fn get_result(&self) -> clawdius_core::Result<String> {
            match &self.response {
                MockResponse::Success(text) => Ok(text.clone()),
                MockResponse::RateLimited { retry_after_ms } => Err(Error::RateLimited {
                    retry_after_ms: *retry_after_ms,
                }),
                MockResponse::Timeout(d) => Err(Error::Timeout(*d)),
                MockResponse::ConfigError(msg) => Err(Error::Config(msg.clone())),
            }
        }
    }

    #[async_trait]
    impl LlmClient for MockLlmClient {
        async fn chat(&self, _messages: Vec<ChatMessage>) -> clawdius_core::Result<String> {
            self.get_result()
        }

        async fn chat_stream(
            &self,
            _messages: Vec<ChatMessage>,
        ) -> clawdius_core::Result<mpsc::Receiver<String>> {
            let (tx, rx) = mpsc::channel(100);
            match &self.response {
                MockResponse::Success(text) => {
                    let text = text.clone();
                    tokio::spawn(async move {
                        for word in text.split_whitespace() {
                            if tx.send(word.to_string()).await.is_err() {
                                break;
                            }
                        }
                    });
                }
                MockResponse::RateLimited { retry_after_ms } => {
                    let retry_after_ms = *retry_after_ms;
                    tokio::spawn(async move {
                        let _ = tx
                            .send(format!(
                                "[Error: Rate limited, retry after {retry_after_ms}ms]"
                            ))
                            .await;
                    });
                }
                MockResponse::Timeout(d) => {
                    let d = *d;
                    tokio::spawn(async move {
                        let _ = tx.send(format!("[Error: Timeout after {d:?}]")).await;
                    });
                }
                MockResponse::ConfigError(msg) => {
                    let msg = msg.clone();
                    tokio::spawn(async move {
                        let _ = tx.send(format!("[Error: {msg}]")).await;
                    });
                }
            }
            Ok(rx)
        }

        fn count_tokens(&self, text: &str) -> usize {
            if self.token_count > 0 {
                self.token_count
            } else {
                text.split_whitespace().count()
            }
        }
    }

    #[tokio::test]
    async fn test_mock_client_successful_chat() {
        let client = MockLlmClient::new(MockResponse::Success(
            "Hello, I am an AI assistant.".to_string(),
        ));
        let messages = vec![ChatMessage {
            role: ChatRole::User,
            content: "Hello".to_string(),
        }];

        let result = client.chat(messages).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Hello"));
    }

    #[tokio::test]
    async fn test_mock_client_rate_limit_error() {
        let client = MockLlmClient::new(MockResponse::RateLimited {
            retry_after_ms: 1000,
        });
        let messages = vec![ChatMessage {
            role: ChatRole::User,
            content: "Hello".to_string(),
        }];

        let result = client.chat(messages).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::RateLimited { .. }));
    }

    #[tokio::test]
    async fn test_mock_client_timeout_error() {
        let client = MockLlmClient::new(MockResponse::Timeout(Duration::from_secs(30)));
        let messages = vec![ChatMessage {
            role: ChatRole::User,
            content: "Hello".to_string(),
        }];

        let result = client.chat(messages).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_timeout());
        assert!(err.is_retryable());
    }

    #[tokio::test]
    async fn test_mock_client_config_error_not_retryable() {
        let client = MockLlmClient::new(MockResponse::ConfigError("API key missing".to_string()));
        let messages = vec![ChatMessage {
            role: ChatRole::User,
            content: "Hello".to_string(),
        }];

        let result = client.chat(messages).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(!err.is_retryable());
    }

    #[tokio::test]
    async fn test_mock_client_token_counting() {
        let client =
            MockLlmClient::new(MockResponse::Success("response".to_string())).with_token_count(42);
        assert_eq!(client.count_tokens("any text"), 42);
    }

    #[tokio::test]
    async fn test_mock_client_default_token_counting() {
        let client = MockLlmClient::new(MockResponse::Success("response".to_string()));
        assert_eq!(client.count_tokens("one two three"), 3);
    }

    #[tokio::test]
    async fn test_mock_client_streaming() {
        let client = MockLlmClient::new(MockResponse::Success("Hello world from AI".to_string()));
        let messages = vec![ChatMessage {
            role: ChatRole::User,
            content: "Hello".to_string(),
        }];

        let mut rx = client.chat_stream(messages).await.unwrap();
        let mut received = Vec::new();
        while let Some(chunk) = rx.recv().await {
            received.push(chunk);
        }

        assert!(!received.is_empty());
    }

    #[tokio::test]
    async fn test_mock_client_streaming_error() {
        let client = MockLlmClient::new(MockResponse::ConfigError("Test error".to_string()));
        let messages = vec![ChatMessage {
            role: ChatRole::User,
            content: "Hello".to_string(),
        }];

        let mut rx = client.chat_stream(messages).await.unwrap();
        let mut received = String::new();
        while let Some(chunk) = rx.recv().await {
            received.push_str(&chunk);
        }

        assert!(received.contains("Error"));
    }

    #[tokio::test]
    async fn test_mock_client_with_retry_wrapper() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let count_clone = attempt_count.clone();

        let result = with_retry_and_circuit(3, 1, 10, 2.0, None, || {
            let count = count_clone.clone();
            async move {
                let c = count.fetch_add(1, Ordering::SeqCst);
                if c < 2 {
                    let client =
                        MockLlmClient::new(MockResponse::RateLimited { retry_after_ms: 1 });
                    client
                        .chat(vec![ChatMessage {
                            role: ChatRole::User,
                            content: "test".to_string(),
                        }])
                        .await
                } else {
                    let client = MockLlmClient::new(MockResponse::Success(
                        "Success after retry".to_string(),
                    ));
                    client
                        .chat(vec![ChatMessage {
                            role: ChatRole::User,
                            content: "test".to_string(),
                        }])
                        .await
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert!(attempt_count.load(Ordering::SeqCst) >= 2);
    }
}

mod llm_config_tests {
    #[test]
    fn test_llm_config_serialization() {
        let config = clawdius_core::llm::LlmConfig {
            provider: "anthropic".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            api_key: Some("secret-key".to_string()),
            base_url: None,
            max_tokens: 4096,
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("anthropic"));
        assert!(json.contains("claude-3-5-sonnet"));
    }

    #[test]
    fn test_llm_config_deserialization() {
        let json = r#"{
            "provider": "openai",
            "model": "gpt-4o",
            "api_key": "test-key",
            "max_tokens": 8192
        }"#;

        let config: clawdius_core::llm::LlmConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.provider, "openai");
        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.max_tokens, 8192);
    }

    #[test]
    fn test_retry_config_default() {
        use clawdius_core::config::RetryConfig;

        let config = RetryConfig::default();
        assert!(config.max_retries > 0);
        assert!(config.initial_delay_ms > 0);
        assert!(config.max_delay_ms > config.initial_delay_ms);
        assert!(config.exponential_base > 1.0);
    }

    #[test]
    fn test_create_provider_with_retry() {
        let config = clawdius_core::llm::LlmConfig {
            provider: "anthropic".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            api_key: Some("test-key".to_string()),
            base_url: None,
            max_tokens: 4096,
        };

        let client = clawdius_core::llm::create_provider_with_retry(&config, None);
        assert!(client.is_ok());
    }

    #[test]
    fn test_create_provider_with_custom_retry() {
        use clawdius_core::config::{RetryCondition, RetryConfig};

        let config = clawdius_core::llm::LlmConfig {
            provider: "anthropic".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            api_key: Some("test-key".to_string()),
            base_url: None,
            max_tokens: 4096,
        };

        let retry_config = RetryConfig {
            max_retries: 5,
            initial_delay_ms: 100,
            max_delay_ms: 10000,
            exponential_base: 2.0,
            retry_on: vec![
                RetryCondition::RateLimit,
                RetryCondition::Timeout,
                RetryCondition::ServerError,
            ],
        };

        let client = clawdius_core::llm::create_provider_with_retry(&config, Some(retry_config));
        assert!(client.is_ok());
    }
}
