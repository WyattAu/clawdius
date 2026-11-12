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
use crate::tools::loop_guard::{state_hash, LoopGuard, LoopStatus};

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

/// The typed per-run state of the delegation loop (the "blackboard").
///
/// Every mutable fact the engine loop needs lives here — transcript,
/// budgets, bounce counter, loop guard — instead of loose locals in
/// `run()`. The controller (`SubagentTool::run`) mutates the blackboard
/// through the transition methods below and renders the model-visible
/// slice with [`LoopBlackboard::context`]; control state never leaks
/// into the transcript.
struct LoopBlackboard {
    messages: Vec<ChatMessage>,
    tool_calls_used: usize,
    strategy_bounces: usize,
    loop_guard: LoopGuard,
    max_tool_calls: usize,
}

impl LoopBlackboard {
    fn new(
        task: &str,
        system_prompt: &str,
        max_state_repeats: usize,
        max_tool_calls: usize,
    ) -> Self {
        Self {
            messages: vec![
                ChatMessage {
                    role: ChatRole::System,
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: ChatRole::User,
                    content: task.to_string(),
                },
            ],
            tool_calls_used: 0,
            strategy_bounces: 0,
            loop_guard: LoopGuard::new(max_state_repeats),
            max_tool_calls,
        }
    }

    /// Semantic state = response text + requested tool names, hashed;
    /// observed on every model turn for loop detection.
    fn observe_state(&mut self, text: &str, tool_names: &[String]) -> LoopStatus {
        let mut state_parts: Vec<String> = vec![text.to_string()];
        state_parts.extend(tool_names.iter().cloned());
        let state = state_hash(&state_parts);
        self.loop_guard.observe(&state.to_string())
    }

    /// First repeat: bounce once with a strategy-change instruction.
    /// Returns `false` when the bounce budget is exhausted (hard fail).
    fn bounce(&mut self) -> bool {
        if self.strategy_bounces == 0 {
            self.strategy_bounces += 1;
            self.messages.push(ChatMessage {
                role: ChatRole::User,
                content: "You are repeating the same approach without progress. \
                          Try a FUNDAMENTALLY DIFFERENT strategy: different files, \
                          different commands, or question your earlier assumptions."
                    .to_string(),
            });
            true
        } else {
            false
        }
    }

    /// Remaining tool-execution budget for this turn.
    fn remaining_tool_calls(&self) -> usize {
        self.max_tool_calls.saturating_sub(self.tool_calls_used)
    }

    fn record_tool_call(&mut self) {
        self.tool_calls_used += 1;
    }

    fn push_assistant(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: ChatRole::Assistant,
            content,
        });
    }

    fn push_user(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: ChatRole::User,
            content,
        });
    }

    /// The model-visible slice: the transcript itself (control state is
    /// carried by the engine, not the context).
    fn context(&self) -> &[ChatMessage] {
        &self.messages
    }
}

/// A bounded nested agent loop.
pub struct SubagentTool {
    client: Arc<dyn LlmClient>,
    tools: Vec<Tool>,
    executor: SubToolExecutor,
    config: SubagentConfig,
    system: String,
    /// Semantic loop detector knobs.
    pub engine_options: crate::tools::subagent::EngineOptions,
}

/// Extra knobs for the engine loop.
#[derive(Debug, Clone)]
pub struct EngineOptions {
    /// Loop-detection: halt after this many visits of an identical
    /// semantic state. `Some(2)` (halt on 2nd repeat) is the default.
    pub max_state_repeats: usize,
}

impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            max_state_repeats: 2,
        }
    }
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
            system: SUBAGENT_SYSTEM.to_string(),
            engine_options: EngineOptions::default(),
        }
    }

    /// Override the subagent system prompt (e.g. with the caller's agent-mode prompt).
    #[must_use]
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system = prompt.into();
        self
    }

    /// Install a semantic loop detector (engine option).
    #[must_use]
    pub fn with_engine_options(mut self, options: EngineOptions) -> Self {
        self.engine_options = options;
        self
    }

    /// Run the delegation loop for `task` and return the subagent summary.
    ///
    /// # Errors
    ///
    /// Provider errors and empty tool-call payloads surface as
    /// [`crate::Error::Llm`].
    pub async fn run(&self, task: &str) -> Result<String> {
        let mut board = LoopBlackboard::new(
            task,
            &self.system,
            self.engine_options.max_state_repeats,
            self.config.max_tool_calls,
        );

        for _iteration in 0..self.config.max_iterations {
            let result: ChatWithToolsResult = self
                .client
                .chat_with_tools(board.context().to_vec(), self.tools.clone())
                .await?;

            // Semantic loop detection: identical (response + tool names)
            // state on consecutive observation means the run is spinning.
            let tool_names: Vec<String> = result
                .tool_calls
                .iter()
                .map(|call| call.fn_name.clone())
                .collect();
            if let LoopStatus::Repeat(count) = board.observe_state(&result.text, &tool_names) {
                if board.bounce() {
                    continue;
                }
                return Err(Error::Llm(format!(
                    "semantic loop detected: state repeated {count} time(s) even after a \
                     strategy change; the run is spinning"
                )));
            }

            board.push_assistant(result.text.clone());

            if result.tool_calls.is_empty() {
                return Ok(if result.text.trim().is_empty() {
                    "Subagent completed without a final summary.".to_string()
                } else {
                    result.text
                });
            }

            let mut feedback = String::from("Tool results:\n");
            for call in &result.tool_calls {
                if board.remaining_tool_calls() == 0 {
                    feedback.push_str(&format!(
                        "{}: skipped (tool-call budget exhausted)\n",
                        call.fn_name
                    ));
                    continue;
                }
                board.record_tool_call();
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
            board.push_user(feedback);
        }

        // Iterations exhausted: one plain turn for the final summary.
        board.push_user("Iteration budget reached. Report your final summary now.".to_string());
        self.client.chat(board.context().to_vec()).await
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

    // ---- blackboard state machine (no provider needed) ----

    #[test]
    fn blackboard_bounces_once_then_hard_fails() {
        let mut board = LoopBlackboard::new("task", "sys", 2, 10);
        assert!(board.bounce(), "first bounce allowed");
        assert!(!board.bounce(), "second bounce refused");
    }

    #[test]
    fn blackboard_tool_budget_exhaustion() {
        let mut board = LoopBlackboard::new("task", "sys", 2, 2);
        assert_eq!(board.remaining_tool_calls(), 2);
        board.record_tool_call();
        board.record_tool_call();
        assert_eq!(board.remaining_tool_calls(), 0);
    }

    #[test]
    fn blackboard_repeats_identical_state() {
        let mut board = LoopBlackboard::new("task", "sys", 2, 10);
        let names = vec!["grep".to_string()];
        assert!(matches!(
            board.observe_state("thinking", &names),
            LoopStatus::Continue
        ));
        // Same state again: guard counts up to max_state_repeats (2).
        assert!(matches!(
            board.observe_state("thinking", &names),
            LoopStatus::Continue
        ));
        assert!(matches!(
            board.observe_state("thinking", &names),
            LoopStatus::Repeat(_)
        ));
    }

    #[test]
    fn blackboard_transcript_shape_is_stable() {
        let mut board = LoopBlackboard::new("the task", "the system", 2, 10);
        board.push_assistant("resp".into());
        board.push_user("feedback".into());
        let ctx = board.context();
        assert_eq!(ctx.len(), 4);
        assert!(matches!(ctx[0].role, ChatRole::System));
        assert_eq!(ctx[1].content, "the task");
        assert!(matches!(ctx[2].role, ChatRole::Assistant));
        assert!(matches!(ctx[3].role, ChatRole::User));
    }

    // ---- scripted end-to-end loop tests ----

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
    async fn identical_states_halt_with_loop_detection() {
        // Blueprint item 46: a run visiting the same semantic state twice
        // is spinning - the engine halts it instead of burning the budget.
        let scripted: Vec<ChatWithToolsResult> = (0..10)
            .map(|_| ChatWithToolsResult {
                text: String::new(),
                tool_calls: vec![tool_call("echo")],
            })
            .collect();
        let client = Arc::new(MockClient {
            scripted_tools: Mutex::new(pop_order(&scripted)),
            final_text: String::new(),
            seen_plain_chat: Arc::new(Mutex::new(Vec::new())),
        });
        let executor: SubToolExecutor =
            Arc::new(|_name: &str, _args: &str| (String::from("x"), false));
        let tool = SubagentTool::new(
            client,
            Vec::new(),
            executor,
            SubagentConfig {
                max_iterations: 4,
                ..SubagentConfig::default()
            },
        );

        let err = tool.run("endless").await.unwrap_err();
        assert!(
            err.to_string().contains("semantic loop detected"),
            "second spin after the strategy bounce must hard-fail"
        );
    }

    #[tokio::test]
    async fn distinct_progress_never_triggers_loop_detection() {
        let scripted: Vec<ChatWithToolsResult> = (0..6)
            .map(|i| ChatWithToolsResult {
                text: format!("progress step {i}"),
                tool_calls: vec![ToolCall {
                    call_id: format!("c{i}"),
                    fn_name: "echo".to_string(),
                    fn_arguments: serde_json::json!({ "i": i }),
                    thought_signatures: None,
                }],
            })
            .collect();
        let client = Arc::new(MockClient {
            scripted_tools: Mutex::new(scripted),
            final_text: "budget summary".to_string(),
            seen_plain_chat: Arc::new(Mutex::new(Vec::new())),
        });
        let executor: SubToolExecutor =
            Arc::new(|_name: &str, _args: &str| (String::from("x"), false));
        let tool = SubagentTool::new(
            client,
            Vec::new(),
            executor,
            SubagentConfig {
                max_iterations: 6,
                ..SubagentConfig::default()
            },
        );

        let summary = tool.run("evolving").await.unwrap();
        assert_eq!(summary, "budget summary");
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
