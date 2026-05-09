//! Integration tests for clawdius-mcp binary.
//!
//! Spawns the MCP stdio server with pre-loaded stdin, captures stdout,
//! validates responses.

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
        tools.as_array().map_or(false, |arr| !arr.is_empty()),
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

    // Close stdin immediately → EOF
    child.stdin.take();

    let output = child.wait_with_output().expect("child exited");
    assert!(
        output.status.success(),
        "server should exit cleanly on EOF, got: {:?}",
        output.status.code()
    );
}
