use crate::llm::{ChatMessage, ChatRole, LlmResponse, StreamEvent};
use crate::mcp::McpRequest;
use crate::session::types::TokenUsage;
use crate::Result;
use serde::Serialize;
use std::borrow::Cow;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize)]
pub struct ToolCallResult {
    pub name: Cow<'static, str>,
    pub arguments: serde_json::Value,
    /// `Arc<str>` avoids cloning the (potentially 100 kB) tool result for each
    /// `AgentEvent::ToolResult` dispatch — one allocation, then refcount bumps.
    pub result: Arc<str>,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize)]
pub enum AgentTerminationReason {
    Completed,
    MaxIterations { iterations: usize },
    TokenBudgetExhausted { tokens_used: usize, budget: usize },
    TimeBudgetExhausted { elapsed: Duration, budget: Duration },
    Error(Cow<'static, str>),
}

#[derive(Debug, Clone)]
pub struct AgentLoopResult {
    /// `Arc<str>` so `Done` event can share the final text without cloning.
    pub text: Arc<str>,
    pub tool_calls: Vec<ToolCallResult>,
    pub total_usage: TokenUsage,
    pub iterations: usize,
    pub termination_reason: AgentTerminationReason,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    Iteration {
        number: usize,
        max: usize,
    },
    Thinking,
    Chunk(Arc<str>),
    ToolCall {
        name: Cow<'static, str>,
        arguments: serde_json::Value,
    },
    ToolResult {
        name: Cow<'static, str>,
        result: Arc<str>,
        is_error: bool,
    },
    Done {
        text: Arc<str>,
    },
    Error {
        message: Cow<'static, str>,
    },
}

#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    pub max_iterations: usize,
    pub max_total_tokens: usize,
    pub max_wall_time: Duration,
    pub max_tokens_per_iteration: usize,
    pub max_tool_result_size: usize,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            max_total_tokens: 200_000,
            max_wall_time: Duration::from_secs(300),
            max_tokens_per_iteration: 50_000,
            max_tool_result_size: 102_400,
        }
    }
}

const SYSTEM_PROMPT: &str =
    "You are a coding agent with access to tools. When you need to read files, run tests, \
     search code, or edit code, use the appropriate tool. Think step-by-step.\n\n\
     Available tools: read_file, list_directory, write_file, edit_file, codebase_search, \
     git_status, git_log, git_diff, check_build, run_tests, web_search, generate_code, \
     codebase_index, codebase_text_search\n\n\
     CRITICAL FILE PROTECTION RULES (these are enforced at the system level — violations will \
     be rejected automatically):\n\
     - NEVER use write_file to overwrite src/lib.rs, src/main.rs, or any mod.rs file with \
       a short cargo-init-style stub (a file containing only a single pub fn and no modules). \
     - NEVER replace a file that contains many module declarations with content that has \
       significantly fewer module declarations. This destroys the crate's public API.\n\
     - If you need to add a new module, use edit_file to append a new `pub mod` line to the \
       existing lib.rs, or ask the user to do it.\n\n\
     When native tool calling is available, use the provider's structured tool_use format. \
     Otherwise, use one of these text formats on a line by itself:\n\
     1. [TOOL_CALL] {\"name\": \"tool_name\", \"arguments\": {\"param\": \"value\"}} [/TOOL_CALL]\n\
     2. ant:invoke:tool_name{\"param\": \"value\"}ant:invoke:end\n\n\
     Always include the tool call on its own line. After receiving the tool result, you can \
     call another tool or provide your final answer.";

pub struct AgentLoop {
    llm_client: Arc<dyn crate::llm::providers::LlmClient>,
    config: AgentLoopConfig,
    /// External MCP client manager for tool dispatch.
    /// When set, tool calls are first routed to external MCP servers.
    /// If no external server handles the tool, falls back to built-in tools.
    #[cfg(not(target_arch = "wasm32"))]
    mcp_manager: Option<Arc<crate::mcp::ClientManager>>,
}

#[derive(Debug, Clone)]
struct ParsedToolCall {
    name: Cow<'static, str>,
    arguments: serde_json::Value,
}

impl AgentLoop {
    pub fn new(llm_client: Arc<dyn crate::llm::providers::LlmClient>) -> Self {
        Self {
            llm_client,
            config: AgentLoopConfig::default(),
            #[cfg(not(target_arch = "wasm32"))]
            mcp_manager: None,
        }
    }

    #[must_use]
    pub const fn with_max_iterations(mut self, max: usize) -> Self {
        self.config.max_iterations = max;
        self
    }

    #[must_use]
    pub const fn with_config(mut self, config: AgentLoopConfig) -> Self {
        self.config = config;
        self
    }

    /// Attach an external MCP client manager for tool dispatch.
    ///
    /// When set, `execute_single_tool` and `execute_concurrent_tools` will
    /// first check if any connected MCP server provides the requested tool.
    /// If found, the call is routed to the external server. Otherwise, the
    /// built-in tool handler is used as fallback.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn with_mcp_manager(mut self, manager: Arc<crate::mcp::ClientManager>) -> Self {
        self.mcp_manager = Some(manager);
        self
    }

    pub async fn run(
        &self,
        messages: Vec<ChatMessage>,
        event_tx: Option<mpsc::Sender<AgentEvent>>,
    ) -> Result<AgentLoopResult> {
        let start = Instant::now();
        let timeout_result = tokio::time::timeout(
            self.config.max_wall_time,
            self.run_inner(messages, event_tx),
        )
        .await;

        if let Ok(result) = timeout_result { result } else {
            let elapsed = start.elapsed();
            Ok(AgentLoopResult {
                text: Arc::from("(Time budget exhausted)"),
                tool_calls: vec![],
                total_usage: TokenUsage::default(),
                iterations: 0,
                termination_reason: AgentTerminationReason::TimeBudgetExhausted {
                    elapsed,
                    budget: self.config.max_wall_time,
                },
            })
        }
    }

    /// Build a budget-exhausted result.
    fn budget_exhausted_result(
        &self,
        total_usage: &TokenUsage,
        iter_num: usize,
        all_tool_calls: &[ToolCallResult],
    ) -> AgentLoopResult {
        AgentLoopResult {
            text: format!(
                "(Token budget exhausted after {} tokens, budget: {})",
                total_usage.total(),
                self.config.max_total_tokens
            )
            .into(),
            tool_calls: all_tool_calls.to_vec(),
            total_usage: total_usage.clone(),
            iterations: iter_num,
            termination_reason: AgentTerminationReason::TokenBudgetExhausted {
                tokens_used: total_usage.total(),
                budget: self.config.max_total_tokens,
            },
        }
    }

    /// Execute a single tool call and emit events.
    async fn execute_single_tool(
        &self,
        tc: &ParsedToolCall,
        event_tx: &Option<mpsc::Sender<AgentEvent>>,
    ) -> (ToolCallResult, String) {
        if let Some(tx) = event_tx {
            tx.send(AgentEvent::ToolCall {
                    name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                })
                .await
                .ok();
        }

        // Try external MCP server first, fall back to built-in tools
        #[cfg(not(target_arch = "wasm32"))]
        let result = {
            if let Some(ref mgr) = self.mcp_manager {
                match Self::try_external_mcp(mgr, &tc.name, &tc.arguments).await {
                    Some(r) => r,
                    None => execute_mcp_tool_call(&tc.name, &tc.arguments),
                }
            } else {
                execute_mcp_tool_call(&tc.name, &tc.arguments)
            }
        };
        #[cfg(target_arch = "wasm32")]
        let result = execute_mcp_tool_call(&tc.name, &tc.arguments);
        let is_error = result.is_error;
        let truncated_result = truncate_result(&result.result, self.config.max_tool_result_size);

        // Record tool execution metrics
        clawdius_core::metrics::record_tool_execution(
            &tc.name,
            std::time::Duration::from_millis(0), // duration not tracked in sync path
            !is_error,
        );

        let tool_result = ToolCallResult {
            name: tc.name.clone(),
            arguments: tc.arguments.clone(),
            result: truncated_result.into(),
            is_error,
        };

        if let Some(tx) = event_tx {
            tx.send(AgentEvent::ToolResult {
                    name: tool_result.name.clone(),
                    result: tool_result.result.clone(),
                    is_error,
                })
                .await
                .ok();
        }

        let text = format!("Tool {} result:\n{}\n\n", tool_result.name, tool_result.result);
        (tool_result, text)
    }

    /// Try to dispatch a tool call to an external MCP server.
    ///
    /// Returns `Some(result)` if an external server handled the call,
    /// or `None` if no server provides the tool (caller should fall back).
    #[cfg(not(target_arch = "wasm32"))]
    async fn try_external_mcp(
        manager: &Arc<crate::mcp::ClientManager>,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Option<McpToolResultInner> {
        // Check if any connected server provides this tool
        let tools = manager.list_all_tools().await.ok()?;
        let provider = tools.iter().find(|(_, tool)| tool.name == tool_name);

        let (server_name, _) = provider?;

        // Dispatch to the external server
        match manager
            .call_tool(server_name, tool_name, arguments.clone())
            .await
        {
            Ok(result) => {
                let is_error = result.is_error;
                let text = result
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        crate::mcp::protocol::McpContent::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                Some(McpToolResultInner {
                    result: if text.is_empty() {
                        "Tool completed (non-text content)".to_string()
                    } else {
                        text
                    },
                    is_error,
                })
            }
            Err(e) => Some(McpToolResultInner {
                result: format!("External MCP error: {e}"),
                is_error: true,
            }),
        }
    }

    /// Execute multiple tool calls concurrently and emit events.
    async fn execute_concurrent_tools(
        &self,
        tool_calls: &[ParsedToolCall],
        event_tx: &Option<mpsc::Sender<AgentEvent>>,
    ) -> (Vec<ToolCallResult>, String) {
        let mut join_set = tokio::task::JoinSet::new();

        for tc in tool_calls {
            if let Some(tx) = event_tx {
                tx.send(AgentEvent::ToolCall {
                        name: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                    })
                    .await
                    .ok();
            }
            let name = tc.name.clone();
            let arguments = tc.arguments.clone();
            join_set.spawn_blocking(move || {
                let result = execute_mcp_tool_call(&name, &arguments);
                (name, result)
            });
        }

        // Collect results in deterministic order
        let mut ordered_results: Vec<Option<(Cow<'static, str>, McpToolResultInner)>> =
            (0..tool_calls.len()).map(|_| None).collect();

        while let Some(result) = join_set.join_next().await {
            match result {
                Ok((name, tool_result)) => {
                    let idx = tool_calls.iter().position(|tc| tc.name == name);
                    if let Some(idx) = idx {
                        let actual_idx = if ordered_results[idx].is_some() {
                            ordered_results
                                .iter()
                                .position(std::option::Option::is_none)
                                .unwrap_or(idx)
                        } else {
                            idx
                        };
                        ordered_results[actual_idx] = Some((name, tool_result));
                    }
                },
                Err(e) => {
                    if let Some(tx) = event_tx {
                        tx.send(AgentEvent::ToolResult {
                                name: Cow::Borrowed("concurrent_error"),
                                result: format!("Task join error: {e}").into(),
                                is_error: true,
                            })
                            .await
                            .ok();
                    }
                },
            }
        }

        // Emit and collect results in original order
        let mut all_results = Vec::with_capacity(tool_calls.len());
        let mut text = String::new();
        for (tc, maybe_result) in tool_calls.iter().zip(ordered_results) {
            let (result_str, is_error) = match maybe_result {
                Some((_, inner)) => (inner.result, inner.is_error),
                None => ("Tool execution failed: no result".to_string(), true),
            };
            let truncated = truncate_result(&result_str, self.config.max_tool_result_size);

            let tool_result = ToolCallResult {
                name: tc.name.clone(),
                arguments: tc.arguments.clone(),
                result: truncated.into(),
                is_error,
            };

            if let Some(tx) = event_tx {
                tx.send(AgentEvent::ToolResult {
                        name: tool_result.name.clone(),
                        result: tool_result.result.clone(),
                        is_error,
                    })
                    .await
                    .ok();
            }

            text.push_str(&format!("Tool {} result:\n{}\n\n", tool_result.name, tool_result.result));

            all_results.push(tool_result);
        }

        (all_results, text)
    }

    async fn run_inner(
        &self,
        messages: Vec<ChatMessage>,
        event_tx: Option<mpsc::Sender<AgentEvent>>,
    ) -> Result<AgentLoopResult> {
        let mut messages = messages;
        if messages.is_empty() || messages[0].role != ChatRole::System {
            messages.insert(
                0,
                ChatMessage {
                    role: ChatRole::System,
                    content: SYSTEM_PROMPT.to_string(),
                },
            );
        }

        let mut all_tool_calls: Vec<ToolCallResult> = Vec::new();
        let mut total_usage = TokenUsage::default();
        let mut final_text: Arc<str> = Arc::from("");

        for iteration in 0..self.config.max_iterations {
            let iter_num = iteration + 1;

            if total_usage.total() >= self.config.max_total_tokens {
                return Ok(self.budget_exhausted_result(&total_usage, iter_num, &all_tool_calls));
            }

            if let Some(ref tx) = event_tx {
                tx.send(AgentEvent::Iteration {
                        number: iter_num,
                        max: self.config.max_iterations,
                    })
                    .await
                    .ok();
                tx.send(AgentEvent::Thinking).await.ok();
            }

            let response = self.call_llm(&messages).await?;
            total_usage.add(&response.usage);

            if total_usage.total() > self.config.max_total_tokens {
                return Ok(self.budget_exhausted_result(&total_usage, iter_num, &all_tool_calls));
            }

            // Prefer native tool calls from the provider API over text-parsed ones.
            // Native tool calls are deterministic and structured; text parsing is
            // a fallback for providers that don't support the tool_use API.
            let (tool_calls, remaining) = if response.tool_calls.is_empty() {
                parse_tool_calls(&response.text)
            } else {
                let native: Vec<ParsedToolCall> = response
                    .tool_calls
                    .into_iter()
                    .map(|tc| ParsedToolCall {
                        name: tc.name,
                        arguments: tc.arguments,
                    })
                    .collect();
                (native, String::new())
            };

            if tool_calls.is_empty() {
                final_text = if remaining.is_empty() {
                    response.text
                } else {
                    remaining
                }
                .into();

                if let Some(ref tx) = event_tx {
                    tx.send(AgentEvent::Chunk(final_text.clone())).await.ok();
                    tx.send(AgentEvent::Done {
                            text: final_text.clone(),
                        })
                        .await
                        .ok();
                }

                return Ok(AgentLoopResult {
                    text: final_text,
                    tool_calls: all_tool_calls,
                    total_usage,
                    iterations: iter_num,
                    termination_reason: AgentTerminationReason::Completed,
                });
            }

            if let Some(ref tx) = event_tx {
                for chunk in response.text.lines() {
                    tx.send(AgentEvent::Chunk(Arc::from(chunk))).await.ok();
                }
            }

            messages.push(ChatMessage {
                role: ChatRole::Assistant,
                content: response.text,
            });

            // Execute tool calls (single fast-path or concurrent)
            let mut tool_results_text = String::new();
            if tool_calls.len() == 1 {
                let (result, text) = self.execute_single_tool(&tool_calls[0], &event_tx).await;
                all_tool_calls.push(result);
                tool_results_text.push_str(&text);
            } else {
                let (results, text) = self.execute_concurrent_tools(&tool_calls, &event_tx).await;
                all_tool_calls.extend(results);
                tool_results_text.push_str(&text);
            }

            messages.push(ChatMessage {
                role: ChatRole::User,
                content: format!(
                    "Here are the tool results. Continue working based on these results. If you \
                     need to call more tools, do so. Otherwise, provide your final answer.\n\n{tool_results_text}"
                ),
            });
        }

        let last_msg = messages
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::Assistant).map_or_else(|| "Agent stopped after max iterations.".to_string(), |m| m.content.clone());

        let max_iter_text: Arc<str> = format!(
            "(Reached max iterations after {} tool calls) {}",
            all_tool_calls.len(),
            last_msg
        )
        .into();

        if let Some(ref tx) = event_tx {
            tx.send(AgentEvent::Done {
                    text: Arc::clone(&max_iter_text),
                })
                .await
                .ok();
        }

        Ok(AgentLoopResult {
            text: max_iter_text,
            tool_calls: all_tool_calls,
            total_usage,
            iterations: self.config.max_iterations,
            termination_reason: AgentTerminationReason::MaxIterations {
                iterations: self.config.max_iterations,
            },
        })
    }

    async fn call_llm(&self, messages: &[ChatMessage]) -> Result<LlmResponse> {
        let mut chunks = Vec::new();
        let mut final_usage = TokenUsage::default();
        let mut native_tool_calls = Vec::new();

        let mut rx = self.llm_client.chat_stream(messages.to_vec()).await?;
        while let Some(event) = rx.recv().await {
            match event {
                StreamEvent::Chunk(text) => chunks.push(text),
                StreamEvent::ToolCallChunk(tc) => {
                    native_tool_calls.push(tc);
                },
                StreamEvent::End { usage, .. } => {
                    final_usage = usage;
                },
            }
        }

        Ok(LlmResponse {
            text: chunks.join(""),
            usage: final_usage,
            tool_calls: native_tool_calls,
        })
    }
}

fn truncate_result(result: &str, max_size: usize) -> String {
    if result.len() <= max_size {
        result.to_string()
    } else {
        let byte_end = max_size.min(result.len());
        let mut end = byte_end;
        while end > 0 && !result.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}... [truncated]", &result[..end])
    }
}

fn parse_tool_calls(text: &str) -> (Vec<ParsedToolCall>, String) {
    let mut tool_calls = Vec::new();
    let mut remaining_lines: Vec<&str> = Vec::new();
    let mut found_any = false;

    for line in text.lines() {
        let trimmed = line.trim();

        if let Some(inner) = trimmed
            .strip_prefix("[TOOL_CALL]")
            .and_then(|s| s.strip_suffix("[/TOOL_CALL]"))
        {
            found_any = true;
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(inner.trim()) {
                if let (Some(name), Some(args)) = (
                    json.get("name").and_then(|v| v.as_str()),
                    json.get("arguments").cloned(),
                ) {
                    tool_calls.push(ParsedToolCall {
                        name: Cow::Owned(name.to_string()),
                        arguments: args,
                    });
                }
            }
            continue;
        }

        if let Some(rest) = trimmed
            .strip_prefix("ant:invoke:")
            .and_then(|s| s.strip_suffix("ant:invoke:end"))
        {
            found_any = true;
            let parts: Vec<&str> = rest
                .splitn(2, |c: char| !c.is_alphanumeric() && c != '_')
                .collect();
            if let Some(name) = parts.first() {
                let args_str = rest.get(name.len()..).unwrap_or("");
                let args: serde_json::Value = if args_str.is_empty() {
                    serde_json::json!({})
                } else {
                    serde_json::from_str(args_str)
                        .unwrap_or_else(|_| serde_json::json!({"raw": args_str.to_string()}))
                };
                tool_calls.push(ParsedToolCall {
                    name: Cow::Owned(name.to_string()),
                    arguments: args,
                });
            }
            continue;
        }

        remaining_lines.push(line);
    }

    let remaining = remaining_lines.join("\n").trim().to_string();
    if found_any && remaining.is_empty() {
        (tool_calls, String::new())
    } else {
        (tool_calls, remaining)
    }
}

struct McpToolResultInner {
    result: String,
    is_error: bool,
}

fn execute_mcp_tool_call(name: &str, arguments: &serde_json::Value) -> McpToolResultInner {
    let request = McpRequest::new(1, "tools/call").with_params(serde_json::json!({
        "name": name,
        "arguments": arguments,
    }));

    let response = crate::mcp::handle_mcp_request(&request);

    match response.result {
        Some(result) => {
            let is_error = result
                .get("is_error")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
                let texts: Vec<&str> = content
                    .iter()
                    .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                    .collect();
                McpToolResultInner {
                    result: texts.join("\n"),
                    is_error,
                }
            } else {
                McpToolResultInner {
                    result: result.to_string(),
                    is_error,
                }
            }
        },
        None => {
            if let Some(err) = response.error {
                McpToolResultInner {
                    result: format!("Tool error: {}", err.message),
                    is_error: true,
                }
            } else {
                McpToolResultInner {
                    result: "Tool returned no result".to_string(),
                    is_error: true,
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::mpsc;

    struct MockLlmClient {
        responses: Vec<String>,
        call_index: AtomicUsize,
        usage_per_call: TokenUsage,
    }

    impl MockLlmClient {
        fn new(responses: Vec<String>) -> Self {
            Self {
                responses,
                call_index: AtomicUsize::new(0),
                usage_per_call: TokenUsage::default(),
            }
        }

        fn with_usage(responses: Vec<String>, usage: TokenUsage) -> Self {
            Self {
                responses,
                call_index: AtomicUsize::new(0),
                usage_per_call: usage,
            }
        }

        fn next_response(&self) -> String {
            let idx = self.call_index.fetch_add(1, Ordering::SeqCst);
            self.responses[idx % self.responses.len()].clone()
        }
    }

    #[async_trait::async_trait]
    impl crate::llm::providers::LlmClient for MockLlmClient {
        async fn chat(&self, _messages: Vec<ChatMessage>) -> crate::Result<LlmResponse> {
            Ok(LlmResponse {
                text: self.next_response(),
                usage: self.usage_per_call.clone(),
                tool_calls: vec![],
            })
        }

        async fn chat_stream(
            &self,
            _messages: Vec<ChatMessage>,
        ) -> crate::Result<mpsc::Receiver<StreamEvent>> {
            let (tx, rx) = mpsc::channel(16);
            let text = self.next_response();
            let usage = self.usage_per_call.clone();
            tokio::spawn(async move {
                let _ = tx.send(StreamEvent::Chunk(text)).await;
                let _ = tx
                    .send(StreamEvent::End {
                        usage,
                        captured_tool_calls: vec![],
                    })
                    .await;
            });
            Ok(rx)
        }

        fn count_tokens(&self, _text: &str) -> usize {
            0
        }
    }

    #[tokio::test]
    async fn test_tool_calls_parsed_and_executed() {
        let cargo_path = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let response_text = format!(
            "[TOOL_CALL] {{\"name\": \"read_file\", \"arguments\": {{\"path\": \"{cargo_path}/Cargo.toml\"}}}} [/TOOL_CALL]"
        );

        let client = Arc::new(MockLlmClient::new(vec![
            response_text,
            "The project uses workspace layout.".to_string(),
        ]));
        let loop_ = AgentLoop::new(client).with_config(AgentLoopConfig {
            max_iterations: 5,
            ..AgentLoopConfig::default()
        });

        let (event_tx, mut event_rx) = mpsc::channel(64);
        let result = loop_
            .run(
                vec![ChatMessage {
                    role: ChatRole::User,
                    content: "Read the Cargo.toml".to_string(),
                }],
                Some(event_tx),
            )
            .await
            .unwrap();

        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name, "read_file");
        assert!(!result.tool_calls[0].result.is_empty());
        assert!(!result.tool_calls[0].is_error);
        assert_eq!(result.iterations, 2);
        assert!(result.text.contains("workspace"));
        matches!(result.termination_reason, AgentTerminationReason::Completed);

        let mut found_tool_call = false;
        let mut found_tool_result = false;
        while let Some(event) = event_rx.recv().await {
            match event {
                AgentEvent::ToolCall { name, .. } => {
                    assert_eq!(name, "read_file");
                    found_tool_call = true;
                },
                AgentEvent::ToolResult { name, .. } => {
                    assert_eq!(name, "read_file");
                    found_tool_result = true;
                },
                _ => {},
            }
        }
        assert!(found_tool_call);
        assert!(found_tool_result);
    }

    #[tokio::test]
    async fn test_max_iterations_respected() {
        let tool_call = "[TOOL_CALL] {\"name\": \"git_status\", \"arguments\": {}} [/TOOL_CALL]";
        let client = Arc::new(MockLlmClient::new(vec![
            tool_call.to_string(),
            tool_call.to_string(),
            tool_call.to_string(),
            "done".to_string(),
        ]));
        let loop_ = AgentLoop::new(client).with_config(AgentLoopConfig {
            max_iterations: 2,
            ..AgentLoopConfig::default()
        });

        let result = loop_
            .run(
                vec![ChatMessage {
                    role: ChatRole::User,
                    content: "Check git".to_string(),
                }],
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.iterations, 2);
        assert!(result.text.contains("max iterations"));
        matches!(
            result.termination_reason,
            AgentTerminationReason::MaxIterations { iterations: 2 }
        );
    }

    #[tokio::test]
    async fn test_max_iterations_termination_reason() {
        let tool_call = "[TOOL_CALL] {\"name\": \"git_status\", \"arguments\": {}} [/TOOL_CALL]";
        let client = Arc::new(MockLlmClient::new(vec![
            tool_call.to_string(),
            tool_call.to_string(),
            "done".to_string(),
        ]));
        let loop_ = AgentLoop::new(client).with_max_iterations(2);

        let result = loop_
            .run(
                vec![ChatMessage {
                    role: ChatRole::User,
                    content: "Check git".to_string(),
                }],
                None,
            )
            .await
            .unwrap();

        match result.termination_reason {
            AgentTerminationReason::MaxIterations { iterations } => {
                assert_eq!(iterations, 2);
            },
            other => panic!("Expected MaxIterations, got {:?}", other),
        }
        assert_eq!(result.iterations, 2);
    }

    #[tokio::test]
    async fn test_token_budget_exhausted() {
        let response_text = "I will think about this.";
        let client = Arc::new(MockLlmClient::with_usage(
            vec![response_text.to_string()],
            TokenUsage {
                input: 60_000,
                output: 60_000,
                cached: 0,
            },
        ));
        let loop_ = AgentLoop::new(client).with_config(AgentLoopConfig {
            max_total_tokens: 100_000,
            max_tokens_per_iteration: 200_000,
            ..AgentLoopConfig::default()
        });

        let result = loop_
            .run(
                vec![ChatMessage {
                    role: ChatRole::User,
                    content: "Tell me something".to_string(),
                }],
                None,
            )
            .await
            .unwrap();

        match result.termination_reason {
            AgentTerminationReason::TokenBudgetExhausted {
                tokens_used,
                budget,
            } => {
                assert_eq!(tokens_used, 120_000);
                assert_eq!(budget, 100_000);
            },
            other => panic!("Expected TokenBudgetExhausted, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_tool_results_fed_back_to_llm() {
        let response1 = "[TOOL_CALL] {\"name\": \"list_directory\", \"arguments\": {\"path\": \".\"}} [/TOOL_CALL]";
        let client = Arc::new(MockLlmClient::new(vec![
            response1.to_string(),
            "I see the directory contents.".to_string(),
        ]));
        let loop_ = AgentLoop::new(client).with_config(AgentLoopConfig {
            max_iterations: 5,
            ..AgentLoopConfig::default()
        });

        let (event_tx, mut event_rx) = mpsc::channel(64);
        let result = loop_
            .run(
                vec![ChatMessage {
                    role: ChatRole::User,
                    content: "List files".to_string(),
                }],
                Some(event_tx),
            )
            .await
            .unwrap();

        assert_eq!(result.iterations, 2);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name, "list_directory");
        assert!(result.text.contains("directory contents"));
        matches!(result.termination_reason, AgentTerminationReason::Completed);

        let mut found_thinking_after_tools = false;
        let mut iteration_count = 0;
        while let Some(event) = event_rx.recv().await {
            match event {
                AgentEvent::Iteration { .. } => {
                    iteration_count += 1;
                },
                AgentEvent::Thinking => {
                    if iteration_count >= 2 {
                        found_thinking_after_tools = true;
                    }
                },
                _ => {},
            }
        }
        assert!(
            found_thinking_after_tools,
            "LLM should be called again after tool results"
        );
    }

    #[test]
    fn test_parse_tool_calls_bracket_format() {
        let input = "I need to read a file.\n[TOOL_CALL] {\"name\": \"read_file\", \"arguments\": {\"path\": \"src/main.rs\"}} [/TOOL_CALL]\nLet me check that.";
        let (calls, remaining) = parse_tool_calls(input);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].arguments["path"], "src/main.rs");
        assert!(remaining.contains("I need to read a file"));
        assert!(remaining.contains("Let me check that"));
    }

    #[test]
    fn test_parse_tool_calls_invoke_format() {
        let input = "ant:invoke:git_status{}ant:invoke:end";
        let (calls, remaining) = parse_tool_calls(input);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "git_status");
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_parse_tool_calls_no_tool_calls() {
        let input = "This is just a plain response with no tool calls.";
        let (calls, remaining) = parse_tool_calls(input);
        assert!(calls.is_empty());
        assert_eq!(remaining, input);
    }

    #[test]
    fn test_parse_tool_calls_multiple() {
        let input = "[TOOL_CALL] {\"name\": \"git_status\", \"arguments\": {}} [/TOOL_CALL]\n[TOOL_CALL] {\"name\": \"read_file\", \"arguments\": {\"path\": \"Cargo.toml\"}} [/TOOL_CALL]";
        let (calls, remaining) = parse_tool_calls(input);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "git_status");
        assert_eq!(calls[1].name, "read_file");
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_execute_mcp_tool_call_git_status() {
        let result = execute_mcp_tool_call("git_status", &serde_json::json!({}));
        assert!(
            result.result.contains("clean")
                || result.result.contains("M")
                || result.result.contains("A")
                || result.result.contains("??")
                || result.result.contains("working tree")
        );
        assert!(!result.is_error);
    }

    #[test]
    fn test_execute_mcp_tool_call_unknown_tool() {
        let result = execute_mcp_tool_call("nonexistent_tool_xyz", &serde_json::json!({}));
        assert!(result.is_error);
    }

    #[test]
    fn test_truncate_result_small() {
        let input = "short result";
        assert_eq!(truncate_result(input, 1024), input);
    }

    #[test]
    fn test_truncate_result_exact() {
        let input = "a".repeat(100);
        assert_eq!(truncate_result(&input, 100), input);
    }

    #[test]
    fn test_truncate_result_exceeds() {
        let input = "x".repeat(200);
        let result = truncate_result(&input, 100);
        assert!(result.ends_with("... [truncated]"));
        assert!(result.len() < input.len());
        assert!(result.len() > 100);
    }

    #[test]
    fn test_truncate_result_multibyte_boundary() {
        let input = "a".repeat(50) + &"日本語".repeat(20);
        let result = truncate_result(&input, 80);
        assert!(result.ends_with("... [truncated]"));
        let without_suffix = result.strip_suffix("... [truncated]").unwrap();
        assert!(without_suffix.is_char_boundary(without_suffix.len()));
    }

    #[test]
    fn test_agent_loop_config_default() {
        let config = AgentLoopConfig::default();
        assert_eq!(config.max_iterations, 50);
        assert_eq!(config.max_total_tokens, 200_000);
        assert_eq!(config.max_wall_time, Duration::from_secs(300));
        assert_eq!(config.max_tokens_per_iteration, 50_000);
        assert_eq!(config.max_tool_result_size, 102_400);
    }

    #[test]
    fn test_with_max_iterations_convenience() {
        let client = Arc::new(MockLlmClient::new(vec!["done".to_string()]));
        let loop_ = AgentLoop::new(client).with_max_iterations(10);
        assert_eq!(loop_.config.max_iterations, 10);
        assert_eq!(loop_.config.max_total_tokens, 200_000);
    }

    #[tokio::test]
    async fn test_concurrent_tool_calls_multiple_tools() {
        let cargo_path = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let response_text = format!(
            "[TOOL_CALL] {{\"name\": \"git_status\", \"arguments\": {{}}}} [/TOOL_CALL]\n\
             [TOOL_CALL] {{\"name\": \"list_directory\", \"arguments\": {{\"path\": \".\"}}}} [/TOOL_CALL]"
        );

        let client = Arc::new(MockLlmClient::new(vec![
            response_text,
            "Both tools returned results.".to_string(),
        ]));
        let loop_ = AgentLoop::new(client).with_config(AgentLoopConfig {
            max_iterations: 5,
            ..AgentLoopConfig::default()
        });

        let result = loop_
            .run(
                vec![ChatMessage {
                    role: ChatRole::User,
                    content: "Check git and list files".to_string(),
                }],
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.tool_calls.len(), 2);
        assert_eq!(result.tool_calls[0].name, "git_status");
        assert_eq!(result.tool_calls[1].name, "list_directory");
        assert!(!result.tool_calls[0].result.is_empty());
        assert!(!result.tool_calls[1].result.is_empty());
        assert_eq!(result.iterations, 2);
        matches!(result.termination_reason, AgentTerminationReason::Completed);
    }

    #[tokio::test]
    async fn test_single_tool_call_uses_fast_path() {
        let cargo_path = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let response_text = format!(
            "[TOOL_CALL] {{\"name\": \"read_file\", \"arguments\": {{\"path\": \"{cargo_path}/Cargo.toml\"}}}} [/TOOL_CALL]"
        );

        let client = Arc::new(MockLlmClient::new(vec![
            response_text,
            "Done reading.".to_string(),
        ]));
        let loop_ = AgentLoop::new(client).with_config(AgentLoopConfig {
            max_iterations: 5,
            ..AgentLoopConfig::default()
        });

        let result = loop_
            .run(
                vec![ChatMessage {
                    role: ChatRole::User,
                    content: "Read Cargo.toml".to_string(),
                }],
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name, "read_file");
        assert!(!result.tool_calls[0].result.is_empty());
    }
}
