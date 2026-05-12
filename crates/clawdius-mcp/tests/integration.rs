//! Integration tests for clawdius-mcp binary.
//!
//! Spawns the MCP stdio server with pre-loaded stdin, captures stdout,
//! validates responses.

#![allow(
    dead_code,
    missing_docs,
    unused_variables,
    clippy::cast_precision_loss,
    clippy::expect_used,
    clippy::items_after_statements,
    clippy::manual_is_multiple_of,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,
    clippy::uninlined_format_args,
    clippy::unwrap_used
)]
use std::io::Write;
use std::process::{Command, Stdio};

/// Spawn the clawdius-mcp binary, write all requests to stdin, close stdin,
/// collect stdout. Returns the raw stdout lines.
fn run_mcp(requests: &[&str]) -> Vec<String> {
    let bin = env!("CARGO_BIN_EXE_clawdius-mcp");
    let mut child = Command::new(bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn clawdius-mcp");

    // Write all requests then close stdin (EOF triggers server exit)
    {
        let stdin = child.stdin.as_mut().expect("stdin available");
        for req in requests {
            writeln!(stdin, "{req}").expect("write to stdin");
        }
    }
    // stdin is dropped here, sending EOF

    let output = child.wait_with_output().expect("child process exited");
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .collect()
}

#[test]
fn test_initialize_request_returns_valid_json() {
    let lines = run_mcp(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1.0"}}}"#,
    ]);
    assert!(!lines.is_empty(), "should get at least one response");
    let parsed: serde_json::Value =
        serde_json::from_str(&lines[0]).expect("response is valid JSON");
    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 1);
    assert!(
        parsed["error"].is_null(),
        "initialize should succeed, got: {parsed}"
    );
}

#[test]
fn test_tools_list_returns_array() {
    let lines = run_mcp(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1.0"}}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
    ]);
    // Should get 2 responses
    assert!(
        lines.len() >= 2,
        "expected >= 2 responses, got {}: {:?}",
        lines.len(),
        lines
    );
    let tools_resp: serde_json::Value =
        serde_json::from_str(&lines[1]).expect("response is valid JSON");
    assert_eq!(tools_resp["id"], 2);
    let tools = &tools_resp["result"]["tools"];
    assert!(tools.is_array(), "tools should be an array, got: {tools}");
    assert!(
        tools.as_array().is_some_and(|arr| !arr.is_empty()),
        "tools list should not be empty"
    );
}

#[test]
fn test_malformed_json_does_not_crash() {
    // Server should not crash on garbage input
    let lines = run_mcp(&[
        "not json at all",
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1.0"}}}"#,
    ]);
    // Should still get the initialize response
    assert!(!lines.is_empty(), "server survived malformed input");
    let parsed: serde_json::Value =
        serde_json::from_str(&lines[0]).expect("response is valid JSON");
    assert_eq!(parsed["jsonrpc"], "2.0");
}

#[test]
fn test_empty_lines_are_ignored() {
    let lines = run_mcp(&[
        "",
        "   ",
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1.0"}}}"#,
    ]);
    assert!(!lines.is_empty(), "server survived empty lines");
    let parsed: serde_json::Value =
        serde_json::from_str(&lines[0]).expect("response is valid JSON");
    assert_eq!(parsed["id"], 1);
}

#[test]
fn test_binary_exits_cleanly_on_eof() {
    let bin = env!("CARGO_BIN_EXE_clawdius-mcp");
    let mut child = Command::new(bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn clawdius-mcp");

    // Close stdin immediately -> EOF
    child.stdin.take();

    let output = child.wait_with_output().expect("child exited");
    assert!(
        output.status.success(),
        "server should exit cleanly on EOF, got: {:?}",
        output.status.code()
    );
}

#[test]
fn test_notification_produces_no_output() {
    let lines = run_mcp(&[
        r#"{"jsonrpc":"2.0","id":0,"method":"notifications/initialized","params":{}}"#,
    ]);
    // Notifications should not produce a response line on stdout
    assert!(
        lines.is_empty(),
        "notification should produce no output, got: {:?}",
        lines
    );
}

#[test]
fn test_ping_returns_empty_object() {
    let lines = run_mcp(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1.0"}}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"ping","params":{}}"#,
    ]);
    assert!(lines.len() >= 2, "expected >= 2 responses, got {}", lines.len());
    let ping_resp: serde_json::Value =
        serde_json::from_str(&lines[1]).expect("ping response is valid JSON");
    assert_eq!(ping_resp["id"], 2);
    assert!(
        !ping_resp["result"].is_null(),
        "ping should return a result, got: {ping_resp}"
    );
}

#[test]
fn test_resources_list_returns_array() {
    let lines = run_mcp(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1.0"}}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"resources/list","params":{}}"#,
    ]);
    assert!(lines.len() >= 2, "expected >= 2 responses, got {}", lines.len());
    let resources_resp: serde_json::Value =
        serde_json::from_str(&lines[1]).expect("resources response is valid JSON");
    assert_eq!(resources_resp["id"], 2);
    let resources = &resources_resp["result"]["resources"];
    assert!(
        resources.is_array(),
        "resources should be an array, got: {resources}"
    );
}

#[test]
fn test_prompts_list_returns_array() {
    let lines = run_mcp(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1.0"}}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"prompts/list","params":{}}"#,
    ]);
    assert!(lines.len() >= 2, "expected >= 2 responses, got {}", lines.len());
    let prompts_resp: serde_json::Value =
        serde_json::from_str(&lines[1]).expect("prompts response is valid JSON");
    assert_eq!(prompts_resp["id"], 2);
    let prompts = &prompts_resp["result"]["prompts"];
    assert!(
        prompts.is_array(),
        "prompts should be an array, got: {prompts}"
    );
}

#[test]
fn test_unknown_method_error_code() {
    let lines = run_mcp(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1.0"}}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"fake/nonexistent","params":{}}"#,
    ]);
    assert!(lines.len() >= 2, "expected >= 2 responses, got {}", lines.len());
    let err_resp: serde_json::Value =
        serde_json::from_str(&lines[1]).expect("error response is valid JSON");
    assert_eq!(err_resp["id"], 2);
    assert_eq!(err_resp["error"]["code"], -32601, "expected Method not found");
}

#[test]
fn test_three_sequential_requests() {
    let lines = run_mcp(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1.0"}}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"ping","params":{}}"#,
    ]);
    assert!(
        lines.len() >= 3,
        "expected >= 3 responses for 3 requests, got {}: {:?}",
        lines.len(),
        lines
    );
    // Verify response ordering by id
    let r1: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    let r2: serde_json::Value = serde_json::from_str(&lines[1]).unwrap();
    let r3: serde_json::Value = serde_json::from_str(&lines[2]).unwrap();
    assert_eq!(r1["id"], 1);
    assert_eq!(r2["id"], 2);
    assert_eq!(r3["id"], 3);
}

#[test]
fn test_tools_call_with_invalid_tool() {
    let lines = run_mcp(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1.0"}}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"nonexistent_tool","arguments":{}}}"#,
    ]);
    assert!(lines.len() >= 2, "expected >= 2 responses, got {}", lines.len());
    let call_resp: serde_json::Value =
        serde_json::from_str(&lines[1]).expect("tools/call response is valid JSON");
    assert_eq!(call_resp["id"], 2);
    // Should be an error since the tool doesn't exist
    assert!(
        !call_resp["error"].is_null(),
        "expected error for nonexistent tool, got: {call_resp}"
    );
}
