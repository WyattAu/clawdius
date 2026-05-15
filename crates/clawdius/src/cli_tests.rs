#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::cli::{Cli, Commands, OutputFormat};

use clap::Parser;

/// Helper: parse CLI args
fn parse(args: &[&str]) -> Cli {
    Cli::try_parse_from(args).expect("failed to parse CLI args")
}

// ─── Global flags ──────────────────────────────────────────────

#[test]
fn test_empty_args() {
    let cli = parse(&["clawdius"]);
    assert!(cli.command.is_none());
    assert!(!cli.no_tui);
    assert_eq!(cli.output_format, OutputFormat::Text);
    assert!(!cli.quiet);
    assert!(cli.config.is_none());
    assert!(cli.lang.is_none());
}

#[test]
fn test_no_tui_flag() {
    let cli = parse(&["clawdius", "--no-tui"]);
    assert!(cli.no_tui);
}

#[test]
fn test_quiet_flag() {
    let cli = parse(&["clawdius", "-q"]);
    assert!(cli.quiet);
}

#[test]
fn test_cwd_default() {
    let cli = parse(&["clawdius"]);
    assert_eq!(cli.cwd.as_os_str(), ".");
}

#[test]
fn test_cwd_custom() {
    let cli = parse(&["clawdius", "--cwd", "/tmp/project"]);
    assert_eq!(cli.cwd.as_os_str(), "/tmp/project");
}

#[test]
fn test_output_format_text() {
    let cli = parse(&["clawdius", "-f", "text"]);
    assert_eq!(cli.output_format, OutputFormat::Text);
}

#[test]
fn test_output_format_json() {
    let cli = parse(&["clawdius", "-f", "json"]);
    assert_eq!(cli.output_format, OutputFormat::Json);
}

#[test]
fn test_output_format_stream_json() {
    let cli = parse(&["clawdius", "-f", "stream-json"]);
    assert_eq!(cli.output_format, OutputFormat::StreamJson);
}

#[test]
fn test_config_path() {
    let cli = parse(&["clawdius", "-C", "/etc/clawdius/config.toml"]);
    assert_eq!(
        cli.config.as_ref().expect("config").to_str().expect("utf8"),
        "/etc/clawdius/config.toml"
    );
}

#[test]
fn test_lang_flag() {
    let cli = parse(&["clawdius", "-L", "ja"]);
    assert_eq!(cli.lang.as_deref(), Some("ja"));
}

#[test]
fn test_combined_global_flags() {
    let cli = parse(&["clawdius", "-q", "--no-tui", "-f", "json", "-L", "zh"]);
    assert!(cli.quiet);
    assert!(cli.no_tui);
    assert_eq!(cli.output_format, OutputFormat::Json);
    assert_eq!(cli.lang.as_deref(), Some("zh"));
}

// ─── Chat command ──────────────────────────────────────────────

#[test]
fn test_chat_subcommand() {
    let cli = parse(&["clawdius", "chat", "hello world"]);
    match cli.command.expect("command") {
        Commands::Chat { prompt, .. } => assert_eq!(prompt.as_deref(), Some("hello world")),
        other => panic!("expected Chat, got {other:?}"),
    }
}

#[test]
fn test_chat_with_model() {
    let cli = parse(&["clawdius", "chat", "--model", "claude-3-opus", "test"]);
    match cli.command.expect("command") {
        Commands::Chat { model, .. } => assert_eq!(model.as_deref(), Some("claude-3-opus")),
        other => panic!("expected Chat, got {other:?}"),
    }
}

#[test]
fn test_chat_provider_default() {
    let cli = parse(&["clawdius", "chat", "test"]);
    match cli.command.expect("command") {
        Commands::Chat { provider, .. } => assert_eq!(provider, "anthropic"),
        other => panic!("expected Chat, got {other:?}"),
    }
}

#[test]
fn test_chat_provider_custom() {
    let cli = parse(&["clawdius", "chat", "-P", "ollama", "test"]);
    match cli.command.expect("command") {
        Commands::Chat { provider, .. } => assert_eq!(provider, "ollama"),
        other => panic!("expected Chat, got {other:?}"),
    }
}

#[test]
fn test_chat_mode_default() {
    let cli = parse(&["clawdius", "chat", "test"]);
    match cli.command.expect("command") {
        Commands::Chat { mode, .. } => assert_eq!(mode, "code"),
        other => panic!("expected Chat, got {other:?}"),
    }
}

#[test]
fn test_chat_auto_approve() {
    let cli = parse(&["clawdius", "chat", "--auto-approve", "test"]);
    match cli.command.expect("command") {
        Commands::Chat { auto_approve, .. } => assert!(auto_approve),
        other => panic!("expected Chat, got {other:?}"),
    }
}

// ─── Auto command ──────────────────────────────────────────────

#[test]
fn test_auto_subcommand() {
    let cli = parse(&["clawdius", "auto", "fix the tests"]);
    match cli.command.expect("command") {
        Commands::Auto { task, .. } => assert_eq!(task, "fix the tests"),
        other => panic!("expected Auto, got {other:?}"),
    }
}

#[test]
fn test_auto_max_iterations() {
    let cli = parse(&["clawdius", "auto", "task", "--max-iterations", "10"]);
    match cli.command.expect("command") {
        Commands::Auto { max_iterations, .. } => assert_eq!(max_iterations, Some(10)),
        other => panic!("expected Auto, got {other:?}"),
    }
}

// ─── Setup / Init commands ─────────────────────────────────────

#[test]
fn test_setup_subcommand() {
    let cli = parse(&["clawdius", "setup"]);
    matches!(cli.command.expect("command"), Commands::Setup { .. });
}

#[test]
fn test_init_subcommand() {
    let cli = parse(&["clawdius", "init"]);
    matches!(cli.command.expect("command"), Commands::Init { .. });
}

// ─── Sessions command ──────────────────────────────────────────

#[test]
fn test_sessions_subcommand() {
    let cli = parse(&["clawdius", "sessions"]);
    matches!(cli.command.expect("command"), Commands::Sessions { .. });
}

// ─── Generate / Test / Doc commands ────────────────────────────

#[test]
fn test_generate_subcommand() {
    let cli = parse(&["clawdius", "generate", "add error handling"]);
    match cli.command.expect("command") {
        Commands::Generate { prompt, .. } => assert_eq!(prompt, "add error handling"),
        other => panic!("expected Generate, got {other:?}"),
    }
}

#[test]
fn test_test_subcommand() {
    let cli = parse(&["clawdius", "test", "src/main.rs"]);
    match cli.command.expect("command") {
        Commands::Test { file, .. } => assert_eq!(file.as_os_str(), "src/main.rs"),
        other => panic!("expected Test, got {other:?}"),
    }
}

#[test]
fn test_doc_subcommand() {
    let cli = parse(&["clawdius", "doc", "src/lib.rs"]);
    match cli.command.expect("command") {
        Commands::Doc { file, .. } => assert_eq!(file.as_os_str(), "src/lib.rs"),
        other => panic!("expected Doc, got {other:?}"),
    }
}

#[test]
fn test_verify_subcommand() {
    // proof is a required positional arg
    let cli = parse(&["clawdius", "verify", "--proof", "proofs/session.lean"]);
    match cli.command.expect("command") {
        Commands::Verify { proof, .. } => assert_eq!(proof.as_os_str(), "proofs/session.lean"),
        other => panic!("expected Verify, got {other:?}"),
    }
}

// ─── Metrics command ───────────────────────────────────────────

#[test]
fn test_metrics_subcommand() {
    let cli = parse(&["clawdius", "metrics"]);
    matches!(cli.command.expect("command"), Commands::Metrics { .. });
}

// ─── Edit command ──────────────────────────────────────────────

#[test]
fn test_edit_subcommand() {
    let cli = parse(&["clawdius", "edit"]);
    matches!(cli.command.expect("command"), Commands::Edit { .. });
}

// ─── Sprint command ────────────────────────────────────────────

#[test]
fn test_sprint_subcommand() {
    let cli = parse(&["clawdius", "sprint", "implement auth"]);
    match cli.command.expect("command") {
        Commands::Sprint { task, .. } => assert_eq!(task, "implement auth"),
        other => panic!("expected Sprint, got {other:?}"),
    }
}

// ─── Ship command ──────────────────────────────────────────────

#[test]
fn test_ship_subcommand_requires_subcommand() {
    let result = Cli::try_parse_from(&["clawdius", "ship"]);
    assert!(result.is_err());
}

// ─── Git commands ──────────────────────────────────────────────

#[test]
fn test_git_subcommand_requires_subcommand() {
    // git requires a sub-subcommand, so bare invocation fails
    let result = Cli::try_parse_from(&["clawdius", "git"]);
    assert!(result.is_err());
}

#[test]
fn test_git_commit_subcommand() {
    let cli = parse(&["clawdius", "git", "commit", "file1.rs", "file2.rs"]);
    matches!(cli.command.expect("command"), Commands::Git { .. });
}

// ─── Server command ────────────────────────────────────────────

#[test]
fn test_server_subcommand() {
    let cli = parse(&["clawdius", "server"]);
    matches!(cli.command.expect("command"), Commands::Server { .. });
}

// ─── Complete command ──────────────────────────────────────────

#[test]
fn test_complete_subcommand() {
    // file, line, character are positional args
    let cli = parse(&["clawdius", "complete", "main.rs", "10", "5"]);
    match cli.command.expect("command") {
        Commands::Complete {
            file,
            line,
            character,
            ..
        } => {
            assert_eq!(file, "main.rs");
            assert_eq!(line, 10);
            assert_eq!(character, 5);
        },
        other => panic!("expected Complete, got {other:?}"),
    }
}

// ─── Memory commands ───────────────────────────────────────────

#[test]
fn test_memory_show() {
    let cli = parse(&["clawdius", "memory", "show"]);
    matches!(cli.command.expect("command"), Commands::Memory { .. });
}

// ─── Modes command ─────────────────────────────────────────────

#[test]
fn test_modes_subcommand_requires_subcommand() {
    let result = Cli::try_parse_from(&["clawdius", "modes"]);
    assert!(result.is_err());
}

// ─── Lang command ──────────────────────────────────────────────

#[test]
fn test_lang_subcommand_requires_subcommand() {
    let result = Cli::try_parse_from(&["clawdius", "lang"]);
    assert!(result.is_err());
}

// ─── Webhook command ───────────────────────────────────────────

#[test]
fn test_webhook_subcommand_requires_subcommand() {
    let result = Cli::try_parse_from(&["clawdius", "webhook"]);
    assert!(result.is_err());
}

// ─── Invalid args ──────────────────────────────────────────────

#[test]
fn test_invalid_output_format() {
    let result = Cli::try_parse_from(&["clawdius", "-f", "xml"]);
    assert!(result.is_err());
}

#[test]
fn test_unknown_command() {
    let result = Cli::try_parse_from(&["clawdius", "nonexistent"]);
    assert!(result.is_err());
}

#[test]
fn test_version_flag() {
    let result = Cli::try_parse_from(&["clawdius", "--version"]);
    // clap exits on --version, but try_parse_from returns Err
    assert!(result.is_err());
}

#[test]
fn test_help_flag() {
    let result = Cli::try_parse_from(&["clawdius", "--help"]);
    assert!(result.is_err());
}

// ─── OutputFormat enum ─────────────────────────────────────────

#[test]
fn test_output_format_default() {
    assert_eq!(OutputFormat::default(), OutputFormat::Text);
}

#[test]
fn test_output_format_roundtrip() {
    for fmt in [
        OutputFormat::Text,
        OutputFormat::Json,
        OutputFormat::StreamJson,
    ] {
        let core: clawdius_core::output::OutputFormat = fmt.into();
        let _ = core; // verify conversion compiles
    }
}
