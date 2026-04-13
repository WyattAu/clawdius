use clawdius_core::llm::providers::anthropic::AnthropicProvider;
use clawdius_core::llm::providers::LlmClient;
use clawdius_core::llm::{ChatMessage, ChatRole};

#[tokio::test]
#[ignore]
async fn test_anthropic_provider_real_api() {
    let api_key = match std::env::var("ANTHROPIC_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!("Skipping: ANTHROPIC_API_KEY not set");
            return;
        },
    };

    let provider = AnthropicProvider::new(&api_key, Some("claude-sonnet-4-20250514"))
        .expect("Failed to create AnthropicProvider");

    let messages = vec![ChatMessage {
        role: ChatRole::User,
        content: "Say hello in exactly 3 words".to_string(),
    }];

    let response = provider.chat(messages).await.expect("Chat request failed");
    assert!(!response.trim().is_empty(), "Response should not be empty");

    let word_count = response.split_whitespace().count();
    assert!(
        word_count >= 2,
        "Response should contain at least 2 words, got: {response}"
    );

    let token_count = provider.count_tokens(&response);
    assert!(token_count > 0, "Token count should be greater than 0");
}
