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
use clawdius_core::llm::providers::anthropic::AnthropicProvider;
use clawdius_core::llm::providers::LlmClient;
use clawdius_core::llm::{ChatMessage, ChatRole};

#[tokio::test]
#[ignore = "Requires live Anthropic API key"]
async fn test_anthropic_provider_real_api() {
    let api_key =
        std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY must be set for this test");

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
