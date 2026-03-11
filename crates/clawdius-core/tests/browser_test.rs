//! Tests for browser automation tool

use clawdius_core::tools::browser::{BrowserTool, BrowserToolConfig};

#[tokio::test]
async fn test_browser_tool_creation() {
    let _browser = BrowserTool::new();
    // Browser starts uninitialized
}

#[tokio::test]
async fn test_browser_config_default() {
    let config = BrowserToolConfig::default();
    assert!(config.headless);
    assert_eq!(config.width, 1920);
    assert_eq!(config.height, 1080);
    assert!(config.enable_console_logs);
    assert_eq!(config.max_console_logs, 100);
}

#[tokio::test]
async fn test_browser_config_custom() {
    let config = BrowserToolConfig {
        headless: false,
        width: 1280,
        height: 720,
        enable_console_logs: false,
        max_console_logs: 50,
        default_timeout_ms: 5000,
        user_agent: Some("CustomAgent/1.0".to_string()),
        accept_insecure_certs: true,
    };

    let _browser = BrowserTool::with_config(config);
}

#[tokio::test]
async fn test_browser_not_initialized_error() {
    let mut browser = BrowserTool::new();

    let result = browser.get_url().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_console_log_structure() {
    use clawdius_core::tools::browser::ConsoleLog;

    let log = ConsoleLog {
        level: "error".to_string(),
        message: "Test error".to_string(),
        url: Some("https://example.com".to_string()),
        line: Some(42),
    };

    assert_eq!(log.level, "error");
    assert_eq!(log.message, "Test error");
}

#[tokio::test]
async fn test_dialog_info_structure() {
    use clawdius_core::tools::browser::DialogInfo;

    let dialog = DialogInfo {
        dialog_type: "alert".to_string(),
        message: "Hello World".to_string(),
        default_value: None,
    };

    assert_eq!(dialog.dialog_type, "alert");
    assert_eq!(dialog.message, "Hello World");
}

#[tokio::test]
async fn test_browser_action_result() {
    use clawdius_core::tools::browser::BrowserActionResult;
    use serde_json::json;

    let success = BrowserActionResult::success(Some(json!({"key": "value"})));
    assert!(success.success);
    assert!(success.data.is_some());
    assert!(success.error.is_none());

    let error = BrowserActionResult::error("Something went wrong");
    assert!(!error.success);
    assert!(error.data.is_none());
    assert!(error.error.is_some());
}
