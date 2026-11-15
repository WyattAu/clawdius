//! Bridge opencode's MCP fleet into clawdius tool loops.
//!
//! opencode runs ~17 MCP servers configured in
//! `~/.config/opencode/opencode.jsonc`; clawdius loops used to see none of
//! them (a fixed 12-tool surface). This module reads that same config,
//! launches the **local stdio** servers, lists their tools, and exposes
//! them to the loop under `mcp__<server>__<tool>` names.
//!
//! Threading model: MCP clients are async (tokio), but the loop's tool
//! executor is a sync `Fn(&str, &str) -> (String, bool)`. The bridge owns
//! a dedicated worker thread running its own current-thread tokio
//! runtime; calls are shipped over an mpsc channel and answered with a
//! timeout. This works from any runtime flavor (the CLI builds a plain
//! `Runtime`, the TUI its own) without `block_in_place`.
//!
//! Scope (v1): local stdio servers only — remote (`url`) servers are
//! skipped until an HTTP transport exists. A server that fails to launch
//! or initialize is skipped with the reason recorded, never fatal: the
//! loop keeps its built-in tools regardless.

use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use super::client::{McpClient, StdioTransport};

/// Per-call timeout. A hung server costs one tool call, not the cycle.
const CALL_TIMEOUT: Duration = Duration::from_secs(30);

/// Startup budget for the whole fleet connect (sequential, ~0.3s/server).
const STARTUP_TIMEOUT: Duration = Duration::from_secs(60);

/// Per-server connect budget: a hung server is skipped, not fatal.
const SERVER_CONNECT_TIMEOUT: Duration = Duration::from_secs(8);

/// Default config: opencode's fleet is the parity target.
const DEFAULT_CONFIG: &str = ".config/opencode/opencode.jsonc";

/// One tool exposed to the loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeTool {
    /// Fully-qualified name: `mcp__<server>__<tool>`.
    pub name: String,
    /// Server tool description, verbatim.
    pub description: String,
    /// JSON Schema for arguments, verbatim.
    pub input_schema: Value,
}

/// A parsed server entry from the opencode config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerSpec {
    /// Server name as configured.
    pub name: String,
    /// Program to launch.
    pub command: String,
    /// Program arguments.
    pub args: Vec<String>,
    /// Extra environment for the subprocess.
    pub env: Vec<(String, String)>,
}

/// A running bridge to opencode's MCP fleet.
pub struct McpBridge {
    tx: Option<mpsc::Sender<Job>>,
    tools: Vec<BridgeTool>,
    servers: Vec<String>,
    skipped: Vec<String>,
}

struct Job {
    server: String,
    tool: String,
    args: Value,
    reply: mpsc::Sender<(String, bool)>,
}

impl McpBridge {
    /// Build a bridge from the environment:
    ///
    /// - `CLAWDIUS_MCP=off` disables the bridge entirely;
    /// - `CLAWDIUS_MCP_SERVERS=a,b` restricts to those servers;
    /// - `CLAWDIUS_MCP_CONFIG` overrides the config path.
    ///
    /// `Err` means the config could not be parsed (surfaced by the
    /// caller); missing config yields an empty bridge, not an error.
    pub fn from_env() -> Result<Option<Self>, String> {
        if std::env::var("CLAWDIUS_MCP").as_deref() == Ok("off") {
            return Ok(None);
        }
        let path = match std::env::var("CLAWDIUS_MCP_CONFIG") {
            Ok(p) => std::path::PathBuf::from(p),
            Err(_) => dirs_home().map_or_else(
                || std::path::PathBuf::from(DEFAULT_CONFIG),
                |h| h.join(DEFAULT_CONFIG),
            ),
        };
        if !path.is_file() {
            return Ok(None);
        }
        let only = std::env::var("CLAWDIUS_MCP_SERVERS").ok().map(|s| {
            s.split(',')
                .map(str::trim)
                .map(String::from)
                .collect::<Vec<_>>()
        });
        Self::from_config(&path, only.as_deref()).map(Some)
    }

    /// Connect to every enabled local stdio server in an opencode config.
    pub fn from_config(path: &Path, only: Option<&[String]>) -> Result<Self, String> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let specs = parse_opencode_config(&raw)?;
        let specs: Vec<ServerSpec> = match only {
            None => specs,
            Some(names) => specs
                .into_iter()
                .filter(|s| names.iter().any(|n| n == &s.name))
                .collect(),
        };
        Ok(Self::connect(&specs))
    }

    /// Launch the worker thread and connect every spec. Skips (with
    /// reasons) are recorded; a fully-skipped fleet is still a working,
    /// if empty, bridge.
    #[must_use]
    pub fn connect(specs: &[ServerSpec]) -> Self {
        let specs = specs.to_vec();
        let (ready_tx, ready_rx) =
            mpsc::channel::<Result<(Vec<BridgeTool>, Vec<String>, Vec<String>), String>>();
        let (tx, rx) = mpsc::channel::<Job>();
        let spawned = std::thread::Builder::new()
            .name("clawdius-mcp-bridge".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = ready_tx.send(Err(format!("runtime: {e}")));
                        return;
                    },
                };
                let (tools, servers, skipped, clients) = rt.block_on(fleet_ready(&specs));
                if ready_tx
                    .send(Ok((tools, servers.clone(), skipped.clone())))
                    .is_err()
                {
                    return;
                }
                // Serve calls until every sender drops.
                for job in rx {
                    let answer = match clients.get(&job.server) {
                        None => (format!("no such MCP server: {}", job.server), true),
                        Some(client) => rt
                            .block_on(async {
                                match tokio::time::timeout(
                                    CALL_TIMEOUT,
                                    client.call_tool(&job.tool, job.args),
                                )
                                .await
                                {
                                    Ok(Ok(result)) => Ok(format_mcp_result(&result)),
                                    Ok(Err(e)) => Err(format!("mcp {}: {e}", job.server)),
                                    Err(_) => Err(format!(
                                        "mcp {}: timed out after {}s",
                                        job.server,
                                        CALL_TIMEOUT.as_secs()
                                    )),
                                }
                            })
                            .unwrap_or_else(|e| (e, true)),
                    };
                    let _ = job.reply.send(answer);
                }
            });
        if spawned.is_err() {
            return Self {
                tx: None,
                tools: Vec::new(),
                servers: Vec::new(),
                skipped: vec![String::from("bridge thread failed to spawn")],
            };
        }
        let (tools, servers, skipped) = match ready_rx.recv_timeout(STARTUP_TIMEOUT) {
            Ok(Ok((tools, servers, skipped))) => (tools, servers, skipped),
            Ok(Err(e)) => (Vec::new(), Vec::new(), vec![format!("connect failed: {e}")]),
            Err(e) => (
                Vec::new(),
                Vec::new(),
                vec![format!("fleet connect timed out: {e}")],
            ),
        };
        Self {
            tx: Some(tx),
            tools,
            servers,
            skipped,
        }
    }

    /// Tool definitions for the loop, `mcp__`-prefixed.
    ///
    /// Schemas are sanitized to Google's function-declaration subset:
    /// the API rejects valid-JSON-Schema keys like `additionalProperties`
    /// with a hard 400, so anything beyond {type, format, description,
    /// nullable, enum, items, properties, required} is stripped. Stripping
    /// is harmless for permissive providers.
    #[must_use]
    pub fn tool_definitions(&self) -> Vec<crate::llm::providers::Tool> {
        self.tools
            .iter()
            .map(|t| {
                crate::llm::providers::Tool::new(t.name.clone())
                    .with_description(t.description.clone())
                    .with_schema(sanitize_schema(&t.input_schema))
            })
            .collect()
    }

    /// Execute a bridged tool call. Sync by design: this is the shape the
    /// loop's `SubToolExecutor` wants. Always returns `(output, is_error)`.
    pub fn execute(&self, name: &str, arguments: &str) -> (String, bool) {
        let Some((server, tool)) = split_prefixed(name) else {
            return (
                format!("not an mcp tool name (want mcp__<server>__<tool>): {name}"),
                true,
            );
        };
        let args: Value = if arguments.trim().is_empty() {
            Value::Object(serde_json::Map::new())
        } else {
            match serde_json::from_str(arguments) {
                Ok(v) => v,
                Err(e) => return (format!("invalid tool arguments: {e}"), true),
            }
        };
        let Some(tx) = &self.tx else {
            return (String::from("bridge is not serving calls"), true);
        };
        let (reply_tx, reply_rx) = mpsc::channel();
        let job = Job {
            server: server.to_string(),
            tool: tool.to_string(),
            args,
            reply: reply_tx,
        };
        if tx.send(job).is_err() {
            return (String::from("bridge worker is gone"), true);
        }
        let margin = Duration::from_secs(5);
        match reply_rx.recv_timeout(CALL_TIMEOUT + margin) {
            Ok(answer) => answer,
            Err(mpsc::RecvTimeoutError::Timeout) => (
                format!(
                    "bridge call timed out after {}s",
                    (CALL_TIMEOUT + margin).as_secs()
                ),
                true,
            ),
            Err(_) => (String::from("bridge worker dropped the reply"), true),
        }
    }

    /// Connected server names.
    #[must_use]
    pub fn servers(&self) -> &[String] {
        &self.servers
    }

    /// Skipped servers with reasons (launch failure, remote, disabled by
    /// an explicit subset).
    #[must_use]
    pub fn skipped(&self) -> &[String] {
        &self.skipped
    }

    /// One-line description for cycle logs.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.servers.is_empty() {
            format!(
                "no MCP servers{}",
                if self.skipped.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", self.skipped.join("; "))
                }
            )
        } else {
            format!(
                "{} server(s), {} tool(s){}",
                self.servers.len(),
                self.tools.len(),
                if self.skipped.is_empty() {
                    String::new()
                } else {
                    format!(", skipped: {}", self.skipped.join("; "))
                }
            )
        }
    }
}

impl Drop for McpBridge {
    fn drop(&mut self) {
        // Dropping `tx` ends the worker's job loop; the worker's runtime
        // and clients drop with the thread, and `kill_on_drop` reaps the
        // server subprocesses.
        self.tx.take();
    }
}

/// Connect to every spec and collect the fleet's tool catalog, the live
/// clients (for `tools/call`), and per-server skip reasons.
async fn fleet_ready(
    specs: &[ServerSpec],
) -> (
    Vec<BridgeTool>,
    Vec<String>,
    Vec<String>,
    HashMap<String, Arc<McpClient>>,
) {
    let mut tools = Vec::new();
    let mut servers = Vec::new();
    let mut skipped = Vec::new();
    let mut clients = HashMap::new();
    for spec in specs {
        let attempt = tokio::time::timeout(SERVER_CONNECT_TIMEOUT, connect_one(spec));
        match attempt.await {
            Ok(Ok((client, listed))) => {
                for tool in listed {
                    let description = if tool.description.is_empty() {
                        format!("(no description) {}", tool.name)
                    } else {
                        tool.description
                    };
                    tools.push(BridgeTool {
                        name: format!("mcp__{}__{}", spec.name, tool.name),
                        description,
                        input_schema: tool.input_schema,
                    });
                }
                servers.push(spec.name.clone());
                clients.insert(spec.name.clone(), client);
            },
            Ok(Err(e)) => skipped.push(format!("{}: {e}", spec.name)),
            Err(_) => skipped.push(format!(
                "{}: connect timed out after {}s",
                spec.name,
                SERVER_CONNECT_TIMEOUT.as_secs()
            )),
        }
    }
    (tools, servers, skipped, clients)
}

/// Launch and initialize one server; return the client and its tools.
async fn connect_one(
    spec: &ServerSpec,
) -> Result<(Arc<McpClient>, Vec<super::protocol::McpTool>), String> {
    let args: Vec<&str> = spec.args.iter().map(String::as_str).collect();
    let env: Vec<(&str, &str)> = spec
        .env
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let transport = Arc::new(
        StdioTransport::launch(&spec.command, &args, &env)
            .await
            .map_err(|e| format!("launch: {e}"))?,
    );
    let client = Arc::new(McpClient::new(spec.name.clone(), transport));
    client
        .initialize()
        .await
        .map_err(|e| format!("initialize: {e}"))?;
    let listed = client
        .list_tools()
        .await
        .map_err(|e| format!("tools/list: {e}"))?;
    Ok((client, listed))
}

/// Split `mcp__<server>__<tool>` into `(server, tool)`.
#[must_use]
pub fn split_prefixed(name: &str) -> Option<(&str, &str)> {
    let rest = name.strip_prefix("mcp__")?;
    let (server, tool) = rest.split_once("__")?;
    if server.is_empty() || tool.is_empty() {
        return None;
    }
    Some((server, tool))
}

/// Format an MCP tool result into loop-friendly text.
#[must_use]
pub fn format_mcp_result(result: &super::protocol::McpToolResult) -> (String, bool) {
    use super::protocol::McpContent;
    let mut parts = Vec::new();
    for content in &result.content {
        match content {
            McpContent::Text { text } => parts.push(text.clone()),
            McpContent::Image { mime_type, .. } => {
                parts.push(format!("[image: {mime_type}]"));
            },
            McpContent::Resource { uri, .. } => {
                parts.push(format!("[resource: {uri}]"));
            },
        }
    }
    (parts.join("\n"), result.is_error)
}

/// Parse the `mcp.servers` map of an opencode.jsonc into local stdio
/// specs. Disabled and remote entries are skipped (remote needs an HTTP
/// transport; noted, not fatal).
#[must_use]
pub fn parse_opencode_config(raw: &str) -> Result<Vec<ServerSpec>, String> {
    let value: Value = serde_json::from_str(&strip_jsonc(raw))
        .map_err(|e| format!("config is not valid JSON after comment strip: {e}"))?;
    let servers = value
        .get("mcp")
        .and_then(|m| m.get("servers"))
        .and_then(Value::as_object)
        .ok_or_else(|| String::from("config has no mcp.servers map"))?;

    let mut specs = Vec::new();
    for (name, entry) in servers {
        if entry.get("disabled").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let command = entry
            .get("command")
            .and_then(Value::as_array)
            .and_then(|c| c.first())
            .and_then(Value::as_str);
        let Some(program) = command else {
            continue; // remote or malformed: skip silently, the loop has its own tools
        };
        let args = entry
            .get("command")
            .and_then(Value::as_array)
            .map(|c| {
                c.iter()
                    .skip(1)
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        let env = entry
            .get("environment")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        specs.push(ServerSpec {
            name: name.clone(),
            command: program.to_string(),
            args,
            env,
        });
    }
    Ok(specs)
}

/// Google's function-declaration API accepts a strict JSON-Schema subset;
/// keys outside it are a hard 400 ("Unknown name \"additionalProperties\"").
#[must_use]
pub fn sanitize_schema(schema: &Value) -> Value {
    const ALLOWED: &[&str] = &[
        "type",
        "format",
        "description",
        "nullable",
        "enum",
        "items",
        "properties",
        "required",
    ];
    match schema {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, value) in map {
                if !ALLOWED.contains(&key.as_str()) {
                    continue;
                }
                let cleaned = match key.as_str() {
                    "properties" => Value::Object(
                        value
                            .as_object()
                            .map(|m| {
                                m.iter()
                                    .map(|(k, v)| (k.clone(), sanitize_schema(v)))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    ),
                    "items" => sanitize_schema(value),
                    _ => value.clone(),
                };
                out.insert(key.clone(), cleaned);
            }
            Value::Object(out)
        },
        other => other.clone(),
    }
}

/// Strip JSONC comments and trailing commas. String literals are
/// preserved (a `//` inside a string is content, not a comment).
#[must_use]
pub fn strip_jsonc(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let bytes: Vec<char> = raw.chars().collect();
    let mut i = 0;
    let mut in_string = false;
    let mut escaped = false;
    while i < bytes.len() {
        let c = bytes[i];
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
                i += 1;
            },
            '/' if i + 1 < bytes.len() && bytes[i + 1] == '/' => {
                while i < bytes.len() && bytes[i] != '\n' {
                    i += 1;
                }
            },
            '/' if i + 1 < bytes.len() && bytes[i + 1] == '*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == '*' && bytes[i + 1] == '/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
            },
            ',' => {
                // Trailing comma: peek past whitespace for } or ].
                let mut j = i + 1;
                while j < bytes.len() && bytes[j].is_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && (bytes[j] == '}' || bytes[j] == ']') {
                    i += 1; // drop the comma
                } else {
                    out.push(c);
                    i += 1;
                }
            },
            _ => {
                out.push(c);
                i += 1;
            },
        }
    }
    out
}

fn dirs_home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_jsonc_keeps_strings_and_kills_comments_and_trailing_commas() {
        let raw = r#"{
            // line comment
            "url": "https://x//not-a-comment", /* block,
                spanning */
            "list": [1, 2,],
            "obj": {"a": "b",},
        }"#;
        let stripped = strip_jsonc(raw);
        let v: Value = serde_json::from_str(&stripped).expect("stripped output must parse");
        assert_eq!(v["url"], "https://x//not-a-comment");
        assert_eq!(v["list"].as_array().unwrap().len(), 2);
        assert_eq!(v["obj"]["a"], "b");
        assert!(!stripped.contains("line comment"));
        assert!(!stripped.contains("spanning"));
    }

    #[test]
    fn parse_picks_local_enabled_servers_and_skips_remote_and_disabled() {
        let raw = r#"{
            "mcp": {"servers": {
                "local-one": {"type": "local", "command": ["python3", "serve.py"], "disabled": false,
                    "environment": {"KEY": "value"}},
                "context7": {"type": "remote", "url": "https://mcp.context7.com/mcp"},
                "sentinel": {"type": "local", "command": ["true"], "disabled": true}
            }}
        }"#;
        let specs = parse_opencode_config(raw).expect("parse");
        assert_eq!(specs.len(), 1, "remote and disabled are skipped");
        assert_eq!(specs[0].name, "local-one");
        assert_eq!(specs[0].command, "python3");
        assert_eq!(specs[0].args, vec!["serve.py".to_string()]);
        assert_eq!(specs[0].env, vec![("KEY".to_string(), "value".to_string())]);
    }

    #[test]
    fn prefixed_names_split_cleanly() {
        assert_eq!(
            split_prefixed("mcp__context7__resolve-library-id"),
            Some(("context7", "resolve-library-id"))
        );
        assert_eq!(split_prefixed("read_file"), None);
        assert_eq!(split_prefixed("mcp__no__"), None);
        assert_eq!(split_prefixed("mcp____tool"), None);
    }

    #[test]
    fn schemas_are_sanitized_to_googles_subset() {
        // Live: Google 400s on additionalProperties with "Unknown name".
        let raw = serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "minLength": 1, "default": "x"},
                "opts": {
                    "type": "object",
                    "additionalProperties": {"type": "string"},
                    "properties": {"depth": {"type": "integer", "title": "Depth"}}
                }
            },
            "required": ["path"],
            "additionalProperties": false,
            "$schema": "http://json-schema.org/draft-07/schema#"
        });
        let clean = sanitize_schema(&raw);
        let s = serde_json::to_string(&clean).unwrap();
        assert!(!s.contains("additionalProperties"));
        assert!(!s.contains("$schema"));
        assert!(!s.contains("minLength"));
        assert!(!s.contains("\"title\""));
        assert!(!s.contains("\"default\""));
        assert_eq!(clean["type"], "object");
        assert_eq!(clean["required"], serde_json::json!(["path"]));
        assert_eq!(clean["properties"]["path"]["type"], "string");
        assert_eq!(
            clean["properties"]["opts"]["properties"]["depth"]["type"],
            "integer"
        );
    }

    #[test]
    fn empty_bridge_reports_honest_summary() {
        let bridge = McpBridge::connect(&[]);
        assert_eq!(bridge.summary(), "no MCP servers");
        assert!(bridge.servers().is_empty());
        let (out, is_err) = bridge.execute("read_file", "{}");
        assert!(is_err, "built-in names are not bridged");
        assert!(out.contains("not an mcp tool name"));
        let (out, is_err) = bridge.execute("mcp__nope__tool", "{}");
        assert!(is_err);
        assert!(out.contains("no such MCP server"));
    }

    /// Live check against the host's real opencode fleet. Run with
    /// `--ignored --nocapture`. Skips itself when the config is absent
    /// (CI, containers).
    #[test]
    #[ignore = "live: launches the host's real MCP servers"]
    fn live_fleet_connects_and_answers() {
        let Some(home) = std::env::var_os("HOME") else {
            return;
        };
        let path = std::path::PathBuf::from(home).join(DEFAULT_CONFIG);
        if !path.is_file() {
            return;
        }
        let bridge = McpBridge::from_config(&path, Some(&["filesystem-kit".into()]))
            .expect("config must parse");
        println!("summary: {}", bridge.summary());
        assert!(bridge.servers().contains(&"filesystem-kit".to_string()));
        assert!(bridge
            .tools
            .iter()
            .any(|t| t.name.starts_with("mcp__filesystem-kit__")));
        let list_dir = bridge
            .tools
            .iter()
            .find(|t| t.name == "mcp__filesystem-kit__list_directory")
            .map(|t| t.name.clone())
            .expect("filesystem-kit exposes list_directory");
        let (out, is_err) = bridge.execute(&list_dir, "{}");
        println!("list_directory -> {} bytes", out.len());
        assert!(!is_err, "list_directory failed: {out}");
    }
}
