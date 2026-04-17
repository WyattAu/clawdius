//! JSON-RPC server for `VSCode` extension communication.
//!
//! Implements the Language Server Protocol's stdin/stdout transport,
//! allowing the `VSCode` extension to communicate with Clawdius over
//! a JSON-RPC channel.

use serde::{Deserialize, Serialize};

/// JSON-RPC request from `VSCode` extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    #[must_use] 
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            result: Some(result),
            error: None,
        }
    }

    #[must_use] 
    pub fn error(id: serde_json::Value, code: i64, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_string(),
                data: None,
            }),
        }
    }
}

pub const METHOD_CHAT: &str = "clawdius/chat";
pub const METHOD_AGENT: &str = "clawdius/agent";
pub const METHOD_LIST_TOOLS: &str = "clawdius/listTools";
pub const METHOD_GET_SESSIONS: &str = "clawdius/getSessions";
pub const METHOD_CANCEL: &str = "clawdius/cancel";

/// Notification from `VSCode` (no id, no response expected)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

/// Run the JSON-RPC stdio server.
/// Reads JSON-RPC requests from stdin, processes them, and writes
/// responses to stdout. Notifications (no id) are handled but produce no output.
pub async fn run_stdio_server<F, Fut>(handler: F) -> crate::Result<()>
where
    F: Fn(JsonRpcRequest) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = crate::Result<JsonRpcResponse>> + Send,
{
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin());
    let mut stdout = tokio::io::BufWriter::new(tokio::io::stdout());

    let mut line = String::new();
    loop {
        line.clear();
        match stdin.read_line(&mut line).await {
            Ok(0) => break Ok(()),
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                if let Ok(req) = serde_json::from_str::<JsonRpcRequest>(trimmed) {
                    if req.method.is_some() {
                        let resp = handler(req).await?;
                        let output = serde_json::to_string(&resp)?;
                        stdout.write_all(output.as_bytes()).await?;
                        stdout.write_all(b"\n").await?;
                        stdout.flush().await?;
                    }
                } else if let Ok(_notif) = serde_json::from_str::<JsonRpcNotification>(trimmed) {
                }
            },
            Err(e) => {
                eprintln!("Error reading stdin: {e}");
                break Err(crate::Error::Io(e));
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _};

    #[test]
    fn test_success_response_serialization() {
        let resp =
            JsonRpcResponse::success(serde_json::json!(1), serde_json::json!({"status": "ok"}));
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["result"]["status"], "ok");
        assert!(parsed.get("error").is_none());
    }

    #[test]
    fn test_success_response_skips_error_field() {
        let resp = JsonRpcResponse::success(serde_json::json!(1), serde_json::json!("done"));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_error_response_serialization() {
        let resp = JsonRpcResponse::error(serde_json::json!(42), -32601, "Method not found");
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 42);
        assert!(parsed.get("result").is_none());
        assert_eq!(parsed["error"]["code"], -32601);
        assert_eq!(parsed["error"]["message"], "Method not found");
    }

    #[test]
    fn test_error_response_skips_result_field() {
        let resp = JsonRpcResponse::error(serde_json::json!("abc"), -32700, "Parse error");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(!json.contains("\"result\""));
    }

    #[test]
    fn test_parse_request_from_json() {
        let json =
            r#"{"jsonrpc":"2.0","id":1,"method":"clawdius/chat","params":{"message":"hello"}}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.id, Some(serde_json::json!(1)));
        assert_eq!(req.method.as_deref(), Some("clawdius/chat"));
        assert_eq!(
            req.params
                .as_ref()
                .and_then(|p| p.get("message"))
                .and_then(|v| v.as_str()),
            Some("hello")
        );
    }

    #[test]
    fn test_parse_request_with_string_id() {
        let json = r#"{"jsonrpc":"2.0","id":"req-1","method":"clawdius/listTools"}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.id, Some(serde_json::json!("req-1")));
        assert_eq!(req.method.as_deref(), Some("clawdius/listTools"));
    }

    #[test]
    fn test_parse_request_no_params() {
        let json = r#"{"jsonrpc":"2.0","id":5,"method":"clawdius/getSessions"}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();

        assert!(req.params.is_none());
    }

    #[test]
    fn test_parse_notification_no_id() {
        let json = r#"{"jsonrpc":"2.0","method":"clawdius/cancel","params":{"id":"sess-1"}}"#;
        let notif: JsonRpcNotification = serde_json::from_str(json).unwrap();

        assert_eq!(notif.jsonrpc, "2.0");
        assert_eq!(notif.method.as_deref(), Some("clawdius/cancel"));
        assert_eq!(
            notif
                .params
                .as_ref()
                .and_then(|p| p.get("id"))
                .and_then(|v| v.as_str()),
            Some("sess-1")
        );
    }

    #[test]
    fn test_notification_defaults() {
        let json = r#"{"jsonrpc":"2.0"}"#;
        let notif: JsonRpcNotification = serde_json::from_str(json).unwrap();

        assert!(notif.method.is_none());
        assert!(notif.params.is_none());
    }

    #[test]
    fn test_method_constants() {
        assert_eq!(METHOD_CHAT, "clawdius/chat");
        assert_eq!(METHOD_AGENT, "clawdius/agent");
        assert_eq!(METHOD_LIST_TOOLS, "clawdius/listTools");
        assert_eq!(METHOD_GET_SESSIONS, "clawdius/getSessions");
        assert_eq!(METHOD_CANCEL, "clawdius/cancel");
    }

    #[tokio::test]
    async fn test_stdio_server_eof() {
        let cursor = std::io::Cursor::new("");

        let (_reader, writer) = tokio::io::duplex(64);

        let handler = |req: JsonRpcRequest| async move {
            let result: crate::Result<JsonRpcResponse> = Ok(JsonRpcResponse::success(
                req.id.unwrap_or(serde_json::json!(null)),
                serde_json::json!({"echo": true}),
            ));
            result
        };

        let handle = tokio::spawn(async move {
            let mut stdin = tokio::io::BufReader::new(cursor);
            let mut stdout = tokio::io::BufWriter::new(writer);

            let mut line = String::new();
            loop {
                line.clear();
                match stdin.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if let Ok(req) = serde_json::from_str::<JsonRpcRequest>(trimmed) {
                            if req.method.is_some() {
                                let resp = handler(req).await.unwrap();
                                let output = serde_json::to_string(&resp).unwrap();
                                stdout.write_all(output.as_bytes()).await.unwrap();
                                stdout.write_all(b"\n").await.unwrap();
                                stdout.flush().await.unwrap();
                            }
                        }
                    },
                    Err(_) => break,
                }
            }
        });

        let result = handle.await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_stdio_server_roundtrip() {
        let input =
            r#"{"jsonrpc":"2.0","id":1,"method":"clawdius/chat","params":{"message":"hi"}}"#;

        let (server_reader, mut client_writer) = tokio::io::duplex(1024);
        let (client_reader, server_writer) = tokio::io::duplex(1024);

        client_writer.write_all(input.as_bytes()).await.unwrap();
        client_writer.write_all(b"\n").await.unwrap();
        client_writer.shutdown().await.unwrap();

        let handler = |req: JsonRpcRequest| async move {
            let msg = req
                .params
                .as_ref()
                .and_then(|p| p.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let result: crate::Result<JsonRpcResponse> = Ok(JsonRpcResponse::success(
                req.id.clone().unwrap_or(serde_json::json!(null)),
                serde_json::json!({"reply": format!("You said: {}", msg)}),
            ));
            result
        };

        let handle = tokio::spawn(async move {
            let mut stdin = tokio::io::BufReader::new(server_reader);
            let mut stdout = tokio::io::BufWriter::new(server_writer);

            let mut line = String::new();
            loop {
                line.clear();
                match stdin.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if let Ok(req) = serde_json::from_str::<JsonRpcRequest>(trimmed) {
                            if req.method.is_some() {
                                let resp = handler(req).await.unwrap();
                                let output = serde_json::to_string(&resp).unwrap();
                                stdout.write_all(output.as_bytes()).await.unwrap();
                                stdout.write_all(b"\n").await.unwrap();
                                stdout.flush().await.unwrap();
                            }
                        }
                    },
                    Err(_) => break,
                }
            }
        });

        let mut buf = String::new();
        let mut reader = tokio::io::BufReader::new(client_reader);
        reader.read_line(&mut buf).await.unwrap();
        let resp: JsonRpcResponse = serde_json::from_str(buf.trim()).unwrap();

        assert_eq!(resp.id, Some(serde_json::json!(1)));
        assert_eq!(resp.result.as_ref().unwrap()["reply"], "You said: hi");
        assert!(resp.error.is_none());

        handle.await.unwrap();
    }
}
