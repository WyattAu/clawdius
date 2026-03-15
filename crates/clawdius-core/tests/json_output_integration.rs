//! Integration tests for JSON Output system
//!
//! Tests all command output formats, JSON serialization,
//! and stream JSON output.

use chrono::Utc;
use clawdius_core::output::{
    BrokerResult, ChangeType, ComplianceResult, ConfigResult, FileChange, FileVersionInfo,
    InitResult, JsonOutput, MetricsResult, OutputFormat, OutputOptions, ProofError,
    RefactorFileChange, RefactorResult, ResearchConcept, ResearchRelationship, ResearchResult,
    TimelineResult, TokenUsageInfo, ToolCallInfo, VerifyResult,
};

#[test]
fn test_json_output_success() {
    let output = JsonOutput::success("Hello, world!", "session-123");

    assert!(output.success);
    assert!(output.error.is_none());
    assert_eq!(output.content, "Hello, world!");
    assert_eq!(output.session_id, "session-123");
}

#[test]
fn test_json_output_error() {
    let output = JsonOutput::error("Something went wrong", "session-456");

    assert!(!output.success);
    assert_eq!(output.error, Some("Something went wrong".to_string()));
    assert_eq!(output.session_id, "session-456");
    assert!(output.content.is_empty());
}

#[test]
fn test_json_output_with_tool_call() {
    let tool_call = ToolCallInfo {
        name: "file_read".to_string(),
        arguments: serde_json::json!({"path": "/test/file.rs"}),
        result: Some(serde_json::json!("file contents")),
        success: true,
        duration_ms: 50,
    };

    let output = JsonOutput::success("Response", "session-1").with_tool_call(tool_call);

    assert_eq!(output.tool_calls.len(), 1);
    assert_eq!(output.tool_calls[0].name, "file_read");
}

#[test]
fn test_json_output_with_file_change() {
    let change = FileChange {
        path: "/src/main.rs".to_string(),
        change_type: ChangeType::Modified,
        lines_added: 10,
        lines_removed: 5,
    };

    let output = JsonOutput::success("Response", "session-1").with_file_change(change);

    assert_eq!(output.files_changed.len(), 1);
    assert_eq!(output.files_changed[0].path, "/src/main.rs");
}

#[test]
fn test_json_output_with_usage() {
    let usage = TokenUsageInfo::new(100, 50);

    let output = JsonOutput::success("Response", "session-1").with_usage(usage);

    assert_eq!(output.usage.input, 100);
    assert_eq!(output.usage.output, 50);
    assert_eq!(output.usage.total, 150);
}

#[test]
fn test_json_output_with_duration() {
    let output = JsonOutput::success("Response", "session-1").with_duration(1500);

    assert_eq!(output.duration_ms, 1500);
}

#[test]
fn test_json_output_serialization() {
    let output = JsonOutput::success("Test", "session-123")
        .with_usage(TokenUsageInfo::new(100, 50))
        .with_duration(1500);

    let json = output.to_json().unwrap();
    assert!(json.contains("\"content\": \"Test\""));
    assert!(json.contains("\"duration_ms\": 1500"));
    assert!(json.contains("\"input\": 100"));
}

#[test]
fn test_json_output_compact_serialization() {
    let output = JsonOutput::success("Test", "session-123");

    let json = output.to_json_compact().unwrap();
    assert!(json.contains("\"content\":\"Test\""));
    assert!(!json.contains('\n'));
}

#[test]
fn test_init_result_success() {
    let result = InitResult::success("/path/to/project", "/path/to/config.toml", true);

    assert!(result.success);
    assert!(result.onboarding_complete);
    assert!(result.error.is_none());
}

#[test]
fn test_init_result_error() {
    let result = InitResult::error("Failed to initialize");

    assert!(!result.success);
    assert!(result.error.is_some());
}

#[test]
fn test_config_result_success() {
    let config = serde_json::json!({"key": "value"});
    let result = ConfigResult::success(config.clone(), "/path/to/config.toml");

    assert!(result.success);
    assert_eq!(result.config, config);
    assert!(result.error.is_none());
}

#[test]
fn test_config_result_error() {
    let result = ConfigResult::error("Config not found");

    assert!(!result.success);
    assert!(result.error.is_some());
}

#[test]
fn test_metrics_result() {
    let result = MetricsResult::new(1000, 10, 150.5, 50000, 0.01);

    assert_eq!(result.requests_total, 1000);
    assert_eq!(result.requests_errors, 10);
    assert_eq!(result.avg_latency_ms, 150.5);
    assert_eq!(result.tokens_used, 50000);
    assert_eq!(result.error_rate, 0.01);
}

#[test]
fn test_verify_result_success() {
    let result = VerifyResult::success("/path/to/proof.rs", 500);

    assert!(result.success);
    assert!(result.errors.is_empty());
    assert!(result.warnings.is_empty());
}

#[test]
fn test_verify_result_failure() {
    let errors = vec![ProofError {
        line: 10,
        column: 5,
        message: "Type mismatch".to_string(),
    }];
    let warnings = vec!["Unused variable".to_string()];

    let result = VerifyResult::failure("/path/to/proof.rs", 500, errors, warnings);

    assert!(!result.success);
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.warnings.len(), 1);
}

#[test]
fn test_refactor_result_success() {
    let files_changed = vec![RefactorFileChange {
        original_path: "/src/main.py".to_string(),
        new_path: Some("/src/main.rs".to_string()),
        change_type: "converted".to_string(),
        lines_added: 100,
        lines_removed: 80,
    }];

    let result = RefactorResult::success(
        "python",
        "rust",
        "/src",
        false,
        files_changed,
        "Conversion complete",
    );

    assert!(result.success);
    assert!(!result.dry_run);
    assert_eq!(result.files_changed.len(), 1);
}

#[test]
fn test_refactor_result_dry_run() {
    let result = RefactorResult::success(
        "javascript",
        "typescript",
        "/src",
        true,
        vec![],
        "Dry run complete",
    );

    assert!(result.success);
    assert!(result.dry_run);
}

#[test]
fn test_refactor_result_error() {
    let result = RefactorResult::error("Unsupported language");

    assert!(!result.success);
    assert!(result.error.is_some());
}

#[test]
fn test_broker_result_success() {
    let result = BrokerResult::success(
        true,
        "connected",
        Some("/path/to/broker.toml".to_string()),
        "Broker initialized",
    );

    assert!(result.success);
    assert!(result.paper_trade);
    assert!(result.error.is_none());
}

#[test]
fn test_broker_result_error() {
    let result = BrokerResult::error("Connection failed");

    assert!(!result.success);
    assert!(result.error.is_some());
}

#[test]
fn test_compliance_result_success() {
    let standards = vec!["SOC2".to_string(), "ISO27001".to_string()];
    let matrix = serde_json::json!({"compliant": true});

    let result = ComplianceResult::success(
        standards.clone(),
        "/project",
        "json",
        Some("/output/matrix.json".to_string()),
        matrix.clone(),
    );

    assert!(result.success);
    assert_eq!(result.standards, standards);
    assert!(result.error.is_none());
}

#[test]
fn test_compliance_result_error() {
    let result = ComplianceResult::error("Standard not found");

    assert!(!result.success);
    assert!(result.error.is_some());
}

#[test]
fn test_research_result_success() {
    let concepts = vec![ResearchConcept {
        language: "rust".to_string(),
        name: "Ownership".to_string(),
        definition: "Memory management pattern".to_string(),
    }];

    let relationships = vec![ResearchRelationship {
        from: "Ownership".to_string(),
        relationship: "enables".to_string(),
        to: "Borrowing".to_string(),
    }];

    let result = ResearchResult::success(
        "What is ownership in Rust?",
        vec!["rust".to_string()],
        0.95,
        concepts,
        relationships,
    );

    assert!(result.success);
    assert_eq!(result.confidence, 0.95);
    assert!(result.error.is_none());
}

#[test]
fn test_research_result_error() {
    let result = ResearchResult::error("Query failed");

    assert!(!result.success);
    assert!(result.error.is_some());
}

#[test]
fn test_timeline_result() {
    let timestamp = Utc::now();
    let result = TimelineResult::new(
        "checkpoint-123".to_string(),
        timestamp,
        5,
        "Before refactor".to_string(),
    );

    assert_eq!(result.checkpoint_id, "checkpoint-123");
    assert_eq!(result.files_changed, 5);
    assert_eq!(result.description, "Before refactor");
}

#[test]
fn test_file_version_info() {
    let timestamp = Utc::now();
    let result = FileVersionInfo::new(
        "/src/main.rs".to_string(),
        3,
        timestamp,
        "abc123def456".to_string(),
        1024,
    );

    assert_eq!(result.path, "/src/main.rs");
    assert_eq!(result.version, 3);
    assert_eq!(result.checksum, "abc123def456");
    assert_eq!(result.size, 1024);
}

#[test]
fn test_output_format_from_str() {
    assert_eq!(OutputFormat::parse_format("text"), Some(OutputFormat::Text));
    assert_eq!(OutputFormat::parse_format("json"), Some(OutputFormat::Json));
    assert_eq!(
        OutputFormat::parse_format("stream-json"),
        Some(OutputFormat::StreamJson)
    );
    assert_eq!(
        OutputFormat::parse_format("stream_json"),
        Some(OutputFormat::StreamJson)
    );
    assert_eq!(OutputFormat::parse_format("invalid"), None);
}

#[test]
fn test_output_format_display() {
    assert_eq!(format!("{}", OutputFormat::Text), "text");
    assert_eq!(format!("{}", OutputFormat::Json), "json");
    assert_eq!(format!("{}", OutputFormat::StreamJson), "stream-json");
}

#[test]
fn test_output_options_default() {
    let options = OutputOptions::default();

    assert_eq!(options.format, OutputFormat::Text);
    assert!(options.show_progress);
    assert!(!options.quiet);
    assert!(options.include_metadata);
}

#[test]
fn test_token_usage_info() {
    let usage = TokenUsageInfo::new(100, 50);

    assert_eq!(usage.input, 100);
    assert_eq!(usage.output, 50);
    assert_eq!(usage.total, 150);
    assert_eq!(usage.cached, 0);
}

#[test]
fn test_change_type_serialization() {
    let created = ChangeType::Created;
    let modified = ChangeType::Modified;
    let deleted = ChangeType::Deleted;
    let renamed = ChangeType::Renamed;

    assert_eq!(serde_json::to_string(&created).unwrap(), "\"created\"");
    assert_eq!(serde_json::to_string(&modified).unwrap(), "\"modified\"");
    assert_eq!(serde_json::to_string(&deleted).unwrap(), "\"deleted\"");
    assert_eq!(serde_json::to_string(&renamed).unwrap(), "\"renamed\"");
}

#[test]
fn test_multiple_tool_calls() {
    let output = JsonOutput::success("Response", "session-1")
        .with_tool_call(ToolCallInfo {
            name: "file_read".to_string(),
            arguments: serde_json::json!({}),
            result: None,
            success: true,
            duration_ms: 10,
        })
        .with_tool_call(ToolCallInfo {
            name: "shell_exec".to_string(),
            arguments: serde_json::json!({}),
            result: None,
            success: true,
            duration_ms: 20,
        });

    assert_eq!(output.tool_calls.len(), 2);
}

#[test]
fn test_multiple_file_changes() {
    let output = JsonOutput::success("Response", "session-1")
        .with_file_change(FileChange {
            path: "/src/main.rs".to_string(),
            change_type: ChangeType::Modified,
            lines_added: 10,
            lines_removed: 5,
        })
        .with_file_change(FileChange {
            path: "/src/lib.rs".to_string(),
            change_type: ChangeType::Created,
            lines_added: 50,
            lines_removed: 0,
        });

    assert_eq!(output.files_changed.len(), 2);
}

#[test]
fn test_combined_output_builders() {
    let output = JsonOutput::success("Complete response", "session-combined")
        .with_tool_call(ToolCallInfo {
            name: "test".to_string(),
            arguments: serde_json::json!({}),
            result: None,
            success: true,
            duration_ms: 10,
        })
        .with_file_change(FileChange {
            path: "/test.rs".to_string(),
            change_type: ChangeType::Modified,
            lines_added: 1,
            lines_removed: 0,
        })
        .with_usage(TokenUsageInfo::new(200, 100))
        .with_duration(2500);

    assert!(output.success);
    assert_eq!(output.tool_calls.len(), 1);
    assert_eq!(output.files_changed.len(), 1);
    assert_eq!(output.usage.total, 300);
    assert_eq!(output.duration_ms, 2500);
}
