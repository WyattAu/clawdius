#![allow(
    dead_code,
    unused_variables,
    clippy::cast_precision_loss,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    clippy::items_after_statements,
    clippy::manual_is_multiple_of,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use
)]

use clap::Parser;
use clawdius::cli::{Cli, Commands, OutputFormat};

#[test]
fn test_cli_default_values() {
    let cli = Cli::parse_from(["clawdius"]);
    assert!(cli.command.is_none());
    assert!(!cli.no_tui);
    assert_eq!(cli.cwd.to_string_lossy(), ".");
    assert_eq!(cli.output_format, OutputFormat::Text);
    assert!(!cli.quiet);
    assert!(cli.config.is_none());
    assert!(cli.lang.is_none());
}

#[test]
fn test_cli_chat_subcommand() {
    let cli = Cli::parse_from(["clawdius", "chat", "hello"]);
    match cli.command {
        Some(Commands::Chat { prompt, .. }) => {
            assert_eq!(prompt.as_deref(), Some("hello"));
        },
        other => panic!("expected Chat command, got: {other:?}"),
    }
}

#[test]
fn test_cli_server_subcommand() {
    let cli = Cli::parse_from(["clawdius", "server", "--port", "9090"]);
    match cli.command {
        Some(Commands::Server { port, .. }) => {
            assert_eq!(port, 9090);
        },
        other => panic!("expected Server command, got: {other:?}"),
    }
}

#[test]
fn test_cli_setup_subcommand() {
    let cli = Cli::parse_from(["clawdius", "setup"]);
    match cli.command {
        Some(Commands::Setup { quick, provider }) => {
            assert!(!quick);
            assert!(provider.is_none());
        },
        other => panic!("expected Setup command, got: {other:?}"),
    }
}

#[test]
fn test_cli_init_subcommand() {
    let cli = Cli::parse_from(["clawdius", "init"]);
    match cli.command {
        Some(Commands::Init { name }) => {
            assert!(name.is_none());
        },
        other => panic!("expected Init command, got: {other:?}"),
    }
}

#[test]
fn test_cli_init_with_name() {
    let cli = Cli::parse_from(["clawdius", "init", "my-project"]);
    match cli.command {
        Some(Commands::Init { name }) => {
            assert_eq!(name.as_deref(), Some("my-project"));
        },
        other => panic!("expected Init command, got: {other:?}"),
    }
}

#[test]
fn test_cli_sessions_subcommand() {
    let cli = Cli::parse_from(["clawdius", "sessions"]);
    match cli.command {
        Some(Commands::Sessions { delete, search }) => {
            assert!(delete.is_none());
            assert!(search.is_none());
        },
        other => panic!("expected Sessions command, got: {other:?}"),
    }
}

#[test]
fn test_cli_sprint_subcommand() {
    let cli = Cli::parse_from(["clawdius", "sprint", "fix bug"]);
    match cli.command {
        Some(Commands::Sprint { task, .. }) => {
            assert_eq!(task, "fix bug");
        },
        other => panic!("expected Sprint command, got: {other:?}"),
    }
}

#[test]
fn test_cli_metrics_subcommand() {
    let cli = Cli::parse_from(["clawdius", "metrics"]);
    match cli.command {
        Some(Commands::Metrics { reset, watch, .. }) => {
            assert!(!reset);
            assert!(!watch);
        },
        other => panic!("expected Metrics command, got: {other:?}"),
    }
}

#[test]
fn test_cli_checkpoint_create_subcommand() {
    let cli = Cli::parse_from(["clawdius", "checkpoint", "create", "save point"]);
    match cli.command {
        Some(Commands::Checkpoint { .. }) => {},
        other => panic!("expected Checkpoint command, got: {other:?}"),
    }
}

#[test]
fn test_cli_checkpoint_list_subcommand() {
    let cli = Cli::parse_from(["clawdius", "checkpoint", "list"]);
    match cli.command {
        Some(Commands::Checkpoint { .. }) => {},
        other => panic!("expected Checkpoint command, got: {other:?}"),
    }
}

#[test]
fn test_cli_timeline_list_subcommand() {
    let cli = Cli::parse_from(["clawdius", "timeline", "list"]);
    match cli.command {
        Some(Commands::Timeline { .. }) => {},
        other => panic!("expected Timeline command, got: {other:?}"),
    }
}

#[test]
fn test_cli_version_flag() {
    let result = Cli::try_parse_from(["clawdius", "--version"]);
    assert!(result.is_err());
}

#[test]
fn test_cli_help_flag() {
    let result = Cli::try_parse_from(["clawdius", "--help"]);
    assert!(result.is_err());
}

#[test]
fn test_cli_auto_subcommand() {
    let cli = Cli::parse_from(["clawdius", "auto", "fix tests"]);
    match cli.command {
        Some(Commands::Auto { task, .. }) => {
            assert_eq!(task, "fix tests");
        },
        other => panic!("expected Auto command, got: {other:?}"),
    }
}

#[test]
fn test_cli_generate_subcommand() {
    let cli = Cli::parse_from(["clawdius", "generate", "tests for foo"]);
    match cli.command {
        Some(Commands::Generate { prompt, .. }) => {
            assert_eq!(prompt, "tests for foo");
        },
        other => panic!("expected Generate command, got: {other:?}"),
    }
}

#[test]
fn test_cli_modes_list_subcommand() {
    let cli = Cli::parse_from(["clawdius", "modes", "list"]);
    match cli.command {
        Some(Commands::Modes { .. }) => {},
        other => panic!("expected Modes command, got: {other:?}"),
    }
}

#[test]
fn test_cli_analyze_subcommand() {
    let cli = Cli::parse_from(["clawdius", "analyze", "main.rs"]);
    match cli.command {
        Some(Commands::Analyze { path, .. }) => {
            assert_eq!(path.to_string_lossy(), "main.rs");
        },
        other => panic!("expected Analyze command, got: {other:?}"),
    }
}

#[test]
fn test_cli_models_list_subcommand() {
    let cli = Cli::parse_from(["clawdius", "models", "list"]);
    match cli.command {
        Some(Commands::Models { .. }) => {},
        other => panic!("expected Models command, got: {other:?}"),
    }
}

#[test]
fn test_cli_config_show_subcommand() {
    let cli = Cli::parse_from(["clawdius", "config", "show"]);
    match cli.command {
        Some(Commands::Config { .. }) => {},
        other => panic!("expected Config command, got: {other:?}"),
    }
}

#[test]
fn test_cli_output_format_json() {
    let cli = Cli::parse_from(["clawdius", "-f", "json", "chat", "hi"]);
    assert_eq!(cli.output_format, OutputFormat::Json);
}

#[test]
fn test_cli_output_format_text() {
    let cli = Cli::parse_from(["clawdius", "-f", "text", "chat", "hi"]);
    assert_eq!(cli.output_format, OutputFormat::Text);
}

#[test]
fn test_cli_output_format_stream_json() {
    let cli = Cli::parse_from(["clawdius", "-f", "stream-json", "chat", "hi"]);
    assert_eq!(cli.output_format, OutputFormat::StreamJson);
}

#[test]
fn test_cli_model_flag() {
    let cli = Cli::parse_from([
        "clawdius",
        "chat",
        "hi",
        "--model",
        "claude-sonnet-4-20250514",
    ]);
    match cli.command {
        Some(Commands::Chat { model, .. }) => {
            assert_eq!(model.as_deref(), Some("claude-sonnet-4-20250514"));
        },
        other => panic!("expected Chat command, got: {other:?}"),
    }
}

#[test]
fn test_cli_provider_flag() {
    let cli = Cli::parse_from(["clawdius", "chat", "hi", "--provider", "anthropic"]);
    match cli.command {
        Some(Commands::Chat { provider, .. }) => {
            assert_eq!(provider, "anthropic");
        },
        other => panic!("expected Chat command, got: {other:?}"),
    }
}

#[test]
fn test_cli_session_flag() {
    let cli = Cli::parse_from(["clawdius", "chat", "hi", "--session", "abc123"]);
    match cli.command {
        Some(Commands::Chat { session, .. }) => {
            assert_eq!(session.as_deref(), Some("abc123"));
        },
        other => panic!("expected Chat command, got: {other:?}"),
    }
}

#[test]
fn test_cli_refactor_subcommand() {
    let cli = Cli::parse_from([
        "clawdius",
        "refactor",
        "--from",
        "typescript",
        "--to",
        "rust",
    ]);
    match cli.command {
        Some(Commands::Refactor { from, to, .. }) => {
            assert_eq!(from, "typescript");
            assert_eq!(to, "rust");
        },
        other => panic!("expected Refactor command, got: {other:?}"),
    }
}

#[test]
fn test_cli_verify_subcommand() {
    let cli = Cli::parse_from(["clawdius", "verify", "--proof", "proof.lean"]);
    match cli.command {
        Some(Commands::Verify { proof, .. }) => {
            assert_eq!(proof.to_string_lossy(), "proof.lean");
        },
        other => panic!("expected Verify command, got: {other:?}"),
    }
}

#[test]
fn test_cli_doc_subcommand() {
    let cli = Cli::parse_from(["clawdius", "doc", "README.md"]);
    match cli.command {
        Some(Commands::Doc { file, .. }) => {
            assert_eq!(file.to_string_lossy(), "README.md");
        },
        other => panic!("expected Doc command, got: {other:?}"),
    }
}

#[test]
fn test_cli_telemetry_subcommand() {
    let cli = Cli::parse_from(["clawdius", "telemetry"]);
    match cli.command {
        Some(Commands::Telemetry {
            enable,
            disable,
            enable_metrics,
            enable_crash_reporting,
        }) => {
            assert!(!enable);
            assert!(!disable);
            assert!(!enable_metrics);
            assert!(!enable_crash_reporting);
        },
        other => panic!("expected Telemetry command, got: {other:?}"),
    }
}

#[test]
fn test_cli_watch_subcommand() {
    let cli = Cli::parse_from(["clawdius", "watch"]);
    match cli.command {
        Some(Commands::Watch { path, .. }) => {
            assert_eq!(path.to_string_lossy(), ".");
        },
        other => panic!("expected Watch command, got: {other:?}"),
    }
}

#[test]
fn test_cli_server_host_port_flags() {
    let cli = Cli::parse_from(["clawdius", "server", "--host", "0.0.0.0", "--port", "9090"]);
    match cli.command {
        Some(Commands::Server { host, port }) => {
            assert_eq!(host, "0.0.0.0");
            assert_eq!(port, 9090);
        },
        other => panic!("expected Server command, got: {other:?}"),
    }
}

#[test]
fn test_cli_invalid_flag() {
    let result = Cli::try_parse_from(["clawdius", "--nonexistent-flag"]);
    assert!(result.is_err());
}

#[test]
fn test_cli_no_tui_flag() {
    let cli = Cli::parse_from(["clawdius", "--no-tui"]);
    assert!(cli.no_tui);
}

#[test]
fn test_cli_quiet_flag() {
    let cli = Cli::parse_from(["clawdius", "--quiet"]);
    assert!(cli.quiet);
}

#[test]
fn test_cli_cwd_flag() {
    let cli = Cli::parse_from(["clawdius", "--cwd", "/tmp/project"]);
    assert_eq!(cli.cwd.to_string_lossy(), "/tmp/project");
}

#[test]
fn test_cli_config_flag() {
    let cli = Cli::parse_from(["clawdius", "--config", "/path/to/config.toml"]);
    assert_eq!(
        cli.config
            .as_deref()
            .map(|p| p.to_string_lossy().to_string()),
        Some("/path/to/config.toml".to_string())
    );
}

#[test]
fn test_cli_chat_mode_flag() {
    let cli = Cli::parse_from(["clawdius", "chat", "hi", "--mode", "debug"]);
    match cli.command {
        Some(Commands::Chat { mode, .. }) => {
            assert_eq!(mode, "debug");
        },
        other => panic!("expected Chat command, got: {other:?}"),
    }
}

#[test]
fn test_cli_chat_editor_flag() {
    let cli = Cli::parse_from(["clawdius", "chat", "--editor"]);
    match cli.command {
        Some(Commands::Chat { editor, .. }) => {
            assert!(editor);
        },
        other => panic!("expected Chat command, got: {other:?}"),
    }
}

#[test]
fn test_cli_chat_exit_flag() {
    let cli = Cli::parse_from(["clawdius", "chat", "hi", "--exit"]);
    match cli.command {
        Some(Commands::Chat { exit, .. }) => {
            assert!(exit);
        },
        other => panic!("expected Chat command, got: {other:?}"),
    }
}

#[test]
fn test_cli_auto_with_run_tests() {
    let cli = Cli::parse_from(["clawdius", "auto", "fix tests", "--run-tests"]);
    match cli.command {
        Some(Commands::Auto { run_tests, .. }) => {
            assert!(run_tests);
        },
        other => panic!("expected Auto command, got: {other:?}"),
    }
}

#[test]
fn test_cli_auto_with_auto_commit() {
    let cli = Cli::parse_from(["clawdius", "auto", "fix tests", "--auto-commit"]);
    match cli.command {
        Some(Commands::Auto { auto_commit, .. }) => {
            assert!(auto_commit);
        },
        other => panic!("expected Auto command, got: {other:?}"),
    }
}

#[test]
fn test_cli_sprint_with_max_iterations() {
    let cli = Cli::parse_from(["clawdius", "sprint", "task", "--max-iterations", "10"]);
    match cli.command {
        Some(Commands::Sprint { max_iterations, .. }) => {
            assert_eq!(max_iterations, 10);
        },
        other => panic!("expected Sprint command, got: {other:?}"),
    }
}

#[test]
fn test_cli_sprint_with_provider() {
    let cli = Cli::parse_from(["clawdius", "sprint", "task", "--provider", "anthropic"]);
    match cli.command {
        Some(Commands::Sprint { provider, .. }) => {
            assert_eq!(provider, "anthropic");
        },
        other => panic!("expected Sprint command, got: {other:?}"),
    }
}

#[test]
fn test_cli_edit_subcommand() {
    let cli = Cli::parse_from(["clawdius", "edit", "--initial", "hello"]);
    match cli.command {
        Some(Commands::Edit { initial, .. }) => {
            assert_eq!(initial.as_deref(), Some("hello"));
        },
        other => panic!("expected Edit command, got: {other:?}"),
    }
}

#[test]
fn test_cli_git_diff_subcommand() {
    let cli = Cli::parse_from(["clawdius", "git", "diff", "--staged"]);
    match cli.command {
        Some(Commands::Git { .. }) => {},
        other => panic!("expected Git command, got: {other:?}"),
    }
}

#[test]
fn test_cli_lang_list_subcommand() {
    let cli = Cli::parse_from(["clawdius", "lang", "list"]);
    match cli.command {
        Some(Commands::Lang { .. }) => {},
        other => panic!("expected Lang command, got: {other:?}"),
    }
}

#[test]
fn test_cli_memory_show_subcommand() {
    let cli = Cli::parse_from(["clawdius", "memory", "show"]);
    match cli.command {
        Some(Commands::Memory { .. }) => {},
        other => panic!("expected Memory command, got: {other:?}"),
    }
}
