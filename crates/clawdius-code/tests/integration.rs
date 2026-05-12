//! Smoke tests for clawdius-code binary.
//!
//! Verifies the JSON-RPC server starts, handles requests, and exits cleanly.

#![allow(
    dead_code,
    unused_variables,
    clippy::cast_precision_loss,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::items_after_statements,
    clippy::manual_is_multiple_of,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use
)]

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
    let lines = run_code(&[r#"{"jsonrpc":"2.0","id":1,"method":"state/get","params":{}}"#]);
    assert!(!lines.is_empty(), "server should respond");
    let parsed: serde_json::Value =
        serde_json::from_str(&lines[0]).expect("response is valid JSON");
    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 1);
}

#[test]
fn test_unknown_method_returns_error() {
    let lines =
        run_code(&[r#"{"jsonrpc":"2.0","id":42,"method":"nonexistent/method","params":{}}"#]);
    assert!(!lines.is_empty(), "server should respond to unknown method");
    let parsed: serde_json::Value =
        serde_json::from_str(&lines[0]).expect("response is valid JSON");
    assert_eq!(parsed["id"], 42);
    assert!(
        !parsed["error"].is_null(),
        "expected error for unknown method, got: {parsed}"
    );
}

#[test]
fn test_malformed_json_returns_parse_error() {
    let lines = run_code(&["{broken json"]);
    assert!(
        !lines.is_empty(),
        "server should respond to malformed input"
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&lines[0]).expect("response is valid JSON");
    assert!(
        !parsed["error"].is_null(),
        "expected parse error for malformed JSON, got: {parsed}"
    );
}

#[test]
fn test_session_list_returns_result() {
    let lines = run_code(&[r#"{"jsonrpc":"2.0","id":3,"method":"session/list","params":{}}"#]);
    assert!(!lines.is_empty(), "server should respond to session/list");
    let parsed: serde_json::Value =
        serde_json::from_str(&lines[0]).expect("response is valid JSON");
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

#[test]
fn test_session_create_returns_id() {
    let lines = run_code(&[r#"{"jsonrpc":"2.0","id":1,"method":"session/create","params":{}}"#]);
    assert!(!lines.is_empty(), "server should respond to session/create");
    let parsed: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert_eq!(parsed["id"], 1);
    assert!(
        !parsed["result"].is_null(),
        "session/create should return a result, got: {parsed}"
    );
}

#[test]
fn test_session_create_then_list() {
    let lines = run_code(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"session/create","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"session/list","params":{}}"#,
    ]);
    assert!(
        lines.len() >= 2,
        "expected >= 2 responses, got {}",
        lines.len()
    );
    let create_resp: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert_eq!(create_resp["id"], 1);
    let list_resp: serde_json::Value = serde_json::from_str(&lines[1]).unwrap();
    assert_eq!(list_resp["id"], 2);
    let sessions = &list_resp["result"];
    assert!(
        !sessions.is_null(),
        "session/list should return a result, got: {list_resp}"
    );
}

#[test]
fn test_session_load_missing_id_returns_error() {
    let lines = run_code(&[r#"{"jsonrpc":"2.0","id":1,"method":"session/load","params":{}}"#]);
    assert!(!lines.is_empty(), "server should respond");
    let parsed: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert!(
        !parsed["error"].is_null(),
        "session/load without id should return error, got: {parsed}"
    );
}

#[test]
fn test_session_delete_nonexistent_returns_error() {
    let lines = run_code(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"session/delete","params":{"id":"nonexistent-uuid"}}"#,
    ]);
    assert!(!lines.is_empty(), "server should respond");
    let parsed: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert!(
        !parsed["error"].is_null(),
        "session/delete with nonexistent id should return error, got: {parsed}"
    );
}

#[test]
fn test_file_read_nonexistent_returns_error() {
    let lines = run_code(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"file/read","params":{"path":"/nonexistent/file.txt"}}"#,
    ]);
    assert!(!lines.is_empty(), "server should respond");
    let parsed: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert!(
        !parsed["error"].is_null(),
        "file/read for nonexistent path should return error, got: {parsed}"
    );
}

#[test]
fn test_context_list_empty() {
    let lines = run_code(&[r#"{"jsonrpc":"2.0","id":1,"method":"context/list","params":{}}"#]);
    assert!(!lines.is_empty(), "server should respond to context/list");
    let parsed: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert_eq!(parsed["id"], 1);
    assert!(
        !parsed["result"].is_null(),
        "context/list should return a result, got: {parsed}"
    );
}

#[test]
fn test_context_add_unknown_type_returns_error() {
    let lines = run_code(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"context/add","params":{"type":"invalid_type","path":"/tmp/test"}}"#,
    ]);
    assert!(!lines.is_empty(), "server should respond");
    let parsed: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert!(
        !parsed["error"].is_null(),
        "context/add with unknown type should return error, got: {parsed}"
    );
}

#[test]
fn test_state_checkpoint_returns_id() {
    let lines = run_code(&[r#"{"jsonrpc":"2.0","id":1,"method":"state/checkpoint","params":{}}"#]);
    assert!(
        !lines.is_empty(),
        "server should respond to state/checkpoint"
    );
    let parsed: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert_eq!(parsed["id"], 1);
    assert!(
        !parsed["result"].is_null(),
        "state/checkpoint should return a result, got: {parsed}"
    );
}

#[test]
fn test_state_list_returns_result() {
    let lines = run_code(&[r#"{"jsonrpc":"2.0","id":1,"method":"state/list","params":{}}"#]);
    assert!(!lines.is_empty(), "server should respond to state/list");
    let parsed: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert_eq!(parsed["id"], 1);
    assert!(
        !parsed["result"].is_null(),
        "state/list should return a result, got: {parsed}"
    );
}

#[test]
fn test_chat_send_no_llm_returns_error() {
    let lines = run_code(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"chat/send","params":{"message":"hello"}}"#,
    ]);
    assert!(!lines.is_empty(), "server should respond to chat/send");
    let parsed: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    // Without LLM configured, should return an error
    assert!(
        !parsed["error"].is_null(),
        "chat/send without LLM should return error, got: {parsed}"
    );
}

#[test]
fn test_empty_lines_are_ignored() {
    let lines = run_code(&[
        "",
        "   ",
        r#"{"jsonrpc":"2.0","id":1,"method":"session/list","params":{}}"#,
    ]);
    assert!(!lines.is_empty(), "server should survive empty lines");
    let parsed: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert_eq!(parsed["id"], 1);
}

#[test]
fn test_notification_style_request() {
    // JSON-RPC 2.0 says servers MUST NOT reply to notifications (id: null),
    // but many implementations return an error for unknown methods.
    // Verify the server handles null-id requests without crashing.
    let lines = run_code(&[r#"{"jsonrpc":"2.0","id":null,"method":"initialized","params":{}}"#]);
    // Server should not crash; it may return an error or silence
    if !lines.is_empty() {
        let parsed: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        // If response given, id should be null
        assert!(
            parsed["id"].is_null(),
            "notification response id should be null"
        );
    }
}

#[test]
fn test_multiple_sequential_requests() {
    let lines = run_code(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"session/create","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"session/list","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"state/get","params":{}}"#,
    ]);
    assert!(
        lines.len() >= 3,
        "expected >= 3 responses, got {}: {:?}",
        lines.len(),
        lines
    );
    // Verify ordering
    let r1: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    let r2: serde_json::Value = serde_json::from_str(&lines[1]).unwrap();
    let r3: serde_json::Value = serde_json::from_str(&lines[2]).unwrap();
    assert_eq!(r1["id"], 1);
    assert_eq!(r2["id"], 2);
    assert_eq!(r3["id"], 3);
}

#[test]
fn test_missing_required_params_returns_error() {
    let lines = run_code(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"file/write","params":{"path":"/tmp/test.txt"}}"#,
    ]);
    assert!(!lines.is_empty(), "server should respond");
    let parsed: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert!(
        !parsed["error"].is_null(),
        "file/write without content should return error, got: {parsed}"
    );
}
