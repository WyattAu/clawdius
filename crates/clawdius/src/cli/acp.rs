//! ACP (Agent Client Protocol) agent — `clawdius acp`.
//!
//! Speaks ACP v1 (JSON-RPC 2.0, newline-delimited over stdio) so editors
//! such as Zed can drive clawdius as a subprocess agent. Baseline
//! methods: `initialize`, `session/new`, `session/prompt`; notifications:
//! `session/update` (`agent_message_chunk`, `tool_call`,
//! `tool_call_update`), `session/cancel` (honored between tool
//! executions and at model awaits).
//!
//! The prompt turn reuses the dogfood-proven delegation engine
//! ([`clawdius_core::tools::subagent::SubagentTool`]) with the auto-mode
//! tool host — one engine, one loop guard, one journal, three hosts.

use super::load_config;
use anyhow::Result;
use clawdius_core::llm::providers::LlmClient;
use clawdius_core::llm::{create_provider, ResolvedLlmConfig};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

/// MCP config items are accepted but ignored in v1 (no client MCP proxy).
pub(super) async fn handle_acp(
    provider: String,
    model: Option<String>,
    config_path: Option<PathBuf>,
) -> Result<()> {
    let config = load_config(config_path.as_deref())?;
    // The provider is created lazily at the first prompt so editors can
    // complete the ACP handshake before any API key is configured (the
    // whole config resolution — including key lookup — is deferred).
    let factory = Arc::new(move || {
        let mut llm_config = ResolvedLlmConfig::from_config(&config.llm, &provider)?;
        if let Some(ref m) = model {
            llm_config.model.clone_from(m);
        }
        create_provider(&llm_config)
    })
        as Arc<dyn Fn() -> clawdius_core::Result<clawdius_core::llm::LlmProvider> + Send + Sync>;
    let mut server = AcpServer::new(None, Some(factory));
    let stdin = std::io::stdin();
    let mut line = String::new();
    loop {
        line.clear();
        let n = stdin.lock().read_line(&mut line)?;
        if n == 0 {
            break; // EOF: client closed the pipe
        }
        let Ok(msg) = serde_json::from_str::<Value>(line.trim()) else {
            continue; // tolerate keep-alive junk lines
        };
        for out in server.handle(&msg).await {
            let mut stdout = std::io::stdout().lock();
            serde_json::to_writer(&mut stdout, &out)?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
        }
        if server.shutdown {
            break;
        }
    }
    Ok(())
}

/// An ACP session: id + cwd + cancel flag.
struct AcpSession {
    #[allow(dead_code)]
    cwd: PathBuf,
    cancel_tx: watch::Sender<bool>,
}

/// Deferred provider construction (called at the first prompt).
type ClientFactory =
    Arc<dyn Fn() -> clawdius_core::Result<clawdius_core::llm::LlmProvider> + Send + Sync>;

/// Protocol-level server state (pure — unit-testable without stdio).
pub(crate) struct AcpServer {
    client: Option<Arc<dyn LlmClient>>,
    client_factory: Option<ClientFactory>,
    sessions: std::collections::HashMap<String, AcpSession>,
    session_seq: usize,
    /// Set when the client asks to shut down.
    pub(crate) shutdown: bool,
}

impl AcpServer {
    pub(crate) fn new(client: Option<Arc<dyn LlmClient>>, factory: Option<ClientFactory>) -> Self {
        Self {
            client,
            client_factory: factory,
            sessions: std::collections::HashMap::new(),
            session_seq: 0,
            shutdown: false,
        }
    }

    /// Resolve the LLM client, creating it lazily via the factory.
    fn resolve_client(&mut self) -> Result<Arc<dyn LlmClient>, Value> {
        if let Some(client) = &self.client {
            return Ok(Arc::clone(client));
        }
        if let Some(factory) = &self.client_factory {
            match factory() {
                Ok(client) => {
                    let client: Arc<dyn LlmClient> = Arc::new(client);
                    self.client = Some(Arc::clone(&client));
                    return Ok(client);
                },
                Err(e) => {
                    return Err(json!({
                        "code": -32000,
                        "message": format!("provider unavailable: {e}")
                    }))
                },
            }
        }
        Err(json!({ "code": -32000, "message": "no LLM provider configured" }))
    }

    /// Handle one JSON-RPC message; returns responses/notifications to
    /// write (in order). Notifications precede the response of the same
    /// message.
    pub(crate) async fn handle(&mut self, msg: &Value) -> Vec<Value> {
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let has_id = msg.get("id").is_some();
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        let params = msg.get("params").cloned().unwrap_or(Value::Null);

        // Notifications (no id): session/cancel and shutdown bookkeeping.
        if !has_id {
            return match method {
                "session/cancel" => {
                    self.cancel_session(&params);
                    Vec::new()
                },
                _ => Vec::new(),
            };
        }

        match method {
            "initialize" => vec![json!({
                "jsonrpc": "2.0", "id": id,
                "result": {
                    "protocolVersion": 1,
                    "agentCapabilities": {
                        "loadSession": false,
                        "promptCapabilities": {
                            "image": false, "audio": false, "embeddedContext": false
                        }
                    },
                    "agentInfo": {
                        "name": "clawdius",
                        "title": "Clawdius",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "authMethods": []
                }
            })],
            "session/new" => {
                let cwd = params.get("cwd").and_then(Value::as_str).map_or_else(
                    || std::env::current_dir().unwrap_or_default(),
                    PathBuf::from,
                );
                self.session_seq += 1;
                let session_id = format!("sess_{}", self.session_seq);
                let (cancel_tx, _cancel_rx) = watch::channel(false);
                self.sessions
                    .insert(session_id.clone(), AcpSession { cwd, cancel_tx });
                vec![json!({
                    "jsonrpc": "2.0", "id": id,
                    "result": { "sessionId": session_id }
                })]
            },
            "session/prompt" => self.handle_prompt(id, &params).await,
            "shutdown" => {
                self.shutdown = true;
                vec![json!({ "jsonrpc": "2.0", "id": id, "result": null })]
            },
            other => vec![json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {other}") }
            })],
        }
    }

    fn cancel_session(&self, params: &Value) {
        if let Some(sid) = params.get("sessionId").and_then(Value::as_str) {
            // send_replace, not send: the value must stick even when no
            // turn is in flight (zero receivers would make `send` a
            // silent no-op, so a pre-turn cancel would be lost).
            if let Some(session) = self.sessions.get(sid) {
                session.cancel_tx.send_replace(true);
            }
        }
    }

    /// Extract the text blocks of a prompt; error string when none.
    fn prompt_text(params: &Value) -> Result<String, Value> {
        let text: Vec<&str> = params
            .get("prompt")
            .and_then(Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                    .filter_map(|b| b.get("text").and_then(Value::as_str))
                    .collect()
            })
            .unwrap_or_default();
        let joined = text.join("\n");
        if joined.trim().is_empty() {
            Err(json!({
                "code": -32602,
                "message": "session/prompt requires at least one text content block"
            }))
        } else {
            Ok(joined)
        }
    }

    async fn handle_prompt(&mut self, id: Value, params: &Value) -> Vec<Value> {
        let Some(sid) = params
            .get("sessionId")
            .and_then(Value::as_str)
            .map(String::from)
        else {
            return vec![Self::error(
                &id,
                -32602,
                "session/prompt requires sessionId",
            )];
        };
        if !self.sessions.contains_key(&sid) {
            return vec![Self::error(
                &id,
                -32602,
                &format!("unknown sessionId: {sid}"),
            )];
        }
        let task = match Self::prompt_text(params) {
            Ok(t) => t,
            Err(err) => {
                return vec![Self::error(
                    &id,
                    err["code"].as_i64().unwrap_or(-32602),
                    err["message"].as_str().unwrap_or("invalid params"),
                )]
            },
        };

        let cancel_tx = self.sessions.get(&sid).map(|s| s.cancel_tx.clone());
        let Some(cancel_tx) = cancel_tx else {
            return vec![Self::error(&id, -32602, "session vanished")];
        };
        let client = match self.resolve_client() {
            Ok(c) => c,
            Err(err) => {
                return vec![Self::error(
                    &id,
                    err["code"].as_i64().unwrap_or(-32000),
                    err["message"].as_str().unwrap_or("provider error"),
                )]
            },
        };

        // Observational events -> session/update notifications.
        let updates: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_updates = Arc::clone(&updates);
        let sink_sid = sid.clone();
        let engine = clawdius_core::tools::subagent::SubagentTool::new(
            client,
            crate::cli::auto_tools::AutoToolHost::tool_definitions(),
            {
                // Refuse tools after cancellation (belt alongside select!).
                let cancelled = cancel_tx.subscribe();
                let host = Arc::new(crate::cli::auto_tools::AutoToolHost::new(
                    std::env::current_dir().unwrap_or_default(),
                ));
                Arc::new(move |name: &str, args: &str| {
                    if *cancelled.borrow() {
                        return (
                            "cancelled: the client interrupted this turn".to_string(),
                            true,
                        );
                    }
                    host.execute(name, args)
                })
            },
            clawdius_core::tools::subagent::SubagentConfig::default(),
        )
        .with_event_sink(Arc::new(move |event| {
            let update = match event {
                clawdius_core::tools::subagent::EngineEvent::ModelTurn { text } if !text.is_empty() => {
                    json!({
                        "sessionUpdate": "agent_message_chunk",
                        "content": { "type": "text", "text": text }
                    })
                },
                clawdius_core::tools::subagent::EngineEvent::ToolCallStart { id, name, args } => {
                    json!({
                        "sessionUpdate": "tool_call",
                        "toolCallId": id,
                        "title": format!("{name} {args}"),
                        "kind": acp_tool_kind(&name),
                        "status": "in_progress"
                    })
                },
                clawdius_core::tools::subagent::EngineEvent::ToolCallEnd { id, output, is_error, .. } => {
                    json!({
                        "sessionUpdate": "tool_call_update",
                        "toolCallId": id,
                        "status": if is_error { "failed" } else { "completed" },
                        "content": [{
                            "type": "content",
                            "content": { "type": "text", "text": output }
                        }]
                    })
                },
                clawdius_core::tools::subagent::EngineEvent::StrategyBounce => {
                    json!({
                        "sessionUpdate": "agent_message_chunk",
                        "content": { "type": "text",
                            "text": "Repeating the same approach without progress — changing strategy." }
                    })
                },
                clawdius_core::tools::subagent::EngineEvent::ModelTurn { .. } => return,
            };
            if let Ok(mut q) = sink_updates.lock() {
                q.push(json!({
                    "jsonrpc": "2.0",
                    "method": "session/update",
                    "params": { "sessionId": sink_sid, "update": update }
                }));
            }
        }));

        // Drive the turn; cancellation wins at await boundaries and
        // applies even if the flag was set before the turn started.
        let mut cancelled = cancel_tx.subscribe();
        let outcome: Option<clawdius_core::Result<String>> = {
            let mut run = Box::pin(engine.run(&task));
            loop {
                if *cancelled.borrow() {
                    break None;
                }
                tokio::select! {
                    res = &mut run => break Some(res),
                    _ = cancelled.changed() => {
                        if *cancelled.borrow() {
                            break None;
                        }
                    },
                }
            }
        };

        let mut out = Vec::new();
        if let Ok(q) = updates.lock() {
            out.extend(q.iter().cloned());
        }
        match outcome {
            Some(Ok(summary)) => {
                // The final summary is usually the same text the last
                // ModelTurn already streamed — skip the duplicate chunk.
                let already_streamed = out.iter().rev().any(|m| {
                    m["params"]["update"]["sessionUpdate"] == "agent_message_chunk"
                        && m["params"]["update"]["content"]["text"] == summary.as_str()
                });
                if !already_streamed && !summary.trim().is_empty() {
                    out.push(json!({
                        "jsonrpc": "2.0", "method": "session/update",
                        "params": { "sessionId": sid, "update": {
                            "sessionUpdate": "agent_message_chunk",
                            "content": { "type": "text", "text": summary }
                        }}
                    }));
                }
                out.push(json!({
                    "jsonrpc": "2.0", "id": id,
                    "result": { "stopReason": "end_turn" }
                }));
            },
            Some(Err(e)) => out.push(Self::error(
                &id,
                -32000,
                &format!("prompt turn failed: {e}"),
            )),
            None => out.push(json!({
                "jsonrpc": "2.0", "id": id,
                "result": { "stopReason": "cancelled" }
            })),
        }
        out
    }

    fn error(id: &Value, code: i64, message: &str) -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
    }
}

/// Map engine tool names onto ACP tool kinds.
fn acp_tool_kind(name: &str) -> &'static str {
    match name {
        "read" | "glob" => "read",
        "edit" | "write" => "edit",
        "grep" | "web_search" => "search",
        "web_fetch" => "fetch",
        "shell" | "git" => "execute",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clawdius_core::llm::providers::{Tool, ToolCall};
    use clawdius_core::llm::{ChatMessage, ChatWithToolsResult};
    use std::sync::Mutex as StdMutex;

    struct ScriptedClient {
        turns: StdMutex<Vec<ChatWithToolsResult>>,
    }

    #[async_trait::async_trait]
    impl LlmClient for ScriptedClient {
        async fn chat(&self, _messages: Vec<ChatMessage>) -> clawdius_core::Result<String> {
            Ok("done".to_string())
        }
        async fn chat_with_tools(
            &self,
            _messages: Vec<ChatMessage>,
            _tools: Vec<Tool>,
        ) -> clawdius_core::Result<ChatWithToolsResult> {
            let mut turns = self.turns.lock().unwrap();
            if turns.is_empty() {
                return Ok(ChatWithToolsResult {
                    text: "all done".to_string(),
                    tool_calls: Vec::new(),
                });
            }
            Ok(turns.remove(0))
        }
        async fn chat_stream(
            &self,
            _messages: Vec<ChatMessage>,
        ) -> clawdius_core::Result<tokio::sync::mpsc::Receiver<String>> {
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            drop(tx);
            Ok(rx)
        }
        fn count_tokens(&self, _text: &str) -> usize {
            0
        }
    }

    fn client_with_tool_turn() -> Arc<dyn LlmClient> {
        Arc::new(ScriptedClient {
            turns: StdMutex::new(vec![ChatWithToolsResult {
                text: "checking".to_string(),
                tool_calls: vec![ToolCall {
                    call_id: "t1".to_string(),
                    fn_name: "grep".to_string(),
                    fn_arguments: serde_json::json!({"pattern": "x"}),
                    thought_signatures: None,
                }],
            }]),
        })
    }

    fn text_turn(text: &str) -> ChatWithToolsResult {
        ChatWithToolsResult {
            text: text.to_string(),
            tool_calls: Vec::new(),
        }
    }

    #[tokio::test]
    async fn initialize_negotiates_v1() {
        let mut server = AcpServer::new(Some(client_with_tool_turn()), None);
        let msg = json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": { "protocolVersion": 1, "clientCapabilities": {}, "clientInfo": {"name": "zed", "version": "1"} }
        });
        let out = server.handle(&msg).await;
        assert_eq!(out.len(), 1);
        let result = &out[0]["result"];
        assert_eq!(result["protocolVersion"], 1);
        assert_eq!(result["agentInfo"]["name"], "clawdius");
    }

    #[tokio::test]
    async fn session_new_and_unknown_method() {
        let mut server = AcpServer::new(Some(client_with_tool_turn()), None);
        let out = server
            .handle(&json!({"jsonrpc":"2.0","id":1,"method":"session/new","params":{"cwd":"/tmp"}}))
            .await;
        let sid = out[0]["result"]["sessionId"].as_str().unwrap().to_string();
        assert!(sid.starts_with("sess_"));
        let out = server
            .handle(&json!({"jsonrpc":"2.0","id":2,"method":"nope"}))
            .await;
        assert_eq!(out[0]["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn prompt_emits_updates_and_stop_reason() {
        // Scripted: one tool turn, then a plain turn; the plain `chat`
        // fallback serves the final text.
        let client = Arc::new(ScriptedClient {
            turns: StdMutex::new(vec![ChatWithToolsResult {
                text: "checking".to_string(),
                tool_calls: vec![ToolCall {
                    call_id: "t1".to_string(),
                    fn_name: "read".to_string(),
                    fn_arguments: serde_json::json!({"path": "x"}),
                    thought_signatures: None,
                }],
            }]),
        });
        let mut server = AcpServer::new(Some(client), None);
        let out = server
            .handle(&json!({"jsonrpc":"2.0","id":1,"method":"session/new","params":{"cwd":"/tmp"}}))
            .await;
        let sid = out[0]["result"]["sessionId"].as_str().unwrap().to_string();

        let prompt = json!({
            "jsonrpc":"2.0","id":2,"method":"session/prompt",
            "params": {"sessionId": sid, "prompt": [{"type":"text","text":"do the thing"}]}
        });
        let out = server.handle(&prompt).await;

        // Last message: the prompt response.
        let response = out.last().unwrap();
        assert_eq!(response["result"]["stopReason"], "end_turn");

        // Updates include the tool_call + completed update + final chunk.
        let methods: Vec<String> = out
            .iter()
            .filter(|m| m.get("method").is_some())
            .map(|m| {
                m["params"]["update"]["sessionUpdate"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert!(methods.contains(&"tool_call".to_string()));
        assert!(methods.contains(&"tool_call_update".to_string()));
        assert!(methods.contains(&"agent_message_chunk".to_string()));
    }

    #[tokio::test]
    async fn prompt_without_text_block_is_invalid_params() {
        let mut server = AcpServer::new(Some(client_with_tool_turn()), None);
        let out = server
            .handle(&json!({"jsonrpc":"2.0","id":1,"method":"session/new","params":{}}))
            .await;
        let sid = out[0]["result"]["sessionId"].as_str().unwrap().to_string();
        let out = server
            .handle(&json!({
                "jsonrpc":"2.0","id":2,"method":"session/prompt",
                "params": {"sessionId": sid, "prompt": [{"type":"image","data":"..."}]}
            }))
            .await;
        assert_eq!(out[0]["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn cancel_between_tools_reports_cancelled() {
        // A client that cancels before the turn: the turn must end with
        // stopReason cancelled.
        let client = Arc::new(ScriptedClient {
            turns: StdMutex::new(vec![
                ChatWithToolsResult {
                    text: "t".to_string(),
                    tool_calls: vec![ToolCall {
                        call_id: "t1".to_string(),
                        fn_name: "shell".to_string(),
                        fn_arguments: serde_json::json!({"command": "true"}),
                        thought_signatures: None,
                    }],
                },
                text_turn("more"),
                text_turn("even more"),
            ]),
        });
        let mut server = AcpServer::new(Some(client), None);
        let out = server
            .handle(&json!({"jsonrpc":"2.0","id":1,"method":"session/new","params":{"cwd":"/tmp"}}))
            .await;
        let sid = out[0]["result"]["sessionId"].as_str().unwrap().to_string();

        // Cancel before the turn runs.
        server
            .handle(&json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId": sid}}))
            .await;

        let prompt = json!({
            "jsonrpc":"2.0","id":2,"method":"session/prompt",
            "params": {"sessionId": sid, "prompt": [{"type":"text","text":"go"}]}
        });
        let out = server.handle(&prompt).await;
        assert_eq!(out.last().unwrap()["result"]["stopReason"], "cancelled");
    }
}
