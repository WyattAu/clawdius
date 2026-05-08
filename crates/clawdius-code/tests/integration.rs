//! Smoke tests for clawdius-code binary.
//!
//! Verifies the JSON-RPC server starts, handles requests, and exits cleanly.

use std::io::Write;
use std::process::{Command, Stdio};

/// Spawn clawdius-code, write requests to stdin, close stdin, collect stdout.
fn run_code(requests: &[&str]) -> Vec<String> {
    let bin = env!("CARGO_BIN_EXE_clawdius-code");
    let mut child = Command::new(bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn clawdius-code");

    {
        let stdin = child.stdin.as_mut().expect("stdin available");
        for req in requests {
            writeln!(stdin, "{req}").expect("write to stdin");
        }
    }

    let output = child.wait_with_output().expect("child process exited");
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .collect()
}

#[test]
fn test_server_responds_to_state_get() {
    let lines = run_code(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"state/get","params":{}}"#,
    ]);
    assert!(!lines.is_empty(), "server should respond");
    let parsed: serde_json::Value = serde_json::from_str(&lines[0]).expect("response is valid JSON");
    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 1);
}

#[test]
fn test_unknown_method_returns_error() {
    let lines = run_code(&[
        r#"{"jsonrpc":"2.0","id":42,"method":"nonexistent/method","params":{}}"#,
    ]);
    assert!(!lines.is_empty(), "server should respond to unknown method");
    let parsed: serde_json::Value = serde_json::from_str(&lines[0]).expect("response is valid JSON");
    assert_eq!(parsed["id"], 42);
    assert!(
        !parsed["error"].is_null(),
        "expected error for unknown method, got: {parsed}"
    );
}

#[test]
fn test_malformed_json_returns_parse_error() {
    let lines = run_code(&["{broken json"]);
    assert!(!lines.is_empty(), "server should respond to malformed input");
    let parsed: serde_json::Value = serde_json::from_str(&lines[0]).expect("response is valid JSON");
    assert!(
        !parsed["error"].is_null(),
        "expected parse error for malformed JSON, got: {parsed}"
    );
}

#[test]
fn test_session_list_returns_result() {
    let lines = run_code(&[
        r#"{"jsonrpc":"2.0","id":3,"method":"session/list","params":{}}"#,
    ]);
    assert!(!lines.is_empty(), "server should respond to session/list");
    let parsed: serde_json::Value = serde_json::from_str(&lines[0]).expect("response is valid JSON");
    assert_eq!(parsed["id"], 3);
    assert!(
        !parsed["result"].is_null(),
        "session/list should return a result, got: {parsed}"
    );
}

#[test]
fn test_binary_exits_cleanly_on_eof() {
    let bin = env!("CARGO_BIN_EXE_clawdius-code");
    let mut child = Command::new(bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn clawdius-code");

    child.stdin.take();

    let output = child.wait_with_output().expect("child exited");
    assert!(
        output.status.success(),
        "server should exit cleanly on EOF, got: {:?}",
        output.status.code()
    );
}
