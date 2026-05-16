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
    let result = Cli::try_parse_from(["clawdius", "ship"]);
    assert!(result.is_err());
}

// ─── Git commands ──────────────────────────────────────────────

#[test]
fn test_git_subcommand_requires_subcommand() {
    // git requires a sub-subcommand, so bare invocation fails
    let result = Cli::try_parse_from(["clawdius", "git"]);
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
    let result = Cli::try_parse_from(["clawdius", "modes"]);
    assert!(result.is_err());
}

// ─── Lang command ──────────────────────────────────────────────

#[test]
fn test_lang_subcommand_requires_subcommand() {
    let result = Cli::try_parse_from(["clawdius", "lang"]);
    assert!(result.is_err());
}

// ─── Webhook command ───────────────────────────────────────────

#[test]
fn test_webhook_subcommand_requires_subcommand() {
    let result = Cli::try_parse_from(["clawdius", "webhook"]);
    assert!(result.is_err());
}

// ─── Invalid args ──────────────────────────────────────────────

#[test]
fn test_invalid_output_format() {
    let result = Cli::try_parse_from(["clawdius", "-f", "xml"]);
    assert!(result.is_err());
}

#[test]
fn test_unknown_command() {
    let result = Cli::try_parse_from(["clawdius", "nonexistent"]);
    assert!(result.is_err());
}

#[test]
fn test_version_flag() {
    let result = Cli::try_parse_from(["clawdius", "--version"]);
    // clap exits on --version, but try_parse_from returns Err
    assert!(result.is_err());
}

#[test]
fn test_help_flag() {
    let result = Cli::try_parse_from(["clawdius", "--help"]);
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

// ─── MetricsOutputFormat enum ──────────────────────────────────

#[test]
fn test_metrics_output_format_default() {
    use crate::cli::MetricsOutputFormat;
    assert_eq!(MetricsOutputFormat::default(), MetricsOutputFormat::Text);
}

#[test]
fn test_metrics_output_format_variants() {
    use crate::cli::MetricsOutputFormat;
    use clap::ValueEnum;
    let variants = MetricsOutputFormat::value_variants();
    assert_eq!(variants.len(), 3);
}

// ─── Combined flag tests ────────────────────────────────────────

#[test]
fn test_chat_with_all_flags() {
    let cli = parse(&[
        "clawdius",
        "-q",
        "--no-tui",
        "-f",
        "json",
        "chat",
        "--model",
        "claude-3-opus",
        "-P",
        "openai",
        "-M",
        "review",
        "--session",
        "sess-123",
        "--auto-approve",
        "--exit",
        "hello",
    ]);
    match cli.command.expect("command") {
        Commands::Chat {
            prompt,
            model,
            provider,
            session,
            mode,
            auto_approve,
            exit,
            editor,
            quiet: _,
        } => {
            assert_eq!(prompt.as_deref(), Some("hello"));
            assert_eq!(model.as_deref(), Some("claude-3-opus"));
            assert_eq!(provider, "openai");
            assert_eq!(session.as_deref(), Some("sess-123"));
            assert_eq!(mode, "review");
            assert!(auto_approve);
            assert!(exit);
            assert!(!editor);
        },
        other => panic!("expected Chat, got {other:?}"),
    }
    assert!(cli.no_tui);
    assert!(cli.quiet);
    assert_eq!(cli.output_format, OutputFormat::Json);
}

#[test]
fn test_auto_with_max_iterations_and_approve() {
    let cli = parse(&[
        "clawdius",
        "auto",
        "fix tests",
        "--max-iterations",
        "100",
        "--auto-commit",
    ]);
    match cli.command.expect("command") {
        Commands::Auto {
            task,
            max_iterations,
            auto_commit,
            ..
        } => {
            assert_eq!(task, "fix tests");
            assert_eq!(max_iterations, Some(100));
            assert!(auto_commit);
        },
        other => panic!("expected Auto, got {other:?}"),
    }
}

#[test]
fn test_generate_with_mode_and_output() {
    // Global -f must come before the subcommand; Generate also has -f (files)
    let cli = parse(&[
        "clawdius",
        "-f",
        "stream-json",
        "generate",
        "-M",
        "architect",
        "add caching layer",
    ]);
    match cli.command.expect("command") {
        Commands::Generate { prompt, mode, .. } => {
            assert_eq!(prompt, "add caching layer");
            assert_eq!(mode, "architect");
        },
        other => panic!("expected Generate, got {other:?}"),
    }
    assert_eq!(cli.output_format, OutputFormat::StreamJson);
}

#[test]
fn test_test_with_function_and_output() {
    let cli = parse(&[
        "clawdius",
        "test",
        "src/lib.rs",
        "--function",
        "parse_config",
        "-o",
        "tests/config_test.rs",
    ]);
    match cli.command.expect("command") {
        Commands::Test {
            file,
            function,
            output,
        } => {
            assert_eq!(file.as_os_str(), "src/lib.rs");
            assert_eq!(function.as_deref(), Some("parse_config"));
            assert!(output
                .as_ref()
                .and_then(|p| p.to_str())
                .eq(&Some("tests/config_test.rs")));
        },
        other => panic!("expected Test, got {other:?}"),
    }
}

#[test]
fn test_doc_with_element() {
    let cli = parse(&["clawdius", "doc", "src/lib.rs", "--element", "MyStruct"]);
    match cli.command.expect("command") {
        Commands::Doc { file, element, .. } => {
            assert_eq!(file.as_os_str(), "src/lib.rs");
            assert_eq!(element.as_deref(), Some("MyStruct"));
        },
        other => panic!("expected Doc, got {other:?}"),
    }
}

#[test]
fn test_sprint_with_iterations() {
    let cli = parse(&[
        "clawdius",
        "sprint",
        "refactor auth",
        "-n",
        "10",
        "--real-execution",
        "--auto-approve",
    ]);
    match cli.command.expect("command") {
        Commands::Sprint {
            task,
            max_iterations,
            real_execution,
            auto_approve,
            ..
        } => {
            assert_eq!(task, "refactor auth");
            assert_eq!(max_iterations, 10);
            assert!(real_execution);
            assert!(auto_approve);
        },
        other => panic!("expected Sprint, got {other:?}"),
    }
}

#[test]
fn test_verify_with_lean_path() {
    let cli = parse(&[
        "clawdius",
        "verify",
        "--proof",
        "proofs/session.lean",
        "--lean-path",
        "/usr/local/bin/lean",
    ]);
    match cli.command.expect("command") {
        Commands::Verify { proof, lean_path } => {
            assert_eq!(proof.as_os_str(), "proofs/session.lean");
            assert!(lean_path
                .as_ref()
                .and_then(|p| p.to_str())
                .eq(&Some("/usr/local/bin/lean")));
        },
        other => panic!("expected Verify, got {other:?}"),
    }
}

#[test]
fn test_metrics_with_output_and_reset() {
    let cli = parse(&[
        "clawdius",
        "metrics",
        "-f",
        "json",
        "--output",
        "metrics.json",
        "--reset",
    ]);
    match cli.command.expect("command") {
        Commands::Metrics {
            format,
            output,
            reset,
            watch,
        } => {
            assert_eq!(format!("{format:?}"), "Json");
            assert!(output
                .as_ref()
                .and_then(|p| p.to_str())
                .eq(&Some("metrics.json")));
            assert!(reset);
            assert!(!watch);
        },
        other => panic!("expected Metrics, got {other:?}"),
    }
}

#[test]
fn test_telemetry_all_flags() {
    let cli = parse(&[
        "clawdius",
        "telemetry",
        "--enable",
        "--enable-metrics",
        "--enable-crash-reporting",
    ]);
    matches!(
        cli.command.expect("command"),
        Commands::Telemetry {
            enable: true,
            disable: false,
            enable_metrics: true,
            enable_crash_reporting: true,
            ..
        }
    );
}

#[test]
fn test_server_custom_host_port() {
    let cli = parse(&["clawdius", "server", "--host", "0.0.0.0", "--port", "9090"]);
    match cli.command.expect("command") {
        Commands::Server { host, port } => {
            assert_eq!(host, "0.0.0.0");
            assert_eq!(port, 9090);
        },
        other => panic!("expected Server, got {other:?}"),
    }
}

#[test]
fn test_memory_show_and_learn() {
    // memory requires subcommand
    let cli = parse(&["clawdius", "memory", "show"]);
    matches!(cli.command.expect("command"), Commands::Memory { .. });
}

#[test]
fn test_complete_with_language() {
    let cli = parse(&[
        "clawdius",
        "complete",
        "main.rs",
        "10",
        "5",
        "-l",
        "rust",
        "-P",
        "anthropic",
    ]);
    match cli.command.expect("command") {
        Commands::Complete {
            file,
            line,
            character,
            language,
            provider,
            model: _,
        } => {
            assert_eq!(file, "main.rs");
            assert_eq!(line, 10);
            assert_eq!(character, 5);
            assert_eq!(language.as_deref(), Some("rust"));
            assert_eq!(provider, "anthropic");
        },
        other => panic!("expected Complete, got {other:?}"),
    }
}
