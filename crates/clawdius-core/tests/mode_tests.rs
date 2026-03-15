//! Tests for agent modes

use clawdius_core::modes::{AgentMode, CustomMode};
use tempfile::TempDir;

#[test]
fn test_builtin_modes() {
    let modes = vec![
        ("code", "Code generation and editing"),
        ("architect", "Design and structure planning"),
        ("ask", "Quick answers and explanations"),
        ("debug", "Troubleshooting and diagnostics"),
        ("review", "Code review and analysis"),
        ("refactor", "Code improvement and refactoring"),
        ("test", "Test generation"),
    ];

    for (name, _description) in modes {
        let mode =
            AgentMode::parse_mode(name).unwrap_or_else(|| panic!("Failed to parse mode: {name}"));
        assert_eq!(mode.name(), name);
        assert!(!mode.system_prompt().is_empty());
        assert!(mode.temperature() > 0.0);
        assert!(mode.temperature() <= 1.0);
    }
}

#[test]
fn test_mode_temperature() {
    assert_eq!(AgentMode::Code.temperature(), 0.7);
    assert_eq!(AgentMode::Architect.temperature(), 0.5);
    assert_eq!(AgentMode::Ask.temperature(), 0.8);
    assert_eq!(AgentMode::Debug.temperature(), 0.6);
    assert_eq!(AgentMode::Review.temperature(), 0.5);
    assert_eq!(AgentMode::Refactor.temperature(), 0.6);
    assert_eq!(AgentMode::Test.temperature(), 0.7);
}

#[test]
fn test_mode_tools() {
    let code_tools = AgentMode::Code.tools();
    assert!(code_tools.contains(&"file".to_string()));
    assert!(code_tools.contains(&"shell".to_string()));
    assert!(code_tools.contains(&"git".to_string()));

    let architect_tools = AgentMode::Architect.tools();
    assert!(architect_tools.contains(&"file".to_string()));
    assert!(!architect_tools.contains(&"shell".to_string()));
}

#[test]
fn test_mode_descriptions() {
    assert_eq!(AgentMode::Code.description(), "Code generation and editing");
    assert_eq!(
        AgentMode::Architect.description(),
        "Design and structure planning"
    );
    assert_eq!(
        AgentMode::Debug.description(),
        "Troubleshooting and diagnostics"
    );
}

#[test]
fn test_custom_mode() {
    let custom = CustomMode {
        name: "custom-test".to_string(),
        system_prompt: "You are a test assistant.".to_string(),
        description: Some("Test custom mode".to_string()),
        temperature: Some(0.8),
        tools: vec!["file".to_string()],
        max_tokens: None,
        requires_approval: None,
        enable_streaming: None,
        model: None,
    };

    let mode = AgentMode::Custom(custom);
    assert_eq!(mode.name(), "custom-test");
    assert_eq!(mode.description(), "Test custom mode");
    assert_eq!(mode.system_prompt(), "You are a test assistant.");
    assert_eq!(mode.temperature(), 0.8);
    assert_eq!(mode.tools(), vec!["file".to_string()]);
}

#[test]
fn test_load_mode_from_toml() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let mode_path = temp_dir.path().join("test-mode.toml");

    let toml_content = r#"
name = "test-mode"
description = "A test mode"
system_prompt = "You are a test assistant for testing."
temperature = 0.75
tools = ["file", "git"]
"#;

    std::fs::write(&mode_path, toml_content).expect("Failed to write mode file");

    let mode = AgentMode::load_from_file(&mode_path).expect("Failed to load mode");

    assert_eq!(mode.name(), "test-mode");
    assert_eq!(mode.description(), "A test mode");
    assert_eq!(
        mode.system_prompt(),
        "You are a test assistant for testing."
    );
    assert_eq!(mode.temperature(), 0.75);
    assert_eq!(mode.tools(), vec!["file".to_string(), "git".to_string()]);
}

#[test]
fn test_list_modes() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let modes_dir = temp_dir.path().join("modes");
    std::fs::create_dir_all(&modes_dir).expect("Failed to create modes dir");

    // Create a custom mode
    let custom_mode_path = modes_dir.join("custom.toml");
    let custom_mode_content = r#"
name = "custom"
description = "Custom mode"
system_prompt = "Custom prompt"
"#;
    std::fs::write(&custom_mode_path, custom_mode_content).expect("Failed to write custom mode");

    let modes = AgentMode::list_all(&modes_dir).expect("Failed to list modes");

    // Should include all built-in modes
    assert!(modes.iter().any(|(name, _)| name == "code"));
    assert!(modes.iter().any(|(name, _)| name == "architect"));
    assert!(modes.iter().any(|(name, _)| name == "debug"));
    assert!(modes.iter().any(|(name, _)| name == "review"));
    assert!(modes.iter().any(|(name, _)| name == "refactor"));
    assert!(modes.iter().any(|(name, _)| name == "test"));

    // Should include custom mode
    assert!(modes.iter().any(|(name, _)| name == "custom"));
}

#[test]
fn test_load_by_name_builtin() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let modes_dir = temp_dir.path().join("modes");

    let mode = AgentMode::load_by_name("code", &modes_dir).expect("Failed to load code mode");
    assert_eq!(mode.name(), "code");

    let mode =
        AgentMode::load_by_name("architect", &modes_dir).expect("Failed to load architect mode");
    assert_eq!(mode.name(), "architect");
}

#[test]
fn test_load_by_name_custom() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let modes_dir = temp_dir.path().join("modes");
    std::fs::create_dir_all(&modes_dir).expect("Failed to create modes dir");

    // Create a custom mode
    let custom_mode_path = modes_dir.join("my-custom.toml");
    let custom_mode_content = r#"
name = "my-custom"
description = "My custom mode"
system_prompt = "Custom prompt for my mode"
"#;
    std::fs::write(&custom_mode_path, custom_mode_content).expect("Failed to write custom mode");

    let mode =
        AgentMode::load_by_name("my-custom", &modes_dir).expect("Failed to load custom mode");
    assert_eq!(mode.name(), "my-custom");
    assert_eq!(mode.description(), "My custom mode");
}

#[test]
fn test_mode_display() {
    let mode = AgentMode::Code;
    assert_eq!(format!("{mode}"), "code");

    let mode = AgentMode::Architect;
    assert_eq!(format!("{mode}"), "architect");
}

#[test]
fn test_mode_default() {
    let mode = AgentMode::default();
    assert_eq!(mode, AgentMode::Code);
}

#[test]
fn test_invalid_mode_from_str() {
    let mode = AgentMode::parse_mode("invalid-mode");
    assert!(mode.is_none());
}

#[test]
fn test_mode_equality() {
    assert_eq!(AgentMode::Code, AgentMode::Code);
    assert_ne!(AgentMode::Code, AgentMode::Architect);
}
