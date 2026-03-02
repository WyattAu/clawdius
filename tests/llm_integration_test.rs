//! LLM Integration Test
//!
//! Verifies connectivity to LLM provider APIs.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::uninlined_format_args)]

use std::env;

fn load_env() {
    let _ = dotenvy::dotenv();
}

#[tokio::test]
async fn test_zai_api_key_present() {
    load_env();
    let api_key = env::var("ZAI_API_KEY").expect("ZAI_API_KEY must be set");
    assert!(!api_key.is_empty(), "ZAI_API_KEY should not be empty");
    assert!(api_key.len() > 20, "ZAI_API_KEY appears too short");
}

#[tokio::test]
async fn test_zai_list_models() {
    load_env();
    let api_key = env::var("ZAI_API_KEY").expect("ZAI_API_KEY must be set");
    
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.z.ai/api/coding/paas/v4/models")
        .bearer_auth(&api_key)
        .send()
        .await
        .expect("Failed to send request");
    
    assert!(response.status().is_success(), "API request failed: {}", response.status());
    
    let body = response.text().await.expect("Failed to read response");
    assert!(body.contains("\"object\":\"list\""), "Response should contain model list");
}

#[tokio::test]
async fn test_zai_chat_completion() {
    load_env();
    let api_key = env::var("ZAI_API_KEY").expect("ZAI_API_KEY must be set");
    
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": "glm-4.5",
        "messages": [{"role": "user", "content": "Say hello"}],
        "max_tokens": 10
    });
    
    let response = client
        .post("https://api.z.ai/api/coding/paas/v4/chat/completions")
        .bearer_auth(&api_key)
        .json(&body)
        .send()
        .await
        .expect("Failed to send request");
    
    let status = response.status();
    let response_body = response.text().await.expect("Failed to read response");
    
    assert!(
        status.is_success() || response_body.contains("余额不足"),
        "Unexpected response: {} - {}",
        status,
        response_body
    );
}
