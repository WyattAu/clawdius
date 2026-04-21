//! Live LLM model test via OpenRouter
//!
//! Tests chat and tool_use across multiple models.
//! Run with: cargo test -p clawdius-core --test test_llm_models -- --nocapture

use clawdius_core::llm::*;
use clawdius_core::llm::providers::{ChatWithToolsResult, Tool};
use serde_json::json;

const API_KEY: &str = "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";

fn make_config(model: &str) -> LlmConfig {
    LlmConfig {
        provider: "openrouter".to_string(),
        model: model.to_string(),
        api_key: Some(API_KEY.to_string()),
        base_url: None,
        max_tokens: 150,
    }
}

fn weather_tool() -> Tool {
    Tool::new("get_weather")
        .with_description("Get the current weather for a location")
        .with_schema(json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The city name"
                }
            },
            "required": ["location"]
        }))
}

async fn test_chat(model: &str) -> Result<String, String> {
    let config = make_config(model);
    let provider = create_provider(&config).map_err(|e| format!("create_provider: {e}"))?;
    let messages = vec![
        ChatMessage {
            role: ChatRole::System,
            content: "Reply in one short sentence.".to_string(),
        },
        ChatMessage {
            role: ChatRole::User,
            content: "What is 2+2? Reply with just the number.".to_string(),
        },
    ];
    let result = tokio::time::timeout(std::time::Duration::from_secs(30), provider.chat(messages))
        .await
        .map_err(|e| format!("timeout: {e}"))?
        .map_err(|e| format!("chat error: {e}"))?;
    Ok(result)
}

async fn test_tool_call(model: &str) -> Result<ChatWithToolsResult, String> {
    let config = make_config(model);
    let provider = create_provider(&config).map_err(|e| format!("create_provider: {e}"))?;

    let tools = vec![weather_tool()];

    let messages = vec![ChatMessage {
        role: ChatRole::User,
        content: "What's the weather in Tokyo?".to_string(),
    }];

    let result =
        tokio::time::timeout(std::time::Duration::from_secs(30), provider.chat_with_tools(messages, tools))
            .await
            .map_err(|e| format!("timeout: {e}"))?
            .map_err(|e| format!("chat_with_tools error: {e}"))?;
    Ok(result)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max])
    } else {
        s.to_string()
    }
}

// ── Free Models ────────────────────────────────────────────────────────

#[tokio::test]
async fn free_gemma_3_4b() {
    let model = "google/gemma-3-27b-it:free";
    println!("\n=== {model} ===");
    match test_chat(model).await {
        Ok(resp) => println!("  Chat: {}", truncate(&resp, 120)),
        Err(e) => println!("  Chat FAIL: {e}"),
    }
    match test_tool_call(model).await {
        Ok(r) => println!(
            "  Tools: text={} tool_calls={}",
            truncate(&r.text, 80),
            r.tool_calls.len()
        ),
        Err(e) => println!("  Tools FAIL: {e}"),
    }
}

#[tokio::test]
async fn free_llama_4_maverick() {
    let model = "openai/gpt-oss-20b:free";
    println!("\n=== {model} ===");
    match test_chat(model).await {
        Ok(resp) => println!("  Chat: {}", truncate(&resp, 120)),
        Err(e) => println!("  Chat FAIL: {e}"),
    }
    match test_tool_call(model).await {
        Ok(r) => println!(
            "  Tools: text={} tool_calls={}",
            truncate(&r.text, 80),
            r.tool_calls.len()
        ),
        Err(e) => println!("  Tools FAIL: {e}"),
    }
}

#[tokio::test]
async fn free_mistral_small_24b() {
    let model = "minimax/minimax-m2.5:free";
    println!("\n=== {model} ===");
    match test_chat(model).await {
        Ok(resp) => println!("  Chat: {}", truncate(&resp, 120)),
        Err(e) => println!("  Chat FAIL: {e}"),
    }
    match test_tool_call(model).await {
        Ok(r) => println!(
            "  Tools: text={} tool_calls={}",
            truncate(&r.text, 80),
            r.tool_calls.len()
        ),
        Err(e) => println!("  Tools FAIL: {e}"),
    }
}

#[tokio::test]
async fn free_deepseek_r1_05() {
    let model = "z-ai/glm-4.5-air:free";
    println!("\n=== {model} ===");
    match test_chat(model).await {
        Ok(resp) => println!("  Chat: {}", truncate(&resp, 200)),
        Err(e) => println!("  Chat FAIL: {e}"),
    }
    // z-ai/glm doesn't support tool calling via OpenRouter
}

#[tokio::test]
async fn free_qwen_3_8b() {
    let model = "nvidia/nemotron-3-nano-30b-a3b:free";
    println!("\n=== {model} ===");
    match test_chat(model).await {
        Ok(resp) => println!("  Chat: {}", truncate(&resp, 200)),
        Err(e) => println!("  Chat FAIL: {e}"),
    }
    match test_tool_call(model).await {
        Ok(r) => println!(
            "  Tools: text={} tool_calls={}",
            truncate(&r.text, 80),
            r.tool_calls.len()
        ),
        Err(e) => println!("  Tools FAIL: {e}"),
    }
}

// ── Claude Models ─────────────────────────────────────────────────────
// NOTE: These require OpenRouter credits (402 if insufficient)

#[tokio::test]
#[ignore] // Requires OpenRouter credits
async fn claude_3_5_haiku() {
    let model = "anthropic/claude-3.5-haiku";
    println!("\n=== {model} ===");
    match test_chat(model).await {
        Ok(resp) => println!("  Chat: {}", truncate(&resp, 120)),
        Err(e) => println!("  Chat FAIL: {e}"),
    }
    match test_tool_call(model).await {
        Ok(r) => {
            println!(
                "  Tools: text={} tool_calls={}",
                truncate(&r.text, 80),
                r.tool_calls.len()
            );
            for tc in &r.tool_calls {
                println!("    → fn={} args={}", tc.fn_name, tc.fn_arguments);
            }
        },
        Err(e) => println!("  Tools FAIL: {e}"),
    }
}

#[tokio::test]
#[ignore] // Requires OpenRouter credits
async fn claude_sonnet_4() {
    let model = "anthropic/claude-sonnet-4";
    println!("\n=== {model} ===");
    match test_chat(model).await {
        Ok(resp) => println!("  Chat: {}", truncate(&resp, 120)),
        Err(e) => println!("  Chat FAIL: {e}"),
    }
    match test_tool_call(model).await {
        Ok(r) => {
            println!(
                "  Tools: text={} tool_calls={}",
                truncate(&r.text, 80),
                r.tool_calls.len()
            );
            for tc in &r.tool_calls {
                println!("    → fn={} args={}", tc.fn_name, tc.fn_arguments);
            }
        },
        Err(e) => println!("  Tools FAIL: {e}"),
    }
}

// ── GPT Models ────────────────────────────────────────────────────────
// NOTE: These require OpenRouter credits (402 if insufficient)

#[tokio::test]
#[ignore] // Requires OpenRouter credits
async fn gpt_4o_mini() {
    let model = "openai/gpt-4o-mini";
    println!("\n=== {model} ===");
    match test_chat(model).await {
        Ok(resp) => println!("  Chat: {}", truncate(&resp, 120)),
        Err(e) => println!("  Chat FAIL: {e}"),
    }
    match test_tool_call(model).await {
        Ok(r) => {
            println!(
                "  Tools: text={} tool_calls={}",
                truncate(&r.text, 80),
                r.tool_calls.len()
            );
            for tc in &r.tool_calls {
                println!("    → fn={} args={}", tc.fn_name, tc.fn_arguments);
            }
        },
        Err(e) => println!("  Tools FAIL: {e}"),
    }
}

#[tokio::test]
#[ignore] // Requires OpenRouter credits
async fn gpt_4o() {
    let model = "openai/gpt-4o";
    println!("\n=== {model} ===");
    match test_chat(model).await {
        Ok(resp) => println!("  Chat: {}", truncate(&resp, 120)),
        Err(e) => println!("  Chat FAIL: {e}"),
    }
    match test_tool_call(model).await {
        Ok(r) => {
            println!(
                "  Tools: text={} tool_calls={}",
                truncate(&r.text, 80),
                r.tool_calls.len()
            );
            for tc in &r.tool_calls {
                println!("    → fn={} args={}", tc.fn_name, tc.fn_arguments);
            }
        },
        Err(e) => println!("  Tools FAIL: {e}"),
    }
}

// ── Other Notable Models ──────────────────────────────────────────────

#[tokio::test]
#[ignore] // Requires OpenRouter credits
async fn gemini_2_5_flash() {
    let model = "google/gemini-2.5-flash-preview";
    println!("\n=== {model} ===");
    match test_chat(model).await {
        Ok(resp) => println!("  Chat: {}", truncate(&resp, 200)),
        Err(e) => println!("  Chat FAIL: {e}"),
    }
    match test_tool_call(model).await {
        Ok(r) => {
            println!(
                "  Tools: text={} tool_calls={}",
                truncate(&r.text, 100),
                r.tool_calls.len()
            );
            for tc in &r.tool_calls {
                println!("    → fn={} args={}", tc.fn_name, tc.fn_arguments);
            }
        },
        Err(e) => println!("  Tools FAIL: {e}"),
    }
}

#[tokio::test]
async fn deepseek_v3_chat() {
    let model = "qwen/qwen3-next-80b-a3b-instruct:free";
    println!("\n=== {model} ===");
    match test_chat(model).await {
        Ok(resp) => println!("  Chat: {}", truncate(&resp, 200)),
        Err(e) => println!("  Chat FAIL: {e}"),
    }
    match test_tool_call(model).await {
        Ok(r) => println!(
            "  Tools: text={} tool_calls={}",
            truncate(&r.text, 80),
            r.tool_calls.len()
        ),
        Err(e) => println!("  Tools FAIL: {e}"),
    }
}

#[tokio::test]
async fn free_gemma_4_31b() {
    let model = "arcee-ai/trinity-large-preview:free";
    println!("\n=== {model} ===");
    match test_chat(model).await {
        Ok(resp) => println!("  Chat: {}", truncate(&resp, 200)),
        Err(e) => println!("  Chat FAIL: {e}"),
    }
    match test_tool_call(model).await {
        Ok(r) => println!(
            "  Tools: text={} tool_calls={}",
            truncate(&r.text, 80),
            r.tool_calls.len()
        ),
        Err(e) => println!("  Tools FAIL: {e}"),
    }
}

#[tokio::test]
async fn free_qwen3_coder() {
    let model = "openai/gpt-oss-120b:free";
    println!("\n=== {model} ===");
    match test_chat(model).await {
        Ok(resp) => println!("  Chat: {}", truncate(&resp, 200)),
        Err(e) => println!("  Chat FAIL: {e}"),
    }
    match test_tool_call(model).await {
        Ok(r) => println!(
            "  Tools: text={} tool_calls={}",
            truncate(&r.text, 80),
            r.tool_calls.len()
        ),
        Err(e) => println!("  Tools FAIL: {e}"),
    }
}
