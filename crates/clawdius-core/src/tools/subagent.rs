//! Subagent tool: a bounded, nested agent loop the primary agent can
//! delegate focused work to.
//!
//! Claude Code parity row (`Task`/`AgentTool`). The primary agent calls
//! `subagent` with a narrowed task; this module runs a secondary tool loop
//! (`chat_with_tools` -> execute -> feed results) against the same LLM
//! client and returns the subagent's final summary. The inner loop is
//! strictly bounded (iterations, total tool calls, output size) so a
//! runaway delegation cannot consume the session.
//!
//! Tool execution is injected as a closure so any caller can share its own
//! dispatcher (the TUI executor passes its `execute_tool`).

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use genai::chat::{ChatMessage as GenaiMessage, ChatRequest};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::llm::providers::{ChatWithToolsResult, LlmClient, Tool};
use crate::llm::{ChatMessage, ChatRole};

/// Executes a named tool with JSON arguments: `(output, is_error)`.
pub type SubToolExecutor = Arc<dyn Fn(&str, &str) -> (String, bool) + Send + Sync>;

/// Bounds for one delegation.
#[derive(Debug, Clone)]
pub struct SubagentConfig {
    /// Maximum `chat_with_tools` round trips.
    pub max_iterations: usize,
    /// Maximum total tool executions across the delegation.
    pub max_tool_calls: usize,
    /// Maximum characters fed back per tool result.
    pub max_output_chars: usize,
}

impl Default for SubagentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 8,
            max_tool_calls: 40,
            max_output_chars: 6_000,
        }
    }
}

/// A bounded nested agent loop.
pub struct SubagentTool {
    client: Arc<dyn LlmClient>,
    tools: Vec<Tool>,
    executor: SubToolExecutor,
    config: SubagentConfig,
}

const SUBAGENT_SYSTEM: &str = "You are a focused subagent completing one \
delegated task. Use the provided tools as needed, then report a concise \
summary of what you did and the outcome. Do not ask questions; decide and \
act.";

impl SubagentTool {
    /// Create a subagent tool over `client` with tool definitions `tools`
    /// executed through `executor`.
    pub fn new(
        client: Arc<dyn LlmClient>,
        tools: Vec<Tool>,
        executor: SubToolExecutor,
        config: SubagentConfig,
    ) -> Self {
        Self {
            client,
            tools,
            executor,
            config,
        }
    }

    /// Run the delegation loop for `task` and return the subagent summary.
    ///
    /// # Errors
    ///
    /// Provider errors and empty tool-call payloads surface as
    /// [`crate::Error::Llm`].
    pub async fn run(&self, task: &str) -> Result<String> {
        let mut messages = vec![
            ChatMessage {
                role: ChatRole::System,
                content: SUBAGENT_SYSTEM.to_string(),
            },
            ChatMessage {
                role: ChatRole::User,
                content: task.to_string(),
            },
        ];
        let mut tool_calls_used = 0usize;

        for _iteration in 0..self.config.max_iterations {
            let result: ChatWithToolsResult = self
                .client
                .chat_with_tools(messages.clone(), self.tools.clone())
                .await?;

            messages.push(ChatMessage {
                role: ChatRole::Assistant,
                content: result.text.clone(),
            });

            if result.tool_calls.is_empty() {
                return Ok(if result.text.trim().is_empty() {
                    "Subagent completed without a final summary.".to_string()
                } else {
                    result.text
                });
            }

            let mut feedback = String::from("Tool results:\n");
            for call in &result.tool_calls {
                if tool_calls_used >= self.config.max_tool_calls {
                    feedback.push_str(&format!(
                        "{}: skipped (tool-call budget exhausted)\n",
                        call.fn_name
                    ));
                    continue;
                }
                tool_calls_used += 1;
                let args =
                    serde_json::to_string(&call.fn_arguments).unwrap_or_else(|_| "{}".to_string());
                let (output, is_error) = (self.executor)(&call.fn_name, &args);
                feedback.push_str(&format!(
                    "{} ({}): {}\n",
                    call.fn_name,
                    call.call_id,
                    format_tool_result(&output, is_error, self.config.max_output_chars)
                ));
            }
            messages.push(ChatMessage {
                role: ChatRole::User,
                content: feedback,
            });
        }

        // Iterations exhausted: one plain turn for the final summary.
        messages.push(ChatMessage {
            role: ChatRole::User,
            content: "Iteration budget reached. Report your final summary now.".to_string(),
        });
        self.client.chat(messages).await
    }
}

/// Formats a tool result payload for the model transcript.
#[must_use]
pub fn format_tool_result(output: &str, is_error: bool, max_chars: usize) -> String {
    let tag = if is_error { "error" } else { "ok" };
    format!(
        "[{tag}] {}",
        output.chars().take(max_chars).collect::<String>()
    )
}

/// Convenience: extract a JSON-string argument with a default.
#[must_use]
pub fn arg_str(arguments: &Value, key: &str) -> Option<String> {
    arguments.get(key).and_then(Value::as_str).map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::providers::ToolCall;
    use std::sync::Mutex;

    /// Scripted client: pops pre-built tool-call results, then serves the
    /// final text for plain `chat` calls.
    struct MockClient {
        scripted_tools: Mutex<Vec<ChatWithToolsResult>>,
        final_text: String,
        seen_plain_chat: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl LlmClient for MockClient {
        async fn chat(&self, messages: Vec<ChatMessage>) -> crate::Result<String> {
            self.seen_plain_chat
                .lock()
                .unwrap()
                .extend(messages.iter().map(|m| m.content.clone()));
            Ok(self.final_text.clone())
        }

        async fn chat_stream(
            &self,
            _messages: Vec<ChatMessage>,
        ) -> crate::Result<tokio::sync::mpsc::Receiver<String>> {
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            drop(tx);
            Ok(rx)
        }

        async fn chat_with_tools(
            &self,
            messages: Vec<ChatMessage>,
            _tools: Vec<Tool>,
        ) -> crate::Result<ChatWithToolsResult> {
            let scripted = self
                .scripted_tools
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| Error::Llm("script exhausted".to_string()))?;
            let _ = messages;
            Ok(scripted)
        }

        fn count_tokens(&self, _text: &str) -> usize {
            0
        }
    }

    fn tool_call(name: &str) -> ToolCall {
        ToolCall {
            call_id: "c1".to_string(),
            fn_name: name.to_string(),
            fn_arguments: serde_json::json!({ "task": "do it" }),
            thought_signatures: None,
        }
    }

    fn pop_order(scripted: &[ChatWithToolsResult]) -> Vec<ChatWithToolsResult> {
        scripted.iter().rev().cloned().collect()
    }

    #[tokio::test]
    async fn delegates_then_reports_summary() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let client = Arc::new(MockClient {
            scripted_tools: Mutex::new(pop_order(&[
                ChatWithToolsResult {
                    text: String::new(),
                    tool_calls: vec![tool_call("echo")],
                },
                ChatWithToolsResult {
                    text: "done: hi".to_string(),
                    tool_calls: Vec::new(),
                },
            ])),
            final_text: "final summary".to_string(),
            seen_plain_chat: Arc::clone(&seen),
        });
        let executed = Arc::new(Mutex::new(Vec::new()));
        let executed_for_closure = Arc::clone(&executed);
        let executor: SubToolExecutor = Arc::new(move |name: &str, args: &str| {
            executed_for_closure
                .lock()
                .unwrap()
                .push((name.to_string(), args.to_string()));
            (String::from("echoed output"), false)
        });
        let tool = SubagentTool::new(client, Vec::new(), executor, SubagentConfig::default());

        let summary = tool.run("do it").await.unwrap();
        assert_eq!(summary, "done: hi");
        assert_eq!(executed.lock().unwrap()[0].0, "echo");
    }

    #[tokio::test]
    async fn iteration_budget_forces_summary() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let scripted: Vec<ChatWithToolsResult> = (0..10)
            .map(|_| ChatWithToolsResult {
                text: String::new(),
                tool_calls: vec![tool_call("echo")],
            })
            .collect();
        let client = Arc::new(MockClient {
            scripted_tools: Mutex::new(pop_order(&scripted)),
            final_text: "budget summary".to_string(),
            seen_plain_chat: Arc::clone(&seen),
        });
        let executor: SubToolExecutor =
            Arc::new(|_name: &str, _args: &str| (String::from("x"), false));
        let tool = SubagentTool::new(
            client,
            Vec::new(),
            executor,
            SubagentConfig {
                max_iterations: 3,
                ..SubagentConfig::default()
            },
        );

        let summary = tool.run("endless").await.unwrap();
        assert_eq!(summary, "budget summary");
        assert!(
            seen.lock()
                .unwrap()
                .iter()
                .any(|c| c.contains("Iteration budget reached")),
            "final summary turn must be requested"
        );
    }

    #[tokio::test]
    async fn tool_errors_are_fed_back_not_fatal() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let client = Arc::new(MockClient {
            scripted_tools: Mutex::new(pop_order(&[
                ChatWithToolsResult {
                    text: String::new(),
                    tool_calls: vec![tool_call("boom")],
                },
                ChatWithToolsResult {
                    text: "recovered".to_string(),
                    tool_calls: Vec::new(),
                },
            ])),
            final_text: String::new(),
            seen_plain_chat: Arc::clone(&seen),
        });
        let executor: SubToolExecutor =
            Arc::new(|_name: &str, _args: &str| (String::from("simulated failure"), true));
        let tool = SubagentTool::new(client, Vec::new(), executor, SubagentConfig::default());
        let summary = tool.run("try anyway").await.unwrap();
        assert_eq!(summary, "recovered");
    }

    #[test]
    fn helpers_shape_output() {
        assert!(format_tool_result("boom", true, 10).starts_with("[error]"));
        assert_eq!(format_tool_result("abcdef", false, 3), "[ok] abc");
        assert_eq!(format_tool_result("hit", false, 100), "[ok] hit");
        assert_eq!(
            arg_str(&serde_json::json!({"path": "/a"}), "path"),
            Some("/a".to_string())
        );
    }
}
