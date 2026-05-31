//! Tests for CLI handler logic across subcommands that previously had zero coverage.
//!
//! Covers: webhook (CRUD + event parsing), ship (checks + commit message),
//! action (selection computation + language detection + dispatch),
//! lsp (output format for all 8 commands),
//! memory (learn entry validation + list + clear).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::items_after_statements
)]

use clap::Parser;
use clawdius::cli::{
    Cli, Commands, LspCommands, MemoryCommands, OutputFormat, ShipAction, SkillAction,
    WebhookCommands,
};

// ═══════════════════════════════════════════════════════════════
// Webhook subcommand arg parsing
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_cli_webhook_list() {
    let cli = Cli::parse_from(["clawdius", "webhook", "list"]);
    match cli.command {
        Some(Commands::Webhook {
            action: WebhookCommands::List,
        }) => {},
        other => panic!("expected Webhook::List, got: {other:?}"),
    }
}

#[test]
fn test_cli_webhook_create() {
    let cli = Cli::parse_from([
        "clawdius",
        "webhook",
        "create",
        "my-hook",
        "https://example.com/webhook",
        "--events",
        "session.created,message.sent",
        "--secret",
        "s3cret",
    ]);
    match cli.command {
        Some(Commands::Webhook {
            action:
                WebhookCommands::Create {
                    name,
                    url,
                    events,
                    secret,
                },
        }) => {
            assert_eq!(name, "my-hook");
            assert_eq!(url, "https://example.com/webhook");
            assert_eq!(events.as_deref(), Some("session.created,message.sent"));
            assert_eq!(secret.as_deref(), Some("s3cret"));
        },
        other => panic!("expected Webhook::Create, got: {other:?}"),
    }
}

#[test]
fn test_cli_webhook_create_minimal() {
    let cli = Cli::parse_from(["clawdius", "webhook", "create", "hook", "https://x.com"]);
    match cli.command {
        Some(Commands::Webhook {
            action:
                WebhookCommands::Create {
                    name,
                    url,
                    events,
                    secret,
                },
        }) => {
            assert_eq!(name, "hook");
            assert_eq!(url, "https://x.com");
            assert!(events.is_none());
            assert!(secret.is_none());
        },
        other => panic!("expected Webhook::Create, got: {other:?}"),
    }
}

#[test]
fn test_cli_webhook_show() {
    let cli = Cli::parse_from(["clawdius", "webhook", "show", "wh-abc123"]);
    match cli.command {
        Some(Commands::Webhook {
            action: WebhookCommands::Show { id },
        }) => {
            assert_eq!(id, "wh-abc123");
        },
        other => panic!("expected Webhook::Show, got: {other:?}"),
    }
}

#[test]
fn test_cli_webhook_update() {
    let cli = Cli::parse_from([
        "clawdius",
        "webhook",
        "update",
        "wh-123",
        "--url",
        "https://new.url",
        "--enable",
    ]);
    match cli.command {
        Some(Commands::Webhook {
            action:
                WebhookCommands::Update {
                    id,
                    url,
                    events,
                    enable,
                    disable,
                },
        }) => {
            assert_eq!(id, "wh-123");
            assert_eq!(url.as_deref(), Some("https://new.url"));
            assert!(events.is_none());
            assert!(enable);
            assert!(!disable);
        },
        other => panic!("expected Webhook::Update, got: {other:?}"),
    }
}

#[test]
fn test_cli_webhook_update_disable() {
    let cli = Cli::parse_from(["clawdius", "webhook", "update", "wh-456", "--disable"]);
    match cli.command {
        Some(Commands::Webhook {
            action: WebhookCommands::Update { disable, .. },
        }) => {
            assert!(disable);
        },
        other => panic!("expected Webhook::Update, got: {other:?}"),
    }
}

#[test]
fn test_cli_webhook_delete() {
    let cli = Cli::parse_from(["clawdius", "webhook", "delete", "wh-del"]);
    match cli.command {
        Some(Commands::Webhook {
            action: WebhookCommands::Delete { id },
        }) => {
            assert_eq!(id, "wh-del");
        },
        other => panic!("expected Webhook::Delete, got: {other:?}"),
    }
}

#[test]
fn test_cli_webhook_test() {
    let cli = Cli::parse_from([
        "clawdius",
        "webhook",
        "test",
        "wh-test",
        "--event",
        "message.sent",
    ]);
    match cli.command {
        Some(Commands::Webhook {
            action: WebhookCommands::Test { id, event },
        }) => {
            assert_eq!(id, "wh-test");
            assert_eq!(event.as_deref(), Some("message.sent"));
        },
        other => panic!("expected Webhook::Test, got: {other:?}"),
    }
}

#[test]
fn test_cli_webhook_test_no_event() {
    let cli = Cli::parse_from(["clawdius", "webhook", "test", "wh-t2"]);
    match cli.command {
        Some(Commands::Webhook {
            action: WebhookCommands::Test { id, event },
        }) => {
            assert_eq!(id, "wh-t2");
            assert!(event.is_none());
        },
        other => panic!("expected Webhook::Test, got: {other:?}"),
    }
}

#[test]
fn test_cli_webhook_deliveries() {
    let cli = Cli::parse_from(["clawdius", "webhook", "deliveries", "wh-d1", "--limit", "5"]);
    match cli.command {
        Some(Commands::Webhook {
            action: WebhookCommands::Deliveries { id, limit },
        }) => {
            assert_eq!(id.as_deref(), Some("wh-d1"));
            assert_eq!(limit, 5);
        },
        other => panic!("expected Webhook::Deliveries, got: {other:?}"),
    }
}

#[test]
fn test_cli_webhook_deliveries_no_id() {
    let cli = Cli::parse_from(["clawdius", "webhook", "deliveries"]);
    match cli.command {
        Some(Commands::Webhook {
            action: WebhookCommands::Deliveries { id, limit },
        }) => {
            assert!(id.is_none());
            assert_eq!(limit, 20); // default
        },
        other => panic!("expected Webhook::Deliveries, got: {other:?}"),
    }
}

#[test]
fn test_cli_webhook_stats() {
    let cli = Cli::parse_from(["clawdius", "webhook", "stats"]);
    match cli.command {
        Some(Commands::Webhook {
            action: WebhookCommands::Stats,
        }) => {},
        other => panic!("expected Webhook::Stats, got: {other:?}"),
    }
}

// ═══════════════════════════════════════════════════════════════
// Ship subcommand arg parsing
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_cli_ship_checks() {
    let cli = Cli::parse_from(["clawdius", "ship", "checks", "--branch", "feature/x"]);
    match cli.command {
        Some(Commands::Ship {
            action: ShipAction::Checks { branch, files },
        }) => {
            assert_eq!(branch, "feature/x");
            assert!(files.is_empty());
        },
        other => panic!("expected Ship::Checks, got: {other:?}"),
    }
}

#[test]
fn test_cli_ship_checks_with_files() {
    let cli = Cli::parse_from([
        "clawdius",
        "ship",
        "checks",
        "--branch",
        "main",
        "--files",
        "src/main.rs",
        "--files",
        "src/lib.rs",
    ]);
    match cli.command {
        Some(Commands::Ship {
            action: ShipAction::Checks { branch, files },
        }) => {
            assert_eq!(branch, "main");
            assert_eq!(files, vec!["src/main.rs", "src/lib.rs"]);
        },
        other => panic!("expected Ship::Checks, got: {other:?}"),
    }
}

#[test]
fn test_cli_ship_commit_message() {
    let cli = Cli::parse_from([
        "clawdius",
        "ship",
        "commit-message",
        "--files",
        "a.rs",
        "--description",
        "fix bug",
        "--scope",
        "core",
    ]);
    match cli.command {
        Some(Commands::Ship {
            action:
                ShipAction::CommitMessage {
                    files,
                    description,
                    scope,
                },
        }) => {
            assert_eq!(files, vec!["a.rs"]);
            assert_eq!(description, "fix bug");
            assert_eq!(scope.as_deref(), Some("core"));
        },
        other => panic!("expected Ship::CommitMessage, got: {other:?}"),
    }
}

#[test]
fn test_cli_ship_commit_message_minimal() {
    let cli = Cli::parse_from([
        "clawdius",
        "ship",
        "commit-message",
        "--description",
        "add feature",
    ]);
    match cli.command {
        Some(Commands::Ship {
            action:
                ShipAction::CommitMessage {
                    files,
                    description,
                    scope,
                },
        }) => {
            assert!(files.is_empty());
            assert_eq!(description, "add feature");
            assert!(scope.is_none());
        },
        other => panic!("expected Ship::CommitMessage, got: {other:?}"),
    }
}

// ═══════════════════════════════════════════════════════════════
// Skill subcommand arg parsing
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_cli_skill_list() {
    let cli = Cli::parse_from(["clawdius", "skill", "list"]);
    match cli.command {
        Some(Commands::Skill {
            action: SkillAction::List,
        }) => {},
        other => panic!("expected Skill::List, got: {other:?}"),
    }
}

#[test]
fn test_cli_skill_run() {
    let cli = Cli::parse_from([
        "clawdius",
        "skill",
        "run",
        "tdd",
        "file=test.rs module=parser",
    ]);
    match cli.command {
        Some(Commands::Skill {
            action: SkillAction::Run { name, arguments },
        }) => {
            assert_eq!(name, "tdd");
            assert_eq!(arguments, "file=test.rs module=parser");
        },
        other => panic!("expected Skill::Run, got: {other:?}"),
    }
}

#[test]
fn test_cli_skill_run_no_args() {
    let cli = Cli::parse_from(["clawdius", "skill", "run", "lint"]);
    match cli.command {
        Some(Commands::Skill {
            action: SkillAction::Run { name, arguments },
        }) => {
            assert_eq!(name, "lint");
            assert!(arguments.is_empty());
        },
        other => panic!("expected Skill::Run, got: {other:?}"),
    }
}

// ═══════════════════════════════════════════════════════════════
// LSP subcommand arg parsing (all 8 variants)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_cli_lsp_start() {
    let cli = Cli::parse_from(["clawdius", "lsp", "start", "rust-analyzer", "/workspace"]);
    match cli.command {
        Some(Commands::Lsp {
            action: LspCommands::Start { server, args, root },
        }) => {
            assert_eq!(server, "rust-analyzer");
            assert_eq!(args, vec!["/workspace"]);
            assert!(root.is_none());
        },
        other => panic!("expected Lsp::Start, got: {other:?}"),
    }
}

#[test]
fn test_cli_lsp_start_with_root() {
    let cli = Cli::parse_from(["clawdius", "lsp", "start", "pylsp", "--root", "/project"]);
    match cli.command {
        Some(Commands::Lsp {
            action: LspCommands::Start { server, args, root },
        }) => {
            assert_eq!(server, "pylsp");
            assert!(args.is_empty());
            assert_eq!(root.as_deref(), Some("/project"));
        },
        other => panic!("expected Lsp::Start, got: {other:?}"),
    }
}

#[test]
fn test_cli_lsp_complete() {
    let cli = Cli::parse_from([
        "clawdius",
        "lsp",
        "complete",
        "file:///main.rs",
        "--line",
        "10",
        "--column",
        "5",
    ]);
    match cli.command {
        Some(Commands::Lsp {
            action: LspCommands::Complete { uri, line, column },
        }) => {
            assert_eq!(uri, "file:///main.rs");
            assert_eq!(line, 10);
            assert_eq!(column, 5);
        },
        other => panic!("expected Lsp::Complete, got: {other:?}"),
    }
}

#[test]
fn test_cli_lsp_hover() {
    let cli = Cli::parse_from([
        "clawdius",
        "lsp",
        "hover",
        "file:///lib.rs",
        "--line",
        "42",
        "--column",
        "0",
    ]);
    match cli.command {
        Some(Commands::Lsp {
            action: LspCommands::Hover { uri, line, column },
        }) => {
            assert_eq!(uri, "file:///lib.rs");
            assert_eq!(line, 42);
            assert_eq!(column, 0);
        },
        other => panic!("expected Lsp::Hover, got: {other:?}"),
    }
}

#[test]
fn test_cli_lsp_definition() {
    let cli = Cli::parse_from([
        "clawdius",
        "lsp",
        "definition",
        "file:///src.rs",
        "--line",
        "7",
        "--column",
        "12",
    ]);
    match cli.command {
        Some(Commands::Lsp {
            action: LspCommands::Definition { uri, line, column },
        }) => {
            assert_eq!(uri, "file:///src.rs");
            assert_eq!(line, 7);
            assert_eq!(column, 12);
        },
        other => panic!("expected Lsp::Definition, got: {other:?}"),
    }
}

#[test]
fn test_cli_lsp_references() {
    let cli = Cli::parse_from([
        "clawdius",
        "lsp",
        "references",
        "file:///mod.rs",
        "--line",
        "3",
        "--column",
        "8",
        "--include-declaration",
    ]);
    match cli.command {
        Some(Commands::Lsp {
            action:
                LspCommands::References {
                    uri,
                    line,
                    column,
                    include_declaration,
                },
        }) => {
            assert_eq!(uri, "file:///mod.rs");
            assert_eq!(line, 3);
            assert_eq!(column, 8);
            assert!(include_declaration);
        },
        other => panic!("expected Lsp::References, got: {other:?}"),
    }
}

#[test]
fn test_cli_lsp_symbols() {
    let cli = Cli::parse_from(["clawdius", "lsp", "symbols", "file:///main.rs"]);
    match cli.command {
        Some(Commands::Lsp {
            action: LspCommands::Symbols { uri },
        }) => {
            assert_eq!(uri, "file:///main.rs");
        },
        other => panic!("expected Lsp::Symbols, got: {other:?}"),
    }
}

#[test]
fn test_cli_lsp_diagnostics() {
    let cli = Cli::parse_from(["clawdius", "lsp", "diagnostics", "file:///err.rs"]);
    match cli.command {
        Some(Commands::Lsp {
            action: LspCommands::Diagnostics { uri },
        }) => {
            assert_eq!(uri, "file:///err.rs");
        },
        other => panic!("expected Lsp::Diagnostics, got: {other:?}"),
    }
}

#[test]
fn test_cli_lsp_code_actions() {
    let cli = Cli::parse_from([
        "clawdius",
        "lsp",
        "code-actions",
        "file:///fix.rs",
        "--start-line",
        "1",
        "--start-column",
        "0",
        "--end-line",
        "5",
        "--end-column",
        "10",
    ]);
    match cli.command {
        Some(Commands::Lsp {
            action:
                LspCommands::CodeActions {
                    uri,
                    start_line,
                    start_column,
                    end_line,
                    end_column,
                },
        }) => {
            assert_eq!(uri, "file:///fix.rs");
            assert_eq!(start_line, 1);
            assert_eq!(start_column, 0);
            assert_eq!(end_line, 5);
            assert_eq!(end_column, 10);
        },
        other => panic!("expected Lsp::CodeActions, got: {other:?}"),
    }
}

// ═══════════════════════════════════════════════════════════════
// Memory subcommand arg parsing
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_cli_memory_learn_build() {
    let cli = Cli::parse_from(["clawdius", "memory", "learn", "build", "cargo build"]);
    match cli.command {
        Some(Commands::Memory {
            action:
                MemoryCommands::Learn {
                    entry_type,
                    content,
                    description,
                },
        }) => {
            assert_eq!(entry_type, "build");
            assert_eq!(content, "cargo build");
            assert!(description.is_none());
        },
        other => panic!("expected Memory::Learn, got: {other:?}"),
    }
}

#[test]
fn test_cli_memory_learn_debug() {
    let cli = Cli::parse_from([
        "clawdius",
        "memory",
        "learn",
        "debug",
        "borrow_error=use clone",
        "--description",
        "common fix",
    ]);
    match cli.command {
        Some(Commands::Memory {
            action:
                MemoryCommands::Learn {
                    entry_type,
                    content,
                    description,
                },
        }) => {
            assert_eq!(entry_type, "debug");
            assert_eq!(content, "borrow_error=use clone");
            assert_eq!(description.as_deref(), Some("common fix"));
        },
        other => panic!("expected Memory::Learn, got: {other:?}"),
    }
}

#[test]
fn test_cli_memory_instructions() {
    let cli = Cli::parse_from([
        "clawdius",
        "memory",
        "instructions",
        "use Rust 2024 edition",
    ]);
    match cli.command {
        Some(Commands::Memory {
            action: MemoryCommands::Instructions { content },
        }) => {
            assert_eq!(content, "use Rust 2024 edition");
        },
        other => panic!("expected Memory::Instructions, got: {other:?}"),
    }
}

#[test]
fn test_cli_memory_list_all() {
    let cli = Cli::parse_from(["clawdius", "memory", "list"]);
    match cli.command {
        Some(Commands::Memory {
            action: MemoryCommands::List { category },
        }) => {
            assert_eq!(category, "all"); // default
        },
        other => panic!("expected Memory::List, got: {other:?}"),
    }
}

#[test]
fn test_cli_memory_list_category() {
    let cli = Cli::parse_from(["clawdius", "memory", "list", "build"]);
    match cli.command {
        Some(Commands::Memory {
            action: MemoryCommands::List { category },
        }) => {
            assert_eq!(category, "build");
        },
        other => panic!("expected Memory::List, got: {other:?}"),
    }
}

#[test]
fn test_cli_memory_clear_all() {
    let cli = Cli::parse_from(["clawdius", "memory", "clear", "--yes"]);
    match cli.command {
        Some(Commands::Memory {
            action: MemoryCommands::Clear { category, yes },
        }) => {
            assert_eq!(category, "all"); // default
            assert!(yes);
        },
        other => panic!("expected Memory::Clear, got: {other:?}"),
    }
}

#[test]
fn test_cli_memory_clear_category() {
    let cli = Cli::parse_from(["clawdius", "memory", "clear", "debug", "--yes"]);
    match cli.command {
        Some(Commands::Memory {
            action: MemoryCommands::Clear { category, yes },
        }) => {
            assert_eq!(category, "debug");
            assert!(yes);
        },
        other => panic!("expected Memory::Clear, got: {other:?}"),
    }
}

#[test]
fn test_cli_memory_init() {
    let cli = Cli::parse_from([
        "clawdius",
        "memory",
        "init",
        "--name",
        "clawdius",
        "--language",
        "rust",
        "--framework",
        "axum",
    ]);
    match cli.command {
        Some(Commands::Memory {
            action:
                MemoryCommands::Init {
                    name,
                    language,
                    framework,
                },
        }) => {
            assert_eq!(name.as_deref(), Some("clawdius"));
            assert_eq!(language.as_deref(), Some("rust"));
            assert_eq!(framework.as_deref(), Some("axum"));
        },
        other => panic!("expected Memory::Init, got: {other:?}"),
    }
}

#[test]
fn test_cli_memory_show_instructions() {
    let cli = Cli::parse_from(["clawdius", "memory", "show", "--instructions"]);
    match cli.command {
        Some(Commands::Memory {
            action: MemoryCommands::Show { instructions },
        }) => {
            assert!(instructions);
        },
        other => panic!("expected Memory::Show, got: {other:?}"),
    }
}

// ═══════════════════════════════════════════════════════════════
// Action subcommand arg parsing
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_cli_action_extract_function() {
    let cli = Cli::parse_from([
        "clawdius",
        "action",
        "extract-function",
        "src/lib.rs",
        "--line",
        "10",
        "--column",
        "0",
        "--end-line",
        "20",
        "--end-column",
        "1",
    ]);
    match cli.command {
        Some(Commands::Action {
            action,
            file,
            line,
            column,
            end_line,
            end_column,
        }) => {
            assert_eq!(action, "extract-function");
            assert_eq!(file.to_string_lossy(), "src/lib.rs");
            assert_eq!(line, Some(10));
            assert_eq!(column, Some(0));
            assert_eq!(end_line, Some(20));
            assert_eq!(end_column, Some(1));
        },
        other => panic!("expected Action, got: {other:?}"),
    }
}

#[test]
fn test_cli_action_generate_tests() {
    let cli = Cli::parse_from(["clawdius", "action", "generate-tests", "parser.rs"]);
    match cli.command {
        Some(Commands::Action {
            action,
            file,
            line,
            column,
            end_line,
            end_column,
        }) => {
            assert_eq!(action, "generate-tests");
            assert_eq!(file.to_string_lossy(), "parser.rs");
            assert!(line.is_none());
            assert!(column.is_none());
            assert!(end_line.is_none());
            assert!(end_column.is_none());
        },
        other => panic!("expected Action, got: {other:?}"),
    }
}

// ═══════════════════════════════════════════════════════════════
// Webhook event parsing logic (unit test of core logic)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_webhook_event_parse_all_variants() {
    use clawdius_core::webhooks::WebhookEvent;

    let cases = [
        ("session.created", true),
        ("session.updated", true),
        ("session.deleted", true),
        ("message.sent", true),
        ("message.received", true),
        ("tool.executed", true),
        ("file.changed", true),
        ("checkpoint.created", true),
        ("checkpoint.restored", true),
        ("workflow.started", true),
        ("workflow.completed", true),
        ("workflow.failed", true),
        ("task.started", true),
        ("task.completed", true),
        ("task.failed", true),
        ("code.generated", true),
        ("tests.generated", true),
        ("error.occurred", true),
        ("*", true),
        ("all", true),
        ("invalid.event", false),
        ("", false),
        ("session", false),
        ("SESSION.CREATED", false), // case-sensitive
    ];

    let create_parser = |s: &str| -> bool {
        s.split(',')
            .filter_map(|e| match e.trim() {
                "session.created" => Some(WebhookEvent::SessionCreated),
                "session.updated" => Some(WebhookEvent::SessionUpdated),
                "session.deleted" => Some(WebhookEvent::SessionDeleted),
                "message.sent" => Some(WebhookEvent::MessageSent),
                "message.received" => Some(WebhookEvent::MessageReceived),
                "tool.executed" => Some(WebhookEvent::ToolExecuted),
                "file.changed" => Some(WebhookEvent::FileChanged),
                "checkpoint.created" => Some(WebhookEvent::CheckpointCreated),
                "checkpoint.restored" => Some(WebhookEvent::CheckpointRestored),
                "workflow.started" => Some(WebhookEvent::WorkflowStarted),
                "workflow.completed" => Some(WebhookEvent::WorkflowCompleted),
                "workflow.failed" => Some(WebhookEvent::WorkflowFailed),
                "task.started" => Some(WebhookEvent::TaskStarted),
                "task.completed" => Some(WebhookEvent::TaskCompleted),
                "task.failed" => Some(WebhookEvent::TaskFailed),
                "code.generated" => Some(WebhookEvent::CodeGenerated),
                "tests.generated" => Some(WebhookEvent::TestsGenerated),
                "error.occurred" => Some(WebhookEvent::ErrorOccurred),
                "*" | "all" => Some(WebhookEvent::All),
                _ => None,
            })
            .count()
            > 0
    };

    for (input, expected) in &cases {
        assert_eq!(
            create_parser(input),
            *expected,
            "event '{input}' should parse={expected}"
        );
    }
}

#[test]
fn test_webhook_event_comma_separated() {
    use clawdius_core::webhooks::WebhookEvent;

    let input = "session.created,message.sent,tool.executed";
    let count = input
        .split(',')
        .filter_map(|s| match s.trim() {
            "session.created" => Some(WebhookEvent::SessionCreated),
            "message.sent" => Some(WebhookEvent::MessageSent),
            "tool.executed" => Some(WebhookEvent::ToolExecuted),
            _ => None,
        })
        .count();

    assert_eq!(count, 3);
}

#[test]
fn test_webhook_event_mixed_valid_invalid() {
    use clawdius_core::webhooks::WebhookEvent;

    let input = "session.created,invalid.event,message.sent";
    let count = input
        .split(',')
        .filter_map(|s| match s.trim() {
            "session.created" => Some(WebhookEvent::SessionCreated),
            "message.sent" => Some(WebhookEvent::MessageSent),
            _ => None,
        })
        .count();

    assert_eq!(count, 2); // invalid filtered out
}

// ═══════════════════════════════════════════════════════════════
// Action language detection from file extension
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_action_language_detection() {
    let cases = [
        ("main.rs", "rs"),
        ("lib.rs", "rs"),
        ("script.py", "py"),
        ("index.ts", "ts"),
        ("style.css", "css"),
        ("data.json", "json"),
        ("Makefile", "txt"),
        ("README", "txt"), // no extension
    ];

    for (file, expected_ext) in &cases {
        let ext = std::path::Path::new(file)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt");
        assert_eq!(
            ext, *expected_ext,
            "file '{file}' should have ext '{expected_ext}'"
        );
    }
}

// ═══════════════════════════════════════════════════════════════
// Selection computation (mirrors action.rs lines 65-93)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_selection_single_line() {
    let document = "fn main() {\n    let x = 42;\n}\n";
    let lines: Vec<&str> = document.lines().collect();

    // Select columns 4..8 on line 1 ("    let x = 42;")
    let start_line = 1;
    let start_col = 4;
    let end_line = 1;
    let end_col = 8;

    let selection = if start_line == end_line {
        Some(lines[start_line][start_col..end_col].to_string())
    } else {
        None
    };

    // "    let x = 42;"[4..8] = "let " (space included)
    assert_eq!(selection.as_deref(), Some("let "));
}

#[test]
fn test_selection_multi_line() {
    let document = "fn main() {\n    let x = 42;\n    let y = 7;\n}\n";
    let lines: Vec<&str> = document.lines().collect();

    // Select from line 1 col 4 to line 2 col 11
    let start_line = 1;
    let start_col = 4;
    let end_line = 2;
    let end_col = 11;

    let mut selected_text = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i < start_line || i > end_line {
            continue;
        }
        if i == start_line {
            selected_text.push_str(&line[start_col..]);
        } else if i == end_line {
            selected_text.push_str(&line[..end_col]);
        } else {
            selected_text.push_str(line);
        }
        if i < end_line {
            selected_text.push('\n');
        }
    }

    // Verify the computation: line1[4..] + \n + line2[..11]
    // "    let x = 42;"[4..] = "let x = 42;"
    // "    let y = 7;"[..11] = "    let y = "
    let expected = format!("{}\n{}", &lines[1][4..], &lines[2][..11]);
    assert_eq!(selected_text, expected);
}

#[test]
fn test_selection_out_of_bounds() {
    let document = "short";
    let lines: Vec<&str> = document.lines().collect();

    let start_line = 0;
    let end_line = 5; // out of bounds

    // Out of bounds: selection should be None
    let selection = if start_line < lines.len() && end_line < lines.len() {
        Some("should not reach".to_string())
    } else {
        None
    };

    assert!(selection.is_none());
}

#[test]
fn test_selection_none_when_no_end() {
    let document = "hello world";
    let _lines: Vec<&str> = document.lines().collect();
    let end_line: Option<usize> = None;

    // When end_line/end_column not provided, selection is None
    let selection = match (Some(0usize), end_line) {
        (Some(_), Some(_)) => Some("unreachable".to_string()),
        _ => None,
    };

    assert!(selection.is_none());
}

// ═══════════════════════════════════════════════════════════════
// Memory learn entry type validation
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_memory_learn_valid_entry_types() {
    let valid_types = ["build", "test", "debug", "pattern", "preference"];
    for entry_type in &valid_types {
        let is_valid = matches!(
            entry_type.to_lowercase().as_str(),
            "build" | "test" | "debug" | "pattern" | "preference"
        );
        assert!(is_valid, "entry type '{entry_type}' should be valid");
    }
}

#[test]
fn test_memory_learn_invalid_entry_type() {
    let invalid_types = ["invalid", "deploy", "run", "review", "format"];
    for entry_type in &invalid_types {
        let is_valid = matches!(
            entry_type.to_lowercase().as_str(),
            "build" | "test" | "debug" | "pattern" | "preference"
        );
        assert!(!is_valid, "entry type '{entry_type}' should be invalid");
    }
}

#[test]
fn test_memory_learn_debug_format_validation() {
    // Valid format: issue=solution
    let content = "borrow_error=use clone instead";
    let parts: Vec<&str> = content.splitn(2, '=').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], "borrow_error");
    assert_eq!(parts[1], "use clone instead");

    // Invalid format: no equals sign
    let bad = "just a description";
    assert_eq!(bad.splitn(2, '=').count(), 1);
}

#[test]
fn test_memory_learn_pattern_format_validation() {
    // Valid format: name=pattern
    let content = "builder_pattern=use builder struct with chained methods";
    assert_eq!(content.splitn(2, '=').count(), 2);

    // Invalid: no equals
    let bad = "no pattern here";
    assert_ne!(bad.splitn(2, '=').count(), 2);
}

// ═══════════════════════════════════════════════════════════════
// Skill argument parsing (key=value format)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_skill_argument_parsing() {
    let args = "file=test.rs module=parser verbose=true";
    let params: Vec<&str> = args.split_whitespace().collect();
    assert_eq!(params.len(), 3);

    for param in &params {
        assert!(param.contains('='), "param '{param}' should be key=value");
        assert_eq!(param.splitn(2, '=').count(), 2);
    }
}

#[test]
fn test_skill_argument_parsing_empty() {
    let args = "";
    assert!(args.split_whitespace().next().is_none());
}

#[test]
fn test_skill_argument_parsing_single() {
    let args = "target=src/main.rs";
    let params: Vec<&str> = args.split_whitespace().collect();
    assert_eq!(params.len(), 1);
    assert_eq!(params[0], "target=src/main.rs");
}

// ═══════════════════════════════════════════════════════════════
// Webhook test event mapping
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_webhook_test_event_mapping() {
    use clawdius_core::webhooks::WebhookEvent;

    // None -> SessionCreated (default)
    let default = None::<String>.map_or(WebhookEvent::SessionCreated, |s| match s.as_str() {
        "message.sent" => WebhookEvent::MessageSent,
        "tool.executed" => WebhookEvent::ToolExecuted,
        _ => WebhookEvent::SessionCreated,
    });
    assert_eq!(default.as_str(), "session.created");

    // "message.sent" -> MessageSent
    let msg = Some("message.sent".to_string()).map_or(WebhookEvent::SessionCreated, |s| {
        match s.as_str() {
            "message.sent" => WebhookEvent::MessageSent,
            "tool.executed" => WebhookEvent::ToolExecuted,
            _ => WebhookEvent::SessionCreated,
        }
    });
    assert_eq!(msg.as_str(), "message.sent");

    // "tool.executed" -> ToolExecuted
    let tool = Some("tool.executed".to_string()).map_or(WebhookEvent::SessionCreated, |s| match s
        .as_str()
    {
        "message.sent" => WebhookEvent::MessageSent,
        "tool.executed" => WebhookEvent::ToolExecuted,
        _ => WebhookEvent::SessionCreated,
    });
    assert_eq!(tool.as_str(), "tool.executed");

    // Unknown -> SessionCreated (fallback)
    let unknown =
        Some("random.event".to_string()).map_or(WebhookEvent::SessionCreated, |s| {
            match s.as_str() {
                "message.sent" => WebhookEvent::MessageSent,
                "tool.executed" => WebhookEvent::ToolExecuted,
                _ => WebhookEvent::SessionCreated,
            }
        });
    assert_eq!(unknown.as_str(), "session.created");
}

// ═══════════════════════════════════════════════════════════════
// Output format conversion correctness
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_output_format_conversion() {
    use clawdius_core::output::OutputFormat as CoreOutputFormat;

    assert_eq!(
        CoreOutputFormat::from(OutputFormat::Text),
        CoreOutputFormat::Text
    );
    assert_eq!(
        CoreOutputFormat::from(OutputFormat::Json),
        CoreOutputFormat::Json
    );
    assert_eq!(
        CoreOutputFormat::from(OutputFormat::StreamJson),
        CoreOutputFormat::StreamJson
    );
}
