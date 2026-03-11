//! Comprehensive tests for JSON output format
//!
//! These tests verify that all CLI commands support --format json
//! and produce valid, parseable JSON output.

use crate::output::{
    ActionEdit, ActionResult, CheckpointResult, ContextFile, ContextResult, ContextSymbol,
    IndexResult, ModeInfo, ModesResult, TelemetryResult, TestCaseInfo, TestResult,
};

#[cfg(test)]
mod json_output_tests {
    use super::*;
    use serde_json::Value;

    fn parse_json(json: &str) -> Value {
        serde_json::from_str(json).expect("Output should be valid JSON")
    }

    #[test]
    fn test_action_result_json_structure() {
        let result = ActionResult::success(
            "extract-function",
            "src/main.rs",
            "Extract function",
            "refactor.extract.function",
            vec![ActionEdit {
                start_line: 10,
                start_column: 0,
                end_line: 20,
                end_column: 5,
                new_text: "fn new_function() {}".to_string(),
            }],
        );

        let json = serde_json::to_string_pretty(&result).unwrap();
        let parsed = parse_json(&json);

        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["action"], "extract-function");
        assert_eq!(parsed["file"], "src/main.rs");
        assert_eq!(parsed["title"], "Extract function");
        assert_eq!(parsed["kind"], "refactor.extract.function");
        assert!(parsed["edits"].is_array());
        assert_eq!(parsed["edits"][0]["start_line"], 10);
        assert_eq!(parsed["edits"][0]["new_text"], "fn new_function() {}");
    }

    #[test]
    fn test_action_result_error_json() {
        let result = ActionResult::error("rename", "src/lib.rs", "Symbol not found");
        let json = serde_json::to_string_pretty(&result).unwrap();
        let parsed = parse_json(&json);

        assert_eq!(parsed["success"], false);
        assert_eq!(parsed["error"], "Symbol not found");
    }

    #[test]
    fn test_test_result_json_structure() {
        let result = TestResult::success(
            "src/calculator.rs",
            Some("add".to_string()),
            "rust",
            vec![TestCaseInfo {
                name: "test_add_positive".to_string(),
                description: "Test adding positive numbers".to_string(),
                code: "assert_eq!(add(1, 2), 3);".to_string(),
            }],
            Some("src/calculator_test.rs".to_string()),
        );

        let json = serde_json::to_string_pretty(&result).unwrap();
        let parsed = parse_json(&json);

        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["file"], "src/calculator.rs");
        assert_eq!(parsed["function"], "add");
        assert_eq!(parsed["language"], "rust");
        assert_eq!(parsed["test_cases"].as_array().unwrap().len(), 1);
        assert_eq!(parsed["output_path"], "src/calculator_test.rs");
    }

    #[test]
    fn test_index_result_json_structure() {
        let result = IndexResult::success(
            "/workspace/myproject",
            50,
            200,
            500,
            150,
            2500,
            vec!["error1".to_string()],
        );

        let json = serde_json::to_string_pretty(&result).unwrap();
        let parsed = parse_json(&json);

        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["workspace_path"], "/workspace/myproject");
        assert_eq!(parsed["files_indexed"], 50);
        assert_eq!(parsed["symbols_found"], 200);
        assert_eq!(parsed["references_found"], 500);
        assert_eq!(parsed["embeddings_created"], 150);
        assert_eq!(parsed["duration_ms"], 2500);
    }

    #[test]
    fn test_context_result_json_structure() {
        let result = ContextResult::success(
            "find authentication logic",
            50000,
            15000,
            vec![ContextFile {
                path: "src/auth/login.rs".to_string(),
                token_count: 500,
                symbols: vec!["login".to_string()],
            }],
            vec![ContextSymbol {
                name: "authenticate".to_string(),
                kind: "function".to_string(),
                location: "src/auth/login.rs:42".to_string(),
                token_count: 100,
            }],
        );

        let json = serde_json::to_string_pretty(&result).unwrap();
        let parsed = parse_json(&json);

        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["query"], "find authentication logic");
        assert_eq!(parsed["max_tokens"], 50000);
        assert_eq!(parsed["total_tokens"], 15000);
        assert_eq!(parsed["files"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_checkpoint_result_json_structure() {
        let result = CheckpointResult::success("create")
            .with_checkpoint_id("cp-abc123")
            .with_session_id("session-xyz")
            .with_description("Before refactoring")
            .with_file_count(15);

        let json = serde_json::to_string_pretty(&result).unwrap();
        let parsed = parse_json(&json);

        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["operation"], "create");
        assert_eq!(parsed["checkpoint_id"], "cp-abc123");
        assert_eq!(parsed["session_id"], "session-xyz");
        assert_eq!(parsed["description"], "Before refactoring");
        assert_eq!(parsed["file_count"], 15);
    }

    #[test]
    fn test_modes_result_json_structure() {
        let result = ModesResult::success("list").with_modes(vec![ModeInfo {
            name: "code".to_string(),
            description: "Code writing mode".to_string(),
        }]);

        let json = serde_json::to_string_pretty(&result).unwrap();
        let parsed = parse_json(&json);

        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["operation"], "list");
        assert_eq!(parsed["modes"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_telemetry_result_json_structure() {
        let result = TelemetryResult::success(
            true,
            false,
            true,
            "/home/user/.clawdius/config.toml".to_string(),
        );

        let json = serde_json::to_string_pretty(&result).unwrap();
        let parsed = parse_json(&json);

        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["metrics_enabled"], true);
        assert_eq!(parsed["crash_reporting_enabled"], false);
        assert_eq!(parsed["config_path"], "/home/user/.clawdius/config.toml");
    }

    #[test]
    fn test_all_results_have_timestamp() {
        let action = ActionResult::success("test", "file", "title", "kind", vec![]);
        let json = serde_json::to_string(&action).unwrap();
        assert!(serde_json::from_str::<Value>(&json)
            .unwrap()
            .get("timestamp")
            .is_some());

        let test = TestResult::success("file", None, "rust", vec![], None);
        let json = serde_json::to_string(&test).unwrap();
        assert!(serde_json::from_str::<Value>(&json)
            .unwrap()
            .get("timestamp")
            .is_some());

        let index = IndexResult::success("/ws", 0, 0, 0, 0, 0, vec![]);
        let json = serde_json::to_string(&index).unwrap();
        assert!(serde_json::from_str::<Value>(&json)
            .unwrap()
            .get("timestamp")
            .is_some());

        let context = ContextResult::success("query", 0, 0, vec![], vec![]);
        let json = serde_json::to_string(&context).unwrap();
        assert!(serde_json::from_str::<Value>(&json)
            .unwrap()
            .get("timestamp")
            .is_some());

        let checkpoint = CheckpointResult::success("create");
        let json = serde_json::to_string(&checkpoint).unwrap();
        assert!(serde_json::from_str::<Value>(&json)
            .unwrap()
            .get("timestamp")
            .is_some());

        let modes = ModesResult::success("list");
        let json = serde_json::to_string(&modes).unwrap();
        assert!(serde_json::from_str::<Value>(&json)
            .unwrap()
            .get("timestamp")
            .is_some());

        let telemetry = TelemetryResult::success(true, true, true, "/config".to_string());
        let json = serde_json::to_string(&telemetry).unwrap();
        assert!(serde_json::from_str::<Value>(&json)
            .unwrap()
            .get("timestamp")
            .is_some());
    }

    #[test]
    fn test_all_errors_have_consistent_structure() {
        let action = ActionResult::error("action", "file", "error");
        let json = serde_json::to_string(&action).unwrap();
        let parsed = serde_json::from_str::<Value>(&json).unwrap();
        assert_eq!(parsed["success"], false);
        assert!(parsed.get("error").is_some());

        let test = TestResult::error("file", "error");
        let json = serde_json::to_string(&test).unwrap();
        let parsed = serde_json::from_str::<Value>(&json).unwrap();
        assert_eq!(parsed["success"], false);
        assert!(parsed.get("error").is_some());

        let index = IndexResult::error("/ws", "error");
        let json = serde_json::to_string(&index).unwrap();
        let parsed = serde_json::from_str::<Value>(&json).unwrap();
        assert_eq!(parsed["success"], false);
        assert!(parsed.get("error").is_some());

        let context = ContextResult::error("query", "error");
        let json = serde_json::to_string(&context).unwrap();
        let parsed = serde_json::from_str::<Value>(&json).unwrap();
        assert_eq!(parsed["success"], false);
        assert!(parsed.get("error").is_some());

        let checkpoint = CheckpointResult::error("op", "error");
        let json = serde_json::to_string(&checkpoint).unwrap();
        let parsed = serde_json::from_str::<Value>(&json).unwrap();
        assert_eq!(parsed["success"], false);
        assert!(parsed.get("error").is_some());

        let modes = ModesResult::error("op", "error");
        let json = serde_json::to_string(&modes).unwrap();
        let parsed = serde_json::from_str::<Value>(&json).unwrap();
        assert_eq!(parsed["success"], false);
        assert!(parsed.get("error").is_some());

        let telemetry = TelemetryResult::error("error");
        let json = serde_json::to_string(&telemetry).unwrap();
        let parsed = serde_json::from_str::<Value>(&json).unwrap();
        assert_eq!(parsed["success"], false);
        assert!(parsed.get("error").is_some());
    }
}
