//! Streamable-HTTP transport for remote MCP servers. Every message is a
//! JSON-RPC POST with `Accept: application/json, text/event-stream`; the
//! server answers with plain JSON or an SSE stream, and a
//! `Mcp-Session-Id` response header must be echoed on later calls.
//!
//! Scope: request/response only. No client-to-server streams and no
//! long-lived listening streams — the bridge's call model is one tool
//! call in, one result out, which a single response serves.

use super::protocol::{McpTool, McpToolResult, MCP_VERSION};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

const CLIENT_NAME: &str = "clawdius";

/// A client for one remote MCP server over streamable HTTP.
pub struct HttpMcpClient {
    url: String,
    http: reqwest::Client,
    session: Mutex<Option<String>>,
    next_id: AtomicU64,
}

impl HttpMcpClient {
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            http: reqwest::Client::new(),
            session: Mutex::new(None),
            next_id: AtomicU64::new(1),
        }
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// List the server's tools (performs the initialize handshake on
    /// first use).
    ///
    /// # Errors
    ///
    /// Transport failures, non-2xx responses, and unparseable bodies.
    pub async fn list_tools(&self) -> Result<Vec<McpTool>, String> {
        let result = self.rpc("tools/list", json!({})).await?;
        serde_json::from_value(result)
            .map(|v: ToolsList| v.tools)
            .map_err(|e| format!("tools/list result shape: {e}"))
    }

    /// Call one tool; the text content of the result is the output.
    ///
    /// # Errors
    ///
    /// Same as [`Self::list_tools`], plus tool-level `isError` results.
    pub async fn call_tool(&self, tool: &str, arguments: Value) -> Result<McpToolResult, String> {
        let result = self
            .rpc("tools/call", json!({"name": tool, "arguments": arguments}))
            .await?;
        serde_json::from_value(result).map_err(|e| format!("tools/call result shape: {e}"))
    }

    /// One JSON-RPC round trip. The initialize handshake happens
    /// transparently before the first non-initialize call.
    async fn rpc(&self, method: &str, params: Value) -> Result<Value, String> {
        if self.session.lock().await.is_none() && method != "initialize" {
            self.handshake().await?;
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let response = self
            .post_message(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params,
            }))
            .await?;
        let value = parse_sse_or_json(&response, Some(id))
            .ok_or_else(|| String::from("response carried no parseable JSON-RPC message"))?;
        if let Some(err) = value.get("error") {
            return Err(format!("server error: {err}"));
        }
        value
            .get("result")
            .cloned()
            .ok_or_else(|| String::from("response had no result"))
    }

    /// initialize + the `notifications/initialized` notification.
    async fn handshake(&self) -> Result<(), String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let response = self
            .post_message(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "initialize",
                "params": {
                    "protocolVersion": MCP_VERSION,
                    "capabilities": {},
                    "clientInfo": {"name": CLIENT_NAME, "version": env!("CARGO_PKG_VERSION")},
                },
            }))
            .await?;
        if parse_sse_or_json(&response, Some(id)).is_none() {
            return Err(String::from("initialize response unparseable"));
        }
        // The notification carries no id and usually answers 202 Accepted
        // with an empty body; a parse miss is fine here.
        let _ = self
            .post_message(&json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized",
            }))
            .await;
        Ok(())
    }

    async fn post_message(&self, message: &Value) -> Result<String, String> {
        let mut request = self
            .http
            .post(&self.url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .json(message);
        if let Some(session) = self.session.lock().await.as_ref() {
            request = request.header("Mcp-Session-Id", session);
        }
        let response = request
            .send()
            .await
            .map_err(|e| format!("POST {}: {e}", self.url))?;
        let status = response.status();
        if let Some(session) = response
            .headers()
            .get("Mcp-Session-Id")
            .and_then(|v| v.to_str().ok())
        {
            *self.session.lock().await = Some(session.to_string());
        }
        let body = response
            .text()
            .await
            .map_err(|e| format!("reading response body: {e}"))?;
        // 202 Accepted is the spec's answer to notifications: success with
        // no body to parse.
        if !status.is_success() && status.as_u16() != 202 {
            return Err(format!("HTTP {status}: {}", first_line(&body)));
        }
        Ok(body)
    }
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("")
}

/// Extract the JSON-RPC message from a response body: plain JSON or an
/// SSE stream of `data:` lines. `id` selects the matching response when
/// several arrive on one stream; notifications (no id) match any.
#[must_use]
pub fn parse_sse_or_json(body: &str, id: Option<u64>) -> Option<Value> {
    let trimmed = body.trim_start();
    if !trimmed.starts_with("event:") && !trimmed.starts_with("data:") {
        return serde_json::from_str(trimmed).ok();
    }
    for data in sse_data_lines(trimmed) {
        let Ok(value) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        match (id, value.get("id").and_then(Value::as_u64)) {
            (Some(want), Some(got)) if want != got => continue,
            _ => return Some(value),
        }
    }
    None
}

/// The payloads of every `data:` line in an SSE body, with the `data: `
/// prefix stripped (the prefix may appear with or without a trailing
/// space per the SSE spec).
fn sse_data_lines(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in body.lines() {
        let Some(rest) = line.strip_prefix("data:") else {
            continue;
        };
        out.push(rest.strip_prefix(' ').unwrap_or(rest).to_string());
    }
    out
}

/// The payload shape of a tools/list result.
#[derive(serde::Deserialize)]
struct ToolsList {
    tools: Vec<McpTool>,
}

#[cfg(test)]
mod tests {
    use super::parse_sse_or_json;

    #[test]
    fn plain_json_bodies_parse_directly() {
        let body = r#"{"jsonrpc":"2.0","id":3,"result":{"tools":[]}}"#;
        let value = parse_sse_or_json(body, Some(3)).expect("plain json");
        assert_eq!(value["id"], 3);
    }

    #[test]
    fn sse_bodies_yield_their_data_lines() {
        let body = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n\n\
                    event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{}}\n";
        assert!(parse_sse_or_json(body, Some(1)).is_some());
        assert!(parse_sse_or_json(body, Some(2)).is_some());
        // The stream for another session's id does not match ours.
        assert!(parse_sse_or_json(body, Some(9)).is_none());
    }

    #[test]
    fn notifications_and_garbage_do_not_match() {
        assert!(parse_sse_or_json("data: {\"method\":\"x\"}", Some(1)).is_some());
        assert!(parse_sse_or_json("data: not json", Some(1)).is_none());
        assert!(parse_sse_or_json("", Some(1)).is_none());
    }

    /// Live probe: the real context7 endpoint must handshake, list tools,
    /// and answer. Ignored by default (network).
    #[test]
    #[ignore]
    fn live_context7_connects_and_lists_tools() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(async {
            let (client, tools) = super::remote_ready("context7", "https://mcp.context7.com/mcp")
                .await
                .expect("context7 connect");
            assert!(!tools.is_empty(), "context7 exposes tools");
            assert!(client.url().starts_with("https://"));
        });
    }
}

/// Register a remote server on the fleet: list its tools and return a
/// call handle plus the bridge-facing tool list.
///
/// # Errors
///
/// Transport failures, non-2xx responses, unparseable bodies.
pub async fn remote_ready(
    name: &str,
    url: &str,
) -> Result<(Arc<HttpMcpClient>, Vec<McpTool>), String> {
    let _ = name;
    let client = Arc::new(HttpMcpClient::new(url));
    let tools = client.list_tools().await?;
    Ok((client, tools))
}
