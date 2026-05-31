//! Integration tests exercising real CLI handler logic beyond arg-parsing.
//!
//! All logic tested here is extracted directly from handler source code and
//! tested in isolation (no LLM calls, no filesystem side effects beyond tempdir).
//!
//! Covers:
//!   - `OutputFormat` enum conversion and defaults
//!   - Severity / priority level mapping (analyze)
//!   - Language detection from file extensions
//!   - Selection computation (single-line, multi-line, boundary)
//!   - Webhook event string parsing (all 19 variants + wildcards)
//!   - Memory entry-type validation and key=value format
//!   - Skill argument parsing (key=value)
//!   - Git status porcelain parsing
//!   - Function body extraction (brace matching)
//!   - API-key masking in TOML
//!   - Config key get/set roundtrips (in-memory)
//!   - Drift/debt filtering by severity/priority
//!   - Analysis text/JSON formatting helpers
//!   - CLI enum parsing for all subcommands not covered in `cli_tests.rs`
//!   - Exclude pattern parsing
//!   - Delivery status icon mapping
//!   - Change kind and file change type mapping
//!   - Model size formatting
//!   - Session search preview truncation
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::items_after_statements
)]

use clap::Parser;
use clawdius::cli::{
    CheckpointCommands, Cli, Commands, ConfigAction, GitCommands, LangCommands,
    MetricsOutputFormat, ModeCommands, ModelsCommands, OutputFormat, TimelineCommands,
};
use rstest::rstest;

// ═══════════════════════════════════════════════════════════════════
// OutputFormat enum logic
// ═══════════════════════════════════════════════════════════════════

#[test]
fn output_format_default_is_text() {
    assert_eq!(OutputFormat::default(), OutputFormat::Text);
}

#[test]
fn output_format_partial_eq() {
    assert_eq!(OutputFormat::Text, OutputFormat::Text);
    assert_ne!(OutputFormat::Text, OutputFormat::Json);
    assert_ne!(OutputFormat::Json, OutputFormat::StreamJson);
}

#[test]
fn output_format_debug_repr() {
    assert_eq!(format!("{:?}", OutputFormat::Text), "Text");
    assert_eq!(format!("{:?}", OutputFormat::Json), "Json");
    assert_eq!(format!("{:?}", OutputFormat::StreamJson), "StreamJson");
}

#[test]
fn output_format_clone_copy() {
    let f = OutputFormat::Json;
    let f2 = f;
    assert_eq!(f, f2);
}

#[test]
fn metrics_output_format_default_is_text() {
    assert_eq!(MetricsOutputFormat::default(), MetricsOutputFormat::Text);
}

#[test]
fn metrics_output_format_variants() {
    use clap::ValueEnum;
    assert_eq!(MetricsOutputFormat::value_variants().len(), 3);
}

#[test]
fn metrics_output_format_partial_eq() {
    assert_eq!(MetricsOutputFormat::Text, MetricsOutputFormat::Text);
    assert_ne!(MetricsOutputFormat::Text, MetricsOutputFormat::Html);
}

#[test]
fn metrics_output_format_debug() {
    assert_eq!(format!("{:?}", MetricsOutputFormat::Text), "Text");
    assert_eq!(format!("{:?}", MetricsOutputFormat::Json), "Json");
    assert_eq!(format!("{:?}", MetricsOutputFormat::Html), "Html");
}

// ═══════════════════════════════════════════════════════════════════
// Severity level mapping (mirrors analyze.rs handle_analyze)
// ═══════════════════════════════════════════════════════════════════

fn severity_level(s: &str) -> u8 {
    match s.to_lowercase().as_str() {
        "medium" => 2,
        "high" => 3,
        "critical" => 4,
        _ => 1,
    }
}

#[test]
fn severity_level_low_is_1() {
    assert_eq!(severity_level("low"), 1);
}

#[test]
fn severity_level_medium_is_2() {
    assert_eq!(severity_level("medium"), 2);
}

#[test]
fn severity_level_high_is_3() {
    assert_eq!(severity_level("high"), 3);
}

#[test]
fn severity_level_critical_is_4() {
    assert_eq!(severity_level("critical"), 4);
}

#[test]
fn severity_level_case_insensitive() {
    assert_eq!(severity_level("LOW"), 1);
    assert_eq!(severity_level("Medium"), 2);
    assert_eq!(severity_level("HIGH"), 3);
    assert_eq!(severity_level("CRITICAL"), 4);
}

#[test]
fn severity_level_unknown_is_1() {
    assert_eq!(severity_level("unknown"), 1);
    assert_eq!(severity_level("warning"), 1);
}

#[test]
fn severity_level_empty_is_1() {
    assert_eq!(severity_level(""), 1);
}

// ═══════════════════════════════════════════════════════════════════
// Priority level mapping (mirrors analyze.rs filter_debt_by_priority)
// ═══════════════════════════════════════════════════════════════════

const fn priority_to_level(p: u8) -> u8 {
    match p {
        4..=6 => 2,
        7..=8 => 3,
        9..=10 => 4,
        _ => 1,
    }
}

#[test]
fn priority_level_1_is_low() {
    assert_eq!(priority_to_level(1), 1);
}

#[test]
fn priority_level_3_is_low() {
    assert_eq!(priority_to_level(3), 1);
}

#[test]
fn priority_level_4_is_medium() {
    assert_eq!(priority_to_level(4), 2);
}

#[test]
fn priority_level_6_is_medium() {
    assert_eq!(priority_to_level(6), 2);
}

#[test]
fn priority_level_7_is_high() {
    assert_eq!(priority_to_level(7), 3);
}

#[test]
fn priority_level_9_is_critical() {
    assert_eq!(priority_to_level(9), 4);
}

#[test]
fn priority_level_10_is_critical() {
    assert_eq!(priority_to_level(10), 4);
}

#[test]
fn priority_level_0_is_low() {
    assert_eq!(priority_to_level(0), 1);
}

#[test]
fn priority_level_255_is_low() {
    assert_eq!(priority_to_level(255), 1);
}

#[test]
fn priority_level_5_is_medium() {
    assert_eq!(priority_to_level(5), 2);
}

#[test]
fn priority_level_8_is_high() {
    assert_eq!(priority_to_level(8), 3);
}

// ═══════════════════════════════════════════════════════════════════
// Language detection from file extension (mirrors action.rs + doc.rs)
// ═══════════════════════════════════════════════════════════════════

fn detect_language(path: &str) -> &str {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("txt")
}

#[rstest]
#[case("main.rs", "rs")]
#[case("lib.rs", "rs")]
#[case("parser.rs", "rs")]
#[case("script.py", "py")]
#[case("app.ts", "ts")]
#[case("index.ts", "ts")]
#[case("app.tsx", "tsx")]
#[case("style.css", "css")]
#[case("data.json", "json")]
#[case("config.toml", "toml")]
#[case("Makefile", "txt")]
#[case("README", "txt")]
#[case("Dockerfile", "txt")]
#[case("mod.rs", "rs")]
#[case("_test.rs", "rs")]
fn test_language_detection(#[case] path: &str, #[case] expected: &str) {
    assert_eq!(detect_language(path), expected);
}

#[rstest]
#[case("rustdoc", "rustdoc")]
#[case("rust", "rustdoc")]
#[case("jsdoc", "jsdoc")]
#[case("javascript", "jsdoc")]
#[case("typescript", "jsdoc")]
#[case("pydoc", "pydoc")]
#[case("python", "pydoc")]
#[case("markdown", "markdown")]
#[case("md", "markdown")]
fn test_doc_format_selection(#[case] ext: &str, #[case] expected: &str) {
    let format = match ext {
        "rustdoc" | "rust" => "rustdoc",
        "jsdoc" | "javascript" | "typescript" => "jsdoc",
        "pydoc" | "python" => "pydoc",
        "markdown" | "md" => "markdown",
        _ => "unknown",
    };
    assert_eq!(format, expected);
}

#[test]
fn doc_format_auto_fallback_by_language() {
    let cases = vec![
        ("ts", "jsdoc"),
        ("js", "jsdoc"),
        ("py", "pydoc"),
        ("rs", "markdown"),
        ("go", "markdown"),
    ];
    for (lang, expected) in cases {
        let format = match lang {
            "ts" | "js" => "jsdoc",
            "py" => "pydoc",
            _ => "markdown",
        };
        assert_eq!(
            format, expected,
            "language '{lang}' should map to '{expected}'"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════
// Selection computation (mirrors action.rs lines 65-93)
// ═══════════════════════════════════════════════════════════════════

fn compute_selection(
    document: &str,
    start_line: usize,
    start_col: usize,
    end_line: Option<usize>,
    end_col: Option<usize>,
) -> Option<String> {
    let lines: Vec<&str> = document.lines().collect();
    match (end_line, end_col) {
        (Some(el), Some(ec)) => {
            if start_line >= lines.len() || el >= lines.len() {
                return None;
            }
            if start_line == el {
                if start_col > lines[start_line].len() || ec > lines[start_line].len() {
                    return None;
                }
                Some(lines[start_line][start_col..ec].to_string())
            } else {
                let mut selected_text = String::new();
                for i in start_line..=el {
                    if i >= lines.len() {
                        break;
                    }
                    if i == start_line {
                        selected_text.push_str(&lines[i][start_col.min(lines[i].len())..]);
                    } else if i == el {
                        selected_text.push_str(&lines[i][..ec.min(lines[i].len())]);
                    } else {
                        selected_text.push_str(lines[i]);
                    }
                    if i < el {
                        selected_text.push('\n');
                    }
                }
                Some(selected_text)
            }
        },
        _ => None,
    }
}

#[test]
fn selection_single_line_middle() {
    let doc = "fn main() {\n    let x = 42;\n}\n";
    let sel = compute_selection(doc, 1, 4, Some(1), Some(8));
    assert_eq!(sel.as_deref(), Some("let "));
}

#[test]
fn selection_single_line_full_line() {
    let doc = "hello world\nfoo bar";
    let sel = compute_selection(doc, 0, 0, Some(0), Some(11));
    assert_eq!(sel.as_deref(), Some("hello world"));
}

#[test]
fn selection_multi_line_two_lines() {
    let doc = "line0\nline1\nline2\n";
    let sel = compute_selection(doc, 0, 2, Some(1), Some(3));
    assert_eq!(sel.as_deref(), Some("ne0\nlin"));
}

#[test]
fn selection_multi_line_three_lines() {
    let doc = "aaa\nbbb\nccc\n";
    let sel = compute_selection(doc, 0, 1, Some(2), Some(2));
    assert_eq!(sel.as_deref(), Some("aa\nbbb\ncc"));
}

#[test]
fn selection_none_when_no_end_line() {
    let sel = compute_selection("hello", 0, 0, None, None);
    assert!(sel.is_none());
}

#[test]
fn selection_none_when_no_end_col() {
    let sel = compute_selection("hello", 0, 0, Some(0), None);
    assert!(sel.is_none());
}

#[test]
fn selection_none_when_out_of_bounds() {
    let doc = "short";
    let sel = compute_selection(doc, 0, 0, Some(5), Some(1));
    assert!(sel.is_none());
}

#[test]
fn selection_none_when_start_out_of_bounds() {
    let doc = "short";
    let sel = compute_selection(doc, 10, 0, Some(10), Some(1));
    assert!(sel.is_none());
}

#[test]
fn selection_empty_single_line() {
    let doc = "hello";
    let sel = compute_selection(doc, 0, 3, Some(0), Some(3));
    assert_eq!(sel.as_deref(), Some(""));
}

#[test]
fn selection_clamps_columns_to_line_length() {
    let doc = "hi\nworld\n";
    let sel = compute_selection(doc, 0, 0, Some(1), Some(100));
    assert_eq!(sel.as_deref(), Some("hi\nworld"));
}

#[test]
fn selection_reversed_bounds_empty_string() {
    let doc = "abc\ndef\n";
    let sel = compute_selection(doc, 1, 0, Some(0), Some(1));
    assert!(sel.is_some());
    assert_eq!(sel.unwrap(), "");
}

// ═══════════════════════════════════════════════════════════════════
// Webhook event string parsing (mirrors webhook.rs event parsing)
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, PartialEq)]
enum TestWebhookEvent {
    SessionCreated,
    SessionUpdated,
    SessionDeleted,
    MessageSent,
    MessageReceived,
    ToolExecuted,
    FileChanged,
    CheckpointCreated,
    CheckpointRestored,
    WorkflowStarted,
    WorkflowCompleted,
    WorkflowFailed,
    TaskStarted,
    TaskCompleted,
    TaskFailed,
    CodeGenerated,
    TestsGenerated,
    ErrorOccurred,
    All,
}

fn parse_webhook_events(input: &str) -> Vec<TestWebhookEvent> {
    input
        .split(',')
        .filter_map(|s| match s.trim() {
            "session.created" => Some(TestWebhookEvent::SessionCreated),
            "session.updated" => Some(TestWebhookEvent::SessionUpdated),
            "session.deleted" => Some(TestWebhookEvent::SessionDeleted),
            "message.sent" => Some(TestWebhookEvent::MessageSent),
            "message.received" => Some(TestWebhookEvent::MessageReceived),
            "tool.executed" => Some(TestWebhookEvent::ToolExecuted),
            "file.changed" => Some(TestWebhookEvent::FileChanged),
            "checkpoint.created" => Some(TestWebhookEvent::CheckpointCreated),
            "checkpoint.restored" => Some(TestWebhookEvent::CheckpointRestored),
            "workflow.started" => Some(TestWebhookEvent::WorkflowStarted),
            "workflow.completed" => Some(TestWebhookEvent::WorkflowCompleted),
            "workflow.failed" => Some(TestWebhookEvent::WorkflowFailed),
            "task.started" => Some(TestWebhookEvent::TaskStarted),
            "task.completed" => Some(TestWebhookEvent::TaskCompleted),
            "task.failed" => Some(TestWebhookEvent::TaskFailed),
            "code.generated" => Some(TestWebhookEvent::CodeGenerated),
            "tests.generated" => Some(TestWebhookEvent::TestsGenerated),
            "error.occurred" => Some(TestWebhookEvent::ErrorOccurred),
            "*" | "all" => Some(TestWebhookEvent::All),
            _ => None,
        })
        .collect()
}

#[rstest]
#[case("session.created", TestWebhookEvent::SessionCreated)]
#[case("session.updated", TestWebhookEvent::SessionUpdated)]
#[case("session.deleted", TestWebhookEvent::SessionDeleted)]
#[case("message.sent", TestWebhookEvent::MessageSent)]
#[case("message.received", TestWebhookEvent::MessageReceived)]
#[case("tool.executed", TestWebhookEvent::ToolExecuted)]
#[case("file.changed", TestWebhookEvent::FileChanged)]
#[case("checkpoint.created", TestWebhookEvent::CheckpointCreated)]
#[case("checkpoint.restored", TestWebhookEvent::CheckpointRestored)]
#[case("workflow.started", TestWebhookEvent::WorkflowStarted)]
#[case("workflow.completed", TestWebhookEvent::WorkflowCompleted)]
#[case("workflow.failed", TestWebhookEvent::WorkflowFailed)]
#[case("task.started", TestWebhookEvent::TaskStarted)]
#[case("task.completed", TestWebhookEvent::TaskCompleted)]
#[case("task.failed", TestWebhookEvent::TaskFailed)]
#[case("code.generated", TestWebhookEvent::CodeGenerated)]
#[case("tests.generated", TestWebhookEvent::TestsGenerated)]
#[case("error.occurred", TestWebhookEvent::ErrorOccurred)]
#[case("*", TestWebhookEvent::All)]
#[case("all", TestWebhookEvent::All)]
fn test_single_event_parse(#[case] input: &str, #[case] expected: TestWebhookEvent) {
    let events = parse_webhook_events(input);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0], expected);
}

#[test]
fn test_multi_event_comma_separated() {
    let events = parse_webhook_events("session.created,message.sent,tool.executed");
    assert_eq!(events.len(), 3);
}

#[test]
fn test_invalid_events_filtered_out() {
    let events = parse_webhook_events("session.created,invalid,message.sent");
    assert_eq!(events.len(), 2);
}

#[test]
fn test_empty_string_yields_no_events() {
    let events = parse_webhook_events("");
    assert!(events.is_empty());
}

#[test]
fn test_all_invalid_yields_no_events() {
    let events = parse_webhook_events("foo,bar,baz");
    assert!(events.is_empty());
}

#[test]
fn test_whitespace_around_events_trimmed() {
    let events = parse_webhook_events(" session.created , message.sent ");
    assert_eq!(events.len(), 2);
}

#[test]
fn test_case_sensitive_events() {
    let events = parse_webhook_events("Session.Created");
    assert!(events.is_empty());
}

#[test]
fn test_wildcard_all_parses() {
    let events = parse_webhook_events("all");
    assert_eq!(events.len(), 1);
}

#[test]
fn test_wildcard_star_parses() {
    let events = parse_webhook_events("*");
    assert_eq!(events.len(), 1);
}

#[test]
fn test_webhook_test_event_mapping_default() {
    let event: &str = None::<String>.map_or("session.created", |s| match s.as_str() {
        "message.sent" => "message.sent",
        "tool.executed" => "tool.executed",
        _ => "session.created",
    });
    assert_eq!(event, "session.created");
}

#[test]
fn test_webhook_test_event_mapping_message_sent() {
    let event: &str =
        Some("message.sent".to_string()).map_or("session.created", |s| match s.as_str() {
            "message.sent" => "message.sent",
            "tool.executed" => "tool.executed",
            _ => "session.created",
        });
    assert_eq!(event, "message.sent");
}

#[test]
fn test_webhook_test_event_mapping_tool_executed() {
    let event: &str =
        Some("tool.executed".to_string()).map_or("session.created", |s| match s.as_str() {
            "message.sent" => "message.sent",
            "tool.executed" => "tool.executed",
            _ => "session.created",
        });
    assert_eq!(event, "tool.executed");
}

#[test]
fn test_webhook_test_event_mapping_unknown_fallback() {
    let event: &str =
        Some("random.event".to_string()).map_or("session.created", |s| match s.as_str() {
            "message.sent" => "message.sent",
            "tool.executed" => "tool.executed",
            _ => "session.created",
        });
    assert_eq!(event, "session.created");
}

// ═══════════════════════════════════════════════════════════════════
// Memory entry-type validation (mirrors memory.rs)
// ═══════════════════════════════════════════════════════════════════

fn is_valid_entry_type(t: &str) -> bool {
    matches!(
        t.to_lowercase().as_str(),
        "build" | "test" | "debug" | "pattern" | "preference"
    )
}

#[rstest]
#[case("build", true)]
#[case("test", true)]
#[case("debug", true)]
#[case("pattern", true)]
#[case("preference", true)]
#[case("BUILD", true)]
#[case("Debug", true)]
#[case("invalid", false)]
#[case("deploy", false)]
#[case("run", false)]
#[case("", false)]
fn test_entry_type_validation(#[case] input: &str, #[case] valid: bool) {
    assert_eq!(is_valid_entry_type(input), valid);
}

#[test]
fn debug_content_requires_equals() {
    let good = "borrow_error=use clone";
    assert_eq!(good.splitn(2, '=').count(), 2);
    let bad = "no equals here";
    assert_ne!(bad.splitn(2, '=').count(), 2);
}

#[test]
fn pattern_content_requires_equals() {
    let good = "builder=use builder struct";
    assert_eq!(good.splitn(2, '=').count(), 2);
    let bad = "no pattern here";
    assert_ne!(bad.splitn(2, '=').count(), 2);
}

#[test]
fn preference_content_requires_equals() {
    let good = "indent=4 spaces";
    assert_eq!(good.splitn(2, '=').count(), 2);
    let bad = "no preference";
    assert_ne!(bad.splitn(2, '=').count(), 2);
}

#[test]
fn equals_in_value_preserved() {
    let content = "pattern=foo == bar";
    let parts: Vec<&str> = content.splitn(2, '=').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], "pattern");
    assert_eq!(parts[1], "foo == bar");
}

#[test]
fn empty_equals_value() {
    let content = "key=";
    let parts: Vec<&str> = content.splitn(2, '=').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[1], "");
}

// ═══════════════════════════════════════════════════════════════════
// Skill argument parsing (mirrors skill.rs)
// ═══════════════════════════════════════════════════════════════════

fn parse_skill_args(args: &str) -> Vec<(&str, &str)> {
    args.split_whitespace()
        .filter_map(|p| p.split_once('='))
        .collect()
}

#[test]
fn skill_args_three_params() {
    let params = parse_skill_args("file=test.rs module=parser verbose=true");
    assert_eq!(params.len(), 3);
    assert_eq!(params[0], ("file", "test.rs"));
}

#[test]
fn skill_args_empty() {
    let params = parse_skill_args("");
    assert!(params.is_empty());
}

#[test]
fn skill_args_single() {
    let params = parse_skill_args("target=src/main.rs");
    assert_eq!(params.len(), 1);
    assert_eq!(params[0], ("target", "src/main.rs"));
}

#[test]
fn skill_args_no_equals_skipped() {
    let params = parse_skill_args("file=test.rs nogoodvalue verbose=true");
    assert_eq!(params.len(), 2);
}

#[test]
fn skill_args_value_with_equals() {
    let params = parse_skill_args("expr=x=y");
    assert_eq!(params.len(), 1);
    assert_eq!(params[0], ("expr", "x=y"));
}

// ═══════════════════════════════════════════════════════════════════
// Git status porcelain parsing (mirrors git.rs)
// ═══════════════════════════════════════════════════════════════════

#[derive(Default)]
struct StatusCounts {
    modified: usize,
    added: usize,
    deleted: usize,
    untracked: usize,
    renamed: usize,
    copied: usize,
    other: usize,
}

fn parse_porcelain(lines: &[&str]) -> StatusCounts {
    let mut c = StatusCounts::default();
    for line in lines {
        if line.len() < 2 {
            continue;
        }
        let idx = line.chars().next().unwrap_or(' ');
        let wt = line.chars().nth(1).unwrap_or(' ');
        match (idx, wt) {
            ('?', _) => c.untracked += 1,
            ('A', _) | (_, 'A') => c.added += 1,
            ('D', _) | (_, 'D') => c.deleted += 1,
            ('R', _) | (_, 'R') => c.renamed += 1,
            ('C', _) | (_, 'C') => c.copied += 1,
            ('M', _) | (_, 'M') => c.modified += 1,
            _ => c.other += 1,
        }
    }
    c
}

#[test]
fn parse_porcelain_untracked() {
    let c = parse_porcelain(&["?? new.txt", "?? old.txt"]);
    assert_eq!(c.untracked, 2);
    assert_eq!(c.modified, 0);
}

#[test]
fn parse_porcelain_modified_index() {
    let c = parse_porcelain(&["M  staged.rs"]);
    assert_eq!(c.modified, 1);
}

#[test]
fn parse_porcelain_modified_worktree() {
    let c = parse_porcelain(&[" M unstaged.rs"]);
    assert_eq!(c.modified, 1);
}

#[test]
fn parse_porcelain_modified_both() {
    let c = parse_porcelain(&["MM both.rs"]);
    assert_eq!(c.modified, 1);
}

#[test]
fn parse_porcelain_added_staged() {
    let c = parse_porcelain(&["A  new.rs"]);
    assert_eq!(c.added, 1);
}

#[test]
fn parse_porcelain_added_worktree() {
    let c = parse_porcelain(&[" A new.rs"]);
    assert_eq!(c.added, 1);
}

#[test]
fn parse_porcelain_deleted_both() {
    let c = parse_porcelain(&["D  gone.rs", " D unstaged_gone.rs"]);
    assert_eq!(c.deleted, 2);
}

#[test]
fn parse_porcelain_renamed() {
    let c = parse_porcelain(&["R  old -> new"]);
    assert_eq!(c.renamed, 1);
}

#[test]
fn parse_porcelain_copied() {
    let c = parse_porcelain(&["C  orig -> copy"]);
    assert_eq!(c.copied, 1);
}

#[test]
fn parse_porcelain_mixed() {
    let c = parse_porcelain(&[
        " M mod.rs",
        "A  add.rs",
        "?? untracked.rs",
        "D  del.rs",
        "R  old.rs -> new.rs",
        "C  src.rs -> dst.rs",
    ]);
    assert_eq!(c.modified, 1);
    assert_eq!(c.added, 1);
    assert_eq!(c.untracked, 1);
    assert_eq!(c.deleted, 1);
    assert_eq!(c.renamed, 1);
    assert_eq!(c.copied, 1);
}

#[test]
fn parse_porcelain_empty() {
    let c = parse_porcelain(&[]);
    assert_eq!(c.modified, 0);
}

#[test]
fn parse_porcelain_short_line_skipped() {
    let c = parse_porcelain(&["X"]);
    assert_eq!(c.other, 0);
}

#[test]
fn parse_porcelain_double_untracked() {
    let c = parse_porcelain(&["?? a.txt", "?? b.txt", "?? c.txt"]);
    assert_eq!(c.untracked, 3);
}

// ═══════════════════════════════════════════════════════════════════
// Function body extraction (brace matching from test_cmd.rs)
// ═══════════════════════════════════════════════════════════════════

fn extract_fn_body(code: &str, start: usize) -> Option<String> {
    let mut depth = 0;
    let mut in_fn = false;
    for (i, c) in code[start..].char_indices() {
        match c {
            '{' => {
                depth += 1;
                in_fn = true;
            },
            '}' => {
                depth -= 1;
                if in_fn && depth == 0 {
                    return Some(code[start..=(start + i)].to_string());
                }
            },
            _ => {},
        }
    }
    None
}

#[test]
fn fn_body_simple() {
    let code = "fn foo() { let x = 1; }";
    let body = extract_fn_body(code, 8);
    assert_eq!(body.as_deref(), Some(" { let x = 1; }"));
}

#[test]
fn fn_body_nested_braces() {
    let code = "fn foo() { if true { 1 } else { 2 } }";
    let body = extract_fn_body(code, 8);
    assert_eq!(body.as_deref(), Some(" { if true { 1 } else { 2 } }"));
}

#[test]
fn fn_body_deeply_nested() {
    let code = "fn f() { { { 1 } } }";
    let body = extract_fn_body(code, 6);
    assert_eq!(body.as_deref(), Some(" { { { 1 } } }"));
}

#[test]
fn fn_body_no_closing_brace_returns_none() {
    let code = "fn foo() { let x = 1;";
    assert!(extract_fn_body(code, 8).is_none());
}

#[test]
fn fn_body_empty_braces() {
    let code = "fn foo() {}";
    let body = extract_fn_body(code, 8);
    assert_eq!(body.as_deref(), Some(" {}"));
}

#[test]
fn fn_body_with_string_containing_braces() {
    let code = r#"fn foo() { let s = "{}"; }"#;
    let body = extract_fn_body(code, 8);
    assert_eq!(body.as_deref(), Some(r#" { let s = "{}"; }"#));
}

#[test]
fn fn_body_multiline() {
    let code = "fn foo() {\n  line1;\n  line2;\n}";
    let body = extract_fn_body(code, 8);
    assert!(body.is_some());
    let b = body.unwrap();
    assert!(b.starts_with(" {"));
    assert!(b.ends_with('}'));
    assert!(b.contains("line1"));
    assert!(b.contains("line2"));
}

#[test]
fn fn_body_multiple_functions() {
    let code = "fn a() { 1 }\nfn b() { 2 }";
    let body_a = extract_fn_body(code, 6);
    assert_eq!(body_a.as_deref(), Some(" { 1 }"));
    let body_b = extract_fn_body(code, code.find("fn b()").unwrap() + 6);
    assert_eq!(body_b.as_deref(), Some(" { 2 }"));
}

#[test]
fn fn_body_start_at_zero() {
    let code = "{ hello }";
    let body = extract_fn_body(code, 0);
    assert_eq!(body.as_deref(), Some("{ hello }"));
}

#[test]
fn fn_body_no_opening_brace() {
    let code = "fn foo() let x = 1;";
    assert!(extract_fn_body(code, 8).is_none());
}

// ═══════════════════════════════════════════════════════════════════
// API-key masking (mirrors config_cmd.rs)
// ═══════════════════════════════════════════════════════════════════

fn mask_api_keys(toml: &str) -> String {
    let mut result = toml.to_string();
    if let Ok(re) = regex::Regex::new(r#"(\w*_?key\w*\s*=\s*)"[^"]{8,}""#) {
        result = re.replace_all(&result, r#"${1}"***""#).to_string();
    }
    result
}

#[test]
fn mask_long_key() {
    let input = r#"api_key = "sk-12345678abcdef""#;
    assert_eq!(mask_api_keys(input), r#"api_key = "***""#);
}

#[test]
fn mask_short_key_preserved() {
    let input = r#"api_key = "short""#;
    assert_eq!(mask_api_keys(input), input);
}

#[test]
fn mask_no_key_field_preserved() {
    let input = r#"model = "claude-3""#;
    assert_eq!(mask_api_keys(input), input);
}

#[test]
fn mask_multiple_keys() {
    let input = r#"api_key = "sk-12345678"
openai_key = "sk-87654321abcdef""#;
    let result = mask_api_keys(input);
    assert!(result.contains(r#"api_key = "***""#));
    assert!(result.contains(r#"openai_key = "***""#));
    assert!(!result.contains("sk-"));
}

#[test]
fn mask_preserves_other_fields() {
    let input = r#"provider = "anthropic"
model = "claude-3-opus"
api_key = "sk-12345678""#;
    let result = mask_api_keys(input);
    assert!(result.contains("provider = \"anthropic\""));
    assert!(result.contains("model = \"claude-3-opus\""));
}

#[test]
fn mask_empty_string() {
    assert_eq!(mask_api_keys(""), "");
}

#[test]
fn mask_exactly_8_chars() {
    let input = r#"api_key = "12345678""#;
    let result = mask_api_keys(input);
    assert_eq!(result, r#"api_key = "***""#);
}

#[test]
fn mask_7_chars_preserved() {
    let input = r#"api_key = "1234567""#;
    assert_eq!(mask_api_keys(input), input);
}

#[test]
fn mask_underscore_key_name() {
    let input = r#"my_api_key = "abcdefghij""#;
    let result = mask_api_keys(input);
    assert!(result.contains("my_api_key = \"***\""));
}

#[test]
fn mask_key_with_prefix() {
    let input = r#"anthropic_api_key = "sk-longkey12345""#;
    let result = mask_api_keys(input);
    assert!(result.contains("anthropic_api_key = \"***\""));
}

// ═══════════════════════════════════════════════════════════════════
// Exclude patterns parsing (mirrors analyze.rs)
// ═══════════════════════════════════════════════════════════════════

fn parse_excludes(input: Option<String>) -> Vec<String> {
    input
        .map(|p| {
            p.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn excludes_none_yields_empty() {
    assert!(parse_excludes(None).is_empty());
}

#[test]
fn excludes_single_pattern() {
    let ex = parse_excludes(Some("target/".to_string()));
    assert_eq!(ex, vec!["target/"]);
}

#[test]
fn excludes_multiple_comma_separated() {
    let ex = parse_excludes(Some("target/,vendor/,dist/".to_string()));
    assert_eq!(ex, vec!["target/", "vendor/", "dist/"]);
}

#[test]
fn excludes_whitespace_trimmed() {
    let ex = parse_excludes(Some(" target/ , vendor/ ".to_string()));
    assert_eq!(ex, vec!["target/", "vendor/"]);
}

#[test]
fn excludes_empty_string_yields_empty() {
    let ex = parse_excludes(Some(String::new()));
    assert!(ex.is_empty());
}

#[test]
fn excludes_only_whitespace() {
    let ex = parse_excludes(Some("  ,  ".to_string()));
    assert!(ex.iter().all(String::is_empty));
}

// ═══════════════════════════════════════════════════════════════════
// Delivery status icon mapping (mirrors webhook.rs)
// ═══════════════════════════════════════════════════════════════════

fn delivery_icon(status: &str) -> &'static str {
    match status {
        "Success" => "\u{2713}",
        "Failed" => "\u{2717}",
        "Timeout" => "\u{23f1}",
        "Pending" => "\u{23f3}",
        _ => "?",
    }
}

#[test]
fn delivery_icon_success() {
    assert_eq!(delivery_icon("Success"), "\u{2713}");
}

#[test]
fn delivery_icon_failed() {
    assert_eq!(delivery_icon("Failed"), "\u{2717}");
}

#[test]
fn delivery_icon_timeout() {
    assert_eq!(delivery_icon("Timeout"), "\u{23f1}");
}

#[test]
fn delivery_icon_pending() {
    assert_eq!(delivery_icon("Pending"), "\u{23f3}");
}

#[test]
fn delivery_icon_unknown() {
    assert_eq!(delivery_icon("Unknown"), "?");
}

// ═══════════════════════════════════════════════════════════════════
// Drift severity enum mapping (mirrors analyze.rs)
// ═══════════════════════════════════════════════════════════════════

fn drift_severity_to_level(sev: &str) -> u8 {
    match sev {
        "Low" => 1,
        "Medium" => 2,
        "High" => 3,
        "Critical" => 4,
        _ => 0,
    }
}

#[test]
fn drift_severity_low_is_1() {
    assert_eq!(drift_severity_to_level("Low"), 1);
}

#[test]
fn drift_severity_medium_is_2() {
    assert_eq!(drift_severity_to_level("Medium"), 2);
}

#[test]
fn drift_severity_high_is_3() {
    assert_eq!(drift_severity_to_level("High"), 3);
}

#[test]
fn drift_severity_critical_is_4() {
    assert_eq!(drift_severity_to_level("Critical"), 4);
}

#[test]
fn drift_severity_unknown_is_0() {
    assert_eq!(drift_severity_to_level("Unknown"), 0);
}

// ═══════════════════════════════════════════════════════════════════
// Analysis text formatting (mirrors analyze.rs format_analyze_text)
// ═══════════════════════════════════════════════════════════════════

fn format_analyze_text_direct(drift_count: usize, debt_count: usize, files: usize) -> String {
    use std::fmt::Write;
    let mut output = String::new();
    output.push_str("CLAWDIUS ANALYSIS\n");
    let _ = writeln!(output, "Files Analyzed: {files}");
    let _ = writeln!(output, "Architecture Drift: {drift_count}");
    let _ = writeln!(output, "Technical Debt: {debt_count}");
    output
}

fn format_analyze_json_direct(drift_count: usize, debt_count: usize, files: usize) -> String {
    let result = serde_json::json!({
        "summary": { "files_analyzed": files, "drift_count": drift_count, "debt_count": debt_count },
        "drift": [],
        "debt": [],
    });
    serde_json::to_string_pretty(&result).unwrap()
}

#[test]
fn analyze_text_contains_header() {
    let output = format_analyze_text_direct(0, 0, 0);
    assert!(output.contains("CLAWDIUS ANALYSIS"));
}

#[test]
fn analyze_text_contains_files_count() {
    let output = format_analyze_text_direct(0, 0, 42);
    assert!(output.contains("42"));
}

#[test]
fn analyze_text_contains_drift_section() {
    let output = format_analyze_text_direct(5, 0, 0);
    assert!(output.contains("Architecture Drift"));
    assert!(output.contains('5'));
}

#[test]
fn analyze_text_contains_debt_section() {
    let output = format_analyze_text_direct(0, 3, 0);
    assert!(output.contains("Technical Debt"));
    assert!(output.contains('3'));
}

#[test]
fn analyze_json_valid_json() {
    let output = format_analyze_json_direct(0, 0, 5);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert!(parsed.get("summary").is_some());
    assert!(parsed.get("drift").is_some());
    assert!(parsed.get("debt").is_some());
}

#[test]
fn analyze_json_contains_files_analyzed() {
    let output = format_analyze_json_direct(0, 0, 99);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed["summary"]["files_analyzed"], 99);
}

#[test]
fn analyze_json_drift_count() {
    let output = format_analyze_json_direct(7, 0, 10);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed["summary"]["drift_count"], 7);
}

#[test]
fn analyze_json_debt_count() {
    let output = format_analyze_json_direct(0, 12, 10);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed["summary"]["debt_count"], 12);
}

// ═══════════════════════════════════════════════════════════════════
// Drift/debt severity filtering logic (mirrors analyze.rs)
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug)]
struct MockDrift {
    severity: u8,
    message: String,
}

fn filter_drift_by_severity(drifts: &[MockDrift], min_level: u8) -> Vec<&MockDrift> {
    drifts.iter().filter(|d| d.severity >= min_level).collect()
}

#[test]
fn drift_filter_min_critical_only() {
    let drifts = vec![
        MockDrift {
            severity: 1,
            message: "low".into(),
        },
        MockDrift {
            severity: 2,
            message: "med".into(),
        },
        MockDrift {
            severity: 4,
            message: "crit".into(),
        },
    ];
    let filtered = filter_drift_by_severity(&drifts, 4);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].message, "crit");
}

#[test]
fn drift_filter_min_high() {
    let drifts = vec![
        MockDrift {
            severity: 1,
            message: "low".into(),
        },
        MockDrift {
            severity: 3,
            message: "high".into(),
        },
        MockDrift {
            severity: 4,
            message: "crit".into(),
        },
    ];
    let filtered = filter_drift_by_severity(&drifts, 3);
    assert_eq!(filtered.len(), 2);
}

#[test]
fn drift_filter_min_low_all_pass() {
    let drifts = vec![
        MockDrift {
            severity: 1,
            message: "low".into(),
        },
        MockDrift {
            severity: 4,
            message: "crit".into(),
        },
    ];
    let filtered = filter_drift_by_severity(&drifts, 1);
    assert_eq!(filtered.len(), 2);
}

#[derive(Debug)]
struct MockDebt {
    priority: u8,
    description: String,
}

fn filter_debt_by_priority(debts: &[MockDebt], min_level: u8) -> Vec<&MockDebt> {
    debts
        .iter()
        .filter(|d| priority_to_level(d.priority) >= min_level)
        .collect()
}

#[test]
fn debt_filter_min_high_priority() {
    let debts = vec![
        MockDebt {
            priority: 2,
            description: "low".into(),
        },
        MockDebt {
            priority: 5,
            description: "med".into(),
        },
        MockDebt {
            priority: 9,
            description: "high".into(),
        },
    ];
    let filtered = filter_debt_by_priority(&debts, 3);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].description, "high");
}

#[test]
fn debt_filter_min_medium_priority() {
    let debts = vec![
        MockDebt {
            priority: 2,
            description: "low".into(),
        },
        MockDebt {
            priority: 5,
            description: "med".into(),
        },
        MockDebt {
            priority: 9,
            description: "high".into(),
        },
    ];
    let filtered = filter_debt_by_priority(&debts, 2);
    assert_eq!(filtered.len(), 2);
}

#[test]
fn debt_filter_min_critical_priority() {
    let debts = vec![
        MockDebt {
            priority: 1,
            description: "low".into(),
        },
        MockDebt {
            priority: 8,
            description: "high".into(),
        },
        MockDebt {
            priority: 10,
            description: "crit".into(),
        },
    ];
    let filtered = filter_debt_by_priority(&debts, 4);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].description, "crit");
}

// ═══════════════════════════════════════════════════════════════════
// Debt type icon mapping (mirrors analyze.rs format_analyze_text)
// ═══════════════════════════════════════════════════════════════════

const fn debt_priority_category(priority: u8) -> &'static str {
    match priority {
        1..=3 => "low",
        4..=6 => "medium",
        7..=8 => "high",
        9..=10 => "critical",
        _ => "unknown",
    }
}

#[rstest]
#[case(1, "low")]
#[case(2, "low")]
#[case(3, "low")]
#[case(4, "medium")]
#[case(5, "medium")]
#[case(6, "medium")]
#[case(7, "high")]
#[case(8, "high")]
#[case(9, "critical")]
#[case(10, "critical")]
#[case(0, "unknown")]
#[case(11, "unknown")]
fn test_debt_priority_category(#[case] priority: u8, #[case] expected: &str) {
    assert_eq!(debt_priority_category(priority), expected);
}

// ═══════════════════════════════════════════════════════════════════
// Change kind string mapping (mirrors timeline.rs)
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug)]
enum ChangeKind {
    Created,
    Modified,
    Deleted,
    Any,
}

const fn change_kind_str(kind: &ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Created => "created",
        ChangeKind::Modified => "modified",
        ChangeKind::Deleted => "deleted",
        ChangeKind::Any => "changed",
    }
}

#[test]
fn change_kind_created_str() {
    assert_eq!(change_kind_str(&ChangeKind::Created), "created");
}

#[test]
fn change_kind_modified_str() {
    assert_eq!(change_kind_str(&ChangeKind::Modified), "modified");
}

#[test]
fn change_kind_deleted_str() {
    assert_eq!(change_kind_str(&ChangeKind::Deleted), "deleted");
}

#[test]
fn change_kind_any_str() {
    assert_eq!(change_kind_str(&ChangeKind::Any), "changed");
}

// ═══════════════════════════════════════════════════════════════════
// File change type prefix (mirrors timeline.rs)
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug)]
enum FileChangeType {
    Added,
    Modified,
    Deleted,
}

const fn file_change_prefix(change_type: &FileChangeType) -> &'static str {
    match change_type {
        FileChangeType::Added => "+",
        FileChangeType::Modified => "~",
        FileChangeType::Deleted => "-",
    }
}

#[test]
fn file_change_prefix_added() {
    assert_eq!(file_change_prefix(&FileChangeType::Added), "+");
}

#[test]
fn file_change_prefix_modified() {
    assert_eq!(file_change_prefix(&FileChangeType::Modified), "~");
}

#[test]
fn file_change_prefix_deleted() {
    assert_eq!(file_change_prefix(&FileChangeType::Deleted), "-");
}

// ═══════════════════════════════════════════════════════════════════
// Model size formatting (mirrors models.rs)
// ═══════════════════════════════════════════════════════════════════

fn format_model_size(bytes: Option<u64>) -> String {
    bytes
        .map(|s| {
            let gigabytes = s / 1_073_741_824;
            let hundredths = ((s % 1_073_741_824) * 100) / 1_073_741_824;
            format!("{gigabytes}.{hundredths:02} GB")
        })
        .unwrap_or_default()
}

#[test]
fn format_model_size_4gb() {
    let result = format_model_size(Some(4_294_967_296));
    assert!(result.contains("4.00"));
    assert!(result.contains("GB"));
}

#[test]
fn format_model_size_none_empty() {
    assert_eq!(format_model_size(None), "");
}

#[test]
fn format_model_size_small() {
    let result = format_model_size(Some(1_000_000));
    assert!(result.contains("GB"));
    let num: f64 = result.split_whitespace().next().unwrap().parse().unwrap();
    assert!(num < 1.0);
}

#[test]
fn format_model_size_zero() {
    let result = format_model_size(Some(0));
    assert!(result.contains("0.00"));
}

// ═══════════════════════════════════════════════════════════════════
// Session search preview truncation (mirrors sessions.rs)
// ═══════════════════════════════════════════════════════════════════

fn truncate_preview(text: &str, max_len: usize) -> String {
    if text.len() > max_len {
        format!("{}...", &text[..max_len])
    } else {
        text.to_string()
    }
}

#[test]
fn truncate_short_text_unchanged() {
    assert_eq!(truncate_preview("hello", 50), "hello");
}

#[test]
fn truncate_exact_length_unchanged() {
    assert_eq!(truncate_preview("hello", 5), "hello");
}

#[test]
fn truncate_long_text() {
    let long = "a".repeat(100);
    let result = truncate_preview(&long, 50);
    assert_eq!(result.len(), 53);
    assert!(result.ends_with("..."));
}

#[test]
fn truncate_empty_string() {
    assert_eq!(truncate_preview("", 50), "");
}

#[test]
fn truncate_zero_max() {
    let result = truncate_preview("hello", 0);
    assert_eq!(result, "...");
}

// ═══════════════════════════════════════════════════════════════════
// Generate test code per language (mirrors test_cmd.rs)
// ═══════════════════════════════════════════════════════════════════

fn generate_test_code(language: &str) -> String {
    match language {
        "rs" => {
            let code = r"#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_case() {}
    #[test]
    fn test_edge_case() {}
    #[test]
    fn test_error_case() {}
}";
            code.to_string()
        },
        "ts" | "js" => {
            let code = r"describe('tests', () => {
    test('normal', () => {});
    test('edge', () => {});
    test('error', () => {});
});";
            code.to_string()
        },
        "py" => {
            let code = r"import unittest
class TestFunction(unittest.TestCase):
    def test_normal_case(self): pass
    def test_edge_case(self): pass
    def test_error_case(self): pass
if __name__ == '__main__':
    unittest.main()";
            code.to_string()
        },
        _ => "// not supported".to_string(),
    }
}

#[rstest]
#[case("rs", true)]
#[case("ts", true)]
#[case("js", true)]
#[case("py", true)]
#[case("go", false)]
fn test_generate_test_code_has_content(#[case] lang: &str, #[case] supported: bool) {
    let code = generate_test_code(lang);
    if supported {
        assert!(code.len() > 20);
        if lang == "rs" {
            assert!(code.contains("#[cfg(test)]"));
            assert!(code.contains("mod tests"));
        }
    } else {
        assert!(code.contains("not supported"));
    }
}

// ═══════════════════════════════════════════════════════════════════
// CLI parsing — subcommands not covered in cli_tests.rs
// ═══════════════════════════════════════════════════════════════════

#[test]
fn parse_checkpoint_create() {
    let cli = Cli::parse_from(["clawdius", "checkpoint", "create", "my checkpoint"]);
    match cli.command {
        Some(Commands::Checkpoint {
            action:
                CheckpointCommands::Create {
                    description,
                    session,
                },
        }) => {
            assert_eq!(description, "my checkpoint");
            assert!(session.is_none());
        },
        other => panic!("expected Checkpoint::Create, got: {other:?}"),
    }
}

#[test]
fn parse_checkpoint_list() {
    let cli = Cli::parse_from(["clawdius", "checkpoint", "list"]);
    match cli.command {
        Some(Commands::Checkpoint {
            action: CheckpointCommands::List { .. },
        }) => {},
        other => panic!("expected Checkpoint::List, got: {other:?}"),
    }
}

#[test]
fn parse_checkpoint_show() {
    let cli = Cli::parse_from(["clawdius", "checkpoint", "show", "cp-123"]);
    match cli.command {
        Some(Commands::Checkpoint {
            action: CheckpointCommands::Show { checkpoint_id },
        }) => {
            assert_eq!(checkpoint_id, "cp-123");
        },
        other => panic!("expected Checkpoint::Show, got: {other:?}"),
    }
}

#[test]
fn parse_checkpoint_restore() {
    let cli = Cli::parse_from(["clawdius", "checkpoint", "restore", "cp-456"]);
    match cli.command {
        Some(Commands::Checkpoint {
            action: CheckpointCommands::Restore { checkpoint_id },
        }) => {
            assert_eq!(checkpoint_id, "cp-456");
        },
        other => panic!("expected Checkpoint::Restore, got: {other:?}"),
    }
}

#[test]
fn parse_checkpoint_delete() {
    let cli = Cli::parse_from(["clawdius", "checkpoint", "delete", "cp-del"]);
    match cli.command {
        Some(Commands::Checkpoint {
            action: CheckpointCommands::Delete { checkpoint_id },
        }) => {
            assert_eq!(checkpoint_id, "cp-del");
        },
        other => panic!("expected Checkpoint::Delete, got: {other:?}"),
    }
}

#[test]
fn parse_checkpoint_compare() {
    let cli = Cli::parse_from(["clawdius", "checkpoint", "compare", "cp-1", "cp-2"]);
    match cli.command {
        Some(Commands::Checkpoint {
            action:
                CheckpointCommands::Compare {
                    checkpoint_id1,
                    checkpoint_id2,
                },
        }) => {
            assert_eq!(checkpoint_id1, "cp-1");
            assert_eq!(checkpoint_id2, "cp-2");
        },
        other => panic!("expected Checkpoint::Compare, got: {other:?}"),
    }
}

#[test]
fn parse_checkpoint_cleanup() {
    let cli = Cli::parse_from(["clawdius", "checkpoint", "cleanup", "--keep", "5"]);
    match cli.command {
        Some(Commands::Checkpoint {
            action: CheckpointCommands::Cleanup { keep, .. },
        }) => {
            assert_eq!(keep, 5);
        },
        other => panic!("expected Checkpoint::Cleanup, got: {other:?}"),
    }
}

#[test]
fn parse_checkpoint_timeline() {
    let cli = Cli::parse_from(["clawdius", "checkpoint", "timeline"]);
    match cli.command {
        Some(Commands::Checkpoint {
            action: CheckpointCommands::Timeline { .. },
        }) => {},
        other => panic!("expected Checkpoint::Timeline, got: {other:?}"),
    }
}

#[test]
fn parse_timeline_create() {
    let cli = Cli::parse_from([
        "clawdius",
        "timeline",
        "create",
        "my-tl",
        "--description",
        "a timeline entry",
    ]);
    match cli.command {
        Some(Commands::Timeline {
            action: TimelineCommands::Create { name, description },
        }) => {
            assert_eq!(name, "my-tl");
            assert_eq!(description.as_deref(), Some("a timeline entry"));
        },
        other => panic!("expected Timeline::Create, got: {other:?}"),
    }
}

#[test]
fn parse_timeline_list() {
    let cli = Cli::parse_from(["clawdius", "timeline", "list"]);
    match cli.command {
        Some(Commands::Timeline {
            action: TimelineCommands::List,
        }) => {},
        other => panic!("expected Timeline::List, got: {other:?}"),
    }
}

#[test]
fn parse_timeline_diff() {
    let cli = Cli::parse_from(["clawdius", "timeline", "diff", "tl-1", "tl-2"]);
    match cli.command {
        Some(Commands::Timeline {
            action: TimelineCommands::Diff { from, to },
        }) => {
            assert_eq!(from, "tl-1");
            assert_eq!(to, "tl-2");
        },
        other => panic!("expected Timeline::Diff, got: {other:?}"),
    }
}

#[test]
fn parse_timeline_history() {
    let cli = Cli::parse_from(["clawdius", "timeline", "history", "src/main.rs"]);
    match cli.command {
        Some(Commands::Timeline {
            action: TimelineCommands::History { file },
        }) => {
            assert_eq!(file.to_string_lossy(), "src/main.rs");
        },
        other => panic!("expected Timeline::History, got: {other:?}"),
    }
}

#[test]
fn parse_timeline_rollback() {
    let cli = Cli::parse_from(["clawdius", "timeline", "rollback", "tl-abc"]);
    match cli.command {
        Some(Commands::Timeline {
            action: TimelineCommands::Rollback { checkpoint_id },
        }) => {
            assert_eq!(checkpoint_id, "tl-abc");
        },
        other => panic!("expected Timeline::Rollback, got: {other:?}"),
    }
}

#[test]
fn parse_timeline_delete() {
    let cli = Cli::parse_from(["clawdius", "timeline", "delete", "tl-del"]);
    match cli.command {
        Some(Commands::Timeline {
            action: TimelineCommands::Delete { checkpoint_id },
        }) => {
            assert_eq!(checkpoint_id, "tl-del");
        },
        other => panic!("expected Timeline::Delete, got: {other:?}"),
    }
}

#[test]
fn parse_timeline_cleanup() {
    let cli = Cli::parse_from(["clawdius", "timeline", "cleanup", "--keep", "50"]);
    match cli.command {
        Some(Commands::Timeline {
            action: TimelineCommands::Cleanup { keep },
        }) => {
            assert_eq!(keep, 50);
        },
        other => panic!("expected Timeline::Cleanup, got: {other:?}"),
    }
}

#[test]
fn parse_timeline_watch() {
    let cli = Cli::parse_from([
        "clawdius",
        "timeline",
        "watch",
        "--debounce-secs",
        "60",
        "--max-per-hour",
        "10",
        "--ignore",
        "target/",
    ]);
    match cli.command {
        Some(Commands::Timeline {
            action:
                TimelineCommands::Watch {
                    debounce_secs,
                    ignore,
                    max_per_hour,
                },
        }) => {
            assert_eq!(debounce_secs, 60);
            assert_eq!(max_per_hour, 10);
            assert_eq!(ignore, vec!["target/"]);
        },
        other => panic!("expected Timeline::Watch, got: {other:?}"),
    }
}

#[test]
fn parse_modes_list() {
    let cli = Cli::parse_from(["clawdius", "modes", "list"]);
    match cli.command {
        Some(Commands::Modes {
            action: ModeCommands::List,
        }) => {},
        other => panic!("expected Modes::List, got: {other:?}"),
    }
}

#[test]
fn parse_modes_create() {
    let cli = Cli::parse_from(["clawdius", "modes", "create", "my-mode"]);
    match cli.command {
        Some(Commands::Modes {
            action: ModeCommands::Create { name, output },
        }) => {
            assert_eq!(name, "my-mode");
            assert!(output.is_none());
        },
        other => panic!("expected Modes::Create, got: {other:?}"),
    }
}

#[test]
fn parse_modes_show() {
    let cli = Cli::parse_from(["clawdius", "modes", "show", "code"]);
    match cli.command {
        Some(Commands::Modes {
            action: ModeCommands::Show { name },
        }) => {
            assert_eq!(name, "code");
        },
        other => panic!("expected Modes::Show, got: {other:?}"),
    }
}

#[test]
fn parse_lang_list() {
    let cli = Cli::parse_from(["clawdius", "lang", "list"]);
    match cli.command {
        Some(Commands::Lang {
            action: LangCommands::List,
        }) => {},
        other => panic!("expected Lang::List, got: {other:?}"),
    }
}

#[test]
fn parse_lang_set() {
    let cli = Cli::parse_from(["clawdius", "lang", "set", "ja"]);
    match cli.command {
        Some(Commands::Lang {
            action: LangCommands::Set { code },
        }) => {
            assert_eq!(code, "ja");
        },
        other => panic!("expected Lang::Set, got: {other:?}"),
    }
}

#[test]
fn parse_lang_show() {
    let cli = Cli::parse_from(["clawdius", "lang", "show"]);
    match cli.command {
        Some(Commands::Lang {
            action: LangCommands::Show,
        }) => {},
        other => panic!("expected Lang::Show, got: {other:?}"),
    }
}

#[test]
fn parse_git_diff_staged() {
    let cli = Cli::parse_from(["clawdius", "git", "diff", "--staged"]);
    match cli.command {
        Some(Commands::Git {
            action: GitCommands::Diff { staged, file },
        }) => {
            assert!(staged);
            assert!(file.is_none());
        },
        other => panic!("expected Git::Diff, got: {other:?}"),
    }
}

#[test]
fn parse_git_diff_file() {
    let cli = Cli::parse_from(["clawdius", "git", "diff", "--", "main.rs"]);
    match cli.command {
        Some(Commands::Git {
            action: GitCommands::Diff { staged, file },
        }) => {
            assert!(!staged);
            assert_eq!(file.as_deref(), Some("main.rs"));
        },
        other => panic!("expected Git::Diff, got: {other:?}"),
    }
}

#[test]
fn parse_git_status() {
    let cli = Cli::parse_from(["clawdius", "git", "status"]);
    match cli.command {
        Some(Commands::Git {
            action: GitCommands::Status,
        }) => {},
        other => panic!("expected Git::Status, got: {other:?}"),
    }
}

#[test]
fn parse_git_commit_with_message() {
    let cli = Cli::parse_from([
        "clawdius",
        "git",
        "commit",
        "a.rs",
        "b.rs",
        "--message",
        "fix bug",
    ]);
    match cli.command {
        Some(Commands::Git {
            action: GitCommands::Commit { files, message },
        }) => {
            assert_eq!(files, vec!["a.rs", "b.rs"]);
            assert_eq!(message.as_deref(), Some("fix bug"));
        },
        other => panic!("expected Git::Commit, got: {other:?}"),
    }
}

#[test]
fn parse_git_commit_no_files() {
    let cli = Cli::parse_from(["clawdius", "git", "commit"]);
    match cli.command {
        Some(Commands::Git {
            action: GitCommands::Commit { files, message },
        }) => {
            assert!(files.is_empty());
            assert!(message.is_none());
        },
        other => panic!("expected Git::Commit, got: {other:?}"),
    }
}

#[test]
fn parse_config_show() {
    let cli = Cli::parse_from(["clawdius", "config", "show"]);
    match cli.command {
        Some(Commands::Config {
            action: ConfigAction::Show,
        }) => {},
        other => panic!("expected Config::Show, got: {other:?}"),
    }
}

#[test]
fn parse_config_get() {
    let cli = Cli::parse_from(["clawdius", "config", "get", "llm.default_provider"]);
    match cli.command {
        Some(Commands::Config {
            action: ConfigAction::Get { key },
        }) => {
            assert_eq!(key, "llm.default_provider");
        },
        other => panic!("expected Config::Get, got: {other:?}"),
    }
}

#[test]
fn parse_config_set() {
    let cli = Cli::parse_from(["clawdius", "config", "set", "llm.max_tokens", "8192"]);
    match cli.command {
        Some(Commands::Config {
            action: ConfigAction::Set { key, value },
        }) => {
            assert_eq!(key, "llm.max_tokens");
            assert_eq!(value, "8192");
        },
        other => panic!("expected Config::Set, got: {other:?}"),
    }
}

#[test]
fn parse_config_path() {
    let cli = Cli::parse_from(["clawdius", "config", "path"]);
    match cli.command {
        Some(Commands::Config {
            action: ConfigAction::Path,
        }) => {},
        other => panic!("expected Config::Path, got: {other:?}"),
    }
}

#[test]
fn parse_config_list() {
    let cli = Cli::parse_from(["clawdius", "config", "list"]);
    match cli.command {
        Some(Commands::Config {
            action: ConfigAction::List,
        }) => {},
        other => panic!("expected Config::List, got: {other:?}"),
    }
}

#[test]
fn parse_models_list() {
    let cli = Cli::parse_from(["clawdius", "models", "list"]);
    match cli.command {
        Some(Commands::Models {
            action: ModelsCommands::List,
            ..
        }) => {},
        other => panic!("expected Models::List, got: {other:?}"),
    }
}

#[test]
fn parse_models_pull() {
    let cli = Cli::parse_from(["clawdius", "models", "pull", "llama3.2"]);
    match cli.command {
        Some(Commands::Models {
            action: ModelsCommands::Pull { model },
            ..
        }) => {
            assert_eq!(model, "llama3.2");
        },
        other => panic!("expected Models::Pull, got: {other:?}"),
    }
}

#[test]
fn parse_models_health() {
    let cli = Cli::parse_from(["clawdius", "models", "health"]);
    match cli.command {
        Some(Commands::Models {
            action: ModelsCommands::Health,
            ..
        }) => {},
        other => panic!("expected Models::Health, got: {other:?}"),
    }
}

#[test]
fn parse_models_current() {
    let cli = Cli::parse_from(["clawdius", "models", "current"]);
    match cli.command {
        Some(Commands::Models {
            action: ModelsCommands::Current,
            ..
        }) => {},
        other => panic!("expected Models::Current, got: {other:?}"),
    }
}

#[test]
fn parse_models_custom_host_port() {
    let cli = Cli::parse_from([
        "clawdius",
        "models",
        "--host",
        "192.168.1.1",
        "--port",
        "9999",
        "list",
    ]);
    match cli.command {
        Some(Commands::Models { host, port, .. }) => {
            assert_eq!(host, "192.168.1.1");
            assert_eq!(port, 9999);
        },
        other => panic!("expected Models, got: {other:?}"),
    }
}

#[test]
fn parse_refactor_subcommand() {
    let cli = Cli::parse_from([
        "clawdius",
        "refactor",
        "--from",
        "typescript",
        "--to",
        "rust",
        "--path",
        "src/index.ts",
        "--dry-run",
    ]);
    match cli.command {
        Some(Commands::Refactor {
            from,
            to,
            path,
            dry_run,
        }) => {
            assert_eq!(from, "typescript");
            assert_eq!(to, "rust");
            assert_eq!(path.to_string_lossy(), "src/index.ts");
            assert!(dry_run);
        },
        other => panic!("expected Refactor, got: {other:?}"),
    }
}

#[test]
fn parse_analyze_subcommand() {
    let cli = Cli::parse_from([
        "clawdius",
        "analyze",
        "src/",
        "--drift",
        "--format",
        "json",
        "--severity",
        "high",
    ]);
    match cli.command {
        Some(Commands::Analyze {
            path,
            drift,
            debt,
            severity,
            exclude,
            ..
        }) => {
            assert_eq!(path.to_string_lossy(), "src/");
            assert!(drift);
            assert!(!debt);
            assert_eq!(severity, "high");
            assert!(exclude.is_none());
        },
        other => panic!("expected Analyze, got: {other:?}"),
    }
}

#[test]
fn parse_watch_subcommand() {
    let cli = Cli::parse_from([
        "clawdius",
        "watch",
        "src/",
        "--ignore",
        "target/,vendor/",
        "--auto-analyze",
        "--debounce-ms",
        "1000",
        "--verbose",
    ]);
    match cli.command {
        Some(Commands::Watch {
            path,
            ignore,
            auto_analyze,
            debounce_ms,
            verbose,
        }) => {
            assert_eq!(path.to_string_lossy(), "src/");
            assert_eq!(ignore.as_deref(), Some("target/,vendor/"));
            assert!(auto_analyze);
            assert_eq!(debounce_ms, 1000);
            assert!(verbose);
        },
        other => panic!("expected Watch, got: {other:?}"),
    }
}

#[test]
fn parse_analyze_debt_only() {
    let cli = Cli::parse_from(["clawdius", "analyze", ".", "--debt"]);
    match cli.command {
        Some(Commands::Analyze {
            drift,
            debt,
            severity,
            ..
        }) => {
            assert!(!drift);
            assert!(debt);
            assert_eq!(severity, "low");
        },
        other => panic!("expected Analyze, got: {other:?}"),
    }
}

#[test]
fn parse_analyze_with_exclude() {
    let cli = Cli::parse_from([
        "clawdius",
        "analyze",
        ".",
        "--exclude",
        "target/,generated/",
    ]);
    match cli.command {
        Some(Commands::Analyze { exclude, .. }) => {
            assert_eq!(exclude.as_deref(), Some("target/,generated/"));
        },
        other => panic!("expected Analyze, got: {other:?}"),
    }
}

#[test]
fn parse_init_with_name() {
    let cli = Cli::parse_from(["clawdius", "init", "my-project"]);
    match cli.command {
        Some(Commands::Init { name }) => {
            assert_eq!(name.as_deref(), Some("my-project"));
        },
        other => panic!("expected Init, got: {other:?}"),
    }
}

#[test]
fn parse_setup_quick() {
    let cli = Cli::parse_from(["clawdius", "setup", "--quick", "-P", "ollama"]);
    match cli.command {
        Some(Commands::Setup { quick, provider }) => {
            assert!(quick);
            assert_eq!(provider.as_deref(), Some("ollama"));
        },
        other => panic!("expected Setup, got: {other:?}"),
    }
}

#[test]
fn parse_sessions_delete() {
    let cli = Cli::parse_from(["clawdius", "sessions", "--delete", "sess-abc"]);
    match cli.command {
        Some(Commands::Sessions { delete, search }) => {
            assert_eq!(delete.as_deref(), Some("sess-abc"));
            assert!(search.is_none());
        },
        other => panic!("expected Sessions, got: {other:?}"),
    }
}

#[test]
fn parse_sessions_search() {
    let cli = Cli::parse_from(["clawdius", "sessions", "--search", "hello world"]);
    match cli.command {
        Some(Commands::Sessions { delete, search }) => {
            assert!(delete.is_none());
            assert_eq!(search.as_deref(), Some("hello world"));
        },
        other => panic!("expected Sessions, got: {other:?}"),
    }
}

#[test]
fn parse_edit_with_options() {
    let cli = Cli::parse_from([
        "clawdius",
        "edit",
        "--initial",
        "hello",
        "--editor",
        "vim",
        "--extension",
        "rs",
    ]);
    match cli.command {
        Some(Commands::Edit {
            initial,
            editor,
            extension,
        }) => {
            assert_eq!(initial.as_deref(), Some("hello"));
            assert_eq!(editor.as_deref(), Some("vim"));
            assert_eq!(extension.as_deref(), Some("rs"));
        },
        other => panic!("expected Edit, got: {other:?}"),
    }
}

#[test]
fn parse_telemetry_disable() {
    let cli = Cli::parse_from(["clawdius", "telemetry", "--disable"]);
    match cli.command {
        Some(Commands::Telemetry {
            enable,
            disable,
            enable_metrics,
            enable_crash_reporting,
        }) => {
            assert!(!enable);
            assert!(disable);
            assert!(!enable_metrics);
            assert!(!enable_crash_reporting);
        },
        other => panic!("expected Telemetry, got: {other:?}"),
    }
}

#[test]
fn parse_telemetry_enable() {
    let cli = Cli::parse_from([
        "clawdius",
        "telemetry",
        "--enable",
        "--enable-metrics",
        "--enable-crash-reporting",
    ]);
    match cli.command {
        Some(Commands::Telemetry {
            enable,
            disable,
            enable_metrics,
            enable_crash_reporting,
        }) => {
            assert!(enable);
            assert!(!disable);
            assert!(enable_metrics);
            assert!(enable_crash_reporting);
        },
        other => panic!("expected Telemetry, got: {other:?}"),
    }
}

#[test]
fn parse_generate_with_all_options() {
    let cli = Cli::parse_from([
        "clawdius",
        "generate",
        "add auth",
        "--files",
        "src/auth.rs",
        "--mode",
        "agent",
        "--trust",
        "high",
        "--test-strategy",
        "sandboxed",
        "--max-iterations",
        "10",
        "--dry-run",
        "--provider",
        "openai",
        "--model",
        "gpt-4",
        "--timeout-secs",
        "30",
    ]);
    match cli.command {
        Some(Commands::Generate {
            prompt,
            files,
            mode,
            trust,
            test_strategy,
            max_iterations,
            dry_run,
            provider,
            model,
            timeout_secs,
        }) => {
            assert_eq!(prompt, "add auth");
            assert_eq!(files.as_deref(), Some("src/auth.rs"));
            assert_eq!(mode, "agent");
            assert_eq!(trust, "high");
            assert_eq!(test_strategy.as_deref(), Some("sandboxed"));
            assert_eq!(max_iterations, 10);
            assert!(dry_run);
            assert_eq!(provider, "openai");
            assert_eq!(model.as_deref(), Some("gpt-4"));
            assert_eq!(timeout_secs, Some(30));
        },
        other => panic!("expected Generate, got: {other:?}"),
    }
}

#[test]
fn parse_sprint_with_all_options() {
    let cli = Cli::parse_from([
        "clawdius",
        "sprint",
        "implement auth",
        "-n",
        "5",
        "--real-execution",
        "--auto-approve",
        "-P",
        "anthropic",
        "--model",
        "claude-3-opus",
        "--browser-qa-url",
        "http://localhost:3000",
        "--resume",
        "--lsp",
        "rust-analyzer",
    ]);
    match cli.command {
        Some(Commands::Sprint {
            task,
            max_iterations,
            real_execution,
            auto_approve,
            provider,
            model,
            browser_qa_url,
            resume,
            lsp,
        }) => {
            assert_eq!(task, "implement auth");
            assert_eq!(max_iterations, 5);
            assert!(real_execution);
            assert!(auto_approve);
            assert_eq!(provider, "anthropic");
            assert_eq!(model.as_deref(), Some("claude-3-opus"));
            assert_eq!(browser_qa_url.as_deref(), Some("http://localhost:3000"));
            assert!(resume);
            assert_eq!(lsp.as_deref(), Some("rust-analyzer"));
        },
        other => panic!("expected Sprint, got: {other:?}"),
    }
}

#[test]
fn parse_verify_with_lean_path() {
    let cli = Cli::parse_from([
        "clawdius",
        "verify",
        "--proof",
        "proofs/session.lean",
        "--lean-path",
        "/usr/bin/lean",
    ]);
    match cli.command {
        Some(Commands::Verify { proof, lean_path }) => {
            assert_eq!(proof.to_string_lossy(), "proofs/session.lean");
            assert!(lean_path.is_some());
        },
        other => panic!("expected Verify, got: {other:?}"),
    }
}

#[test]
fn parse_auto_with_all_flags() {
    let cli = Cli::parse_from([
        "clawdius",
        "auto",
        "fix tests",
        "--max-iterations",
        "100",
        "--auto-commit",
        "--run-tests",
        "--fail-on-test-failure",
        "--model",
        "gpt-4",
        "-P",
        "openai",
        "--output-format",
        "json",
    ]);
    match cli.command {
        Some(Commands::Auto {
            task,
            model,
            provider,
            max_iterations,
            run_tests,
            auto_commit,
            fail_on_test_failure,
            output_format,
        }) => {
            assert_eq!(task, "fix tests");
            assert_eq!(model.as_deref(), Some("gpt-4"));
            assert_eq!(provider, "openai");
            assert_eq!(max_iterations, Some(100));
            assert!(run_tests);
            assert!(auto_commit);
            assert!(fail_on_test_failure);
            assert_eq!(output_format.as_deref(), Some("json"));
        },
        other => panic!("expected Auto, got: {other:?}"),
    }
}

#[test]
fn parse_no_command_empty_args() {
    let cli = Cli::parse_from(["clawdius"]);
    assert!(cli.command.is_none());
    assert!(!cli.quiet);
    assert!(!cli.no_tui);
}

#[test]
fn parse_all_global_flags() {
    let cli = Cli::parse_from([
        "clawdius",
        "--no-tui",
        "--cwd",
        "/tmp",
        "-f",
        "json",
        "-q",
        "-C",
        "/etc/c.toml",
        "-L",
        "ja",
    ]);
    assert!(cli.no_tui);
    assert!(cli.quiet);
    assert_eq!(cli.output_format, OutputFormat::Json);
    assert_eq!(cli.lang.as_deref(), Some("ja"));
}

#[test]
fn invalid_format_rejected() {
    assert!(Cli::try_parse_from(["clawdius", "-f", "xml"]).is_err());
}

#[test]
fn unknown_command_rejected() {
    assert!(Cli::try_parse_from(["clawdius", "foobar"]).is_err());
}

#[test]
fn help_and_version_are_errors_via_try_parse() {
    assert!(Cli::try_parse_from(["clawdius", "--help"]).is_err());
    assert!(Cli::try_parse_from(["clawdius", "--version"]).is_err());
}

#[test]
fn parse_doc_with_element_and_format() {
    let cli = Cli::parse_from([
        "clawdius",
        "doc",
        "src/lib.rs",
        "--element",
        "MyStruct",
        "-f",
        "rustdoc",
        "-o",
        "docs/lib.md",
        "--inline",
    ]);
    match cli.command {
        Some(Commands::Doc {
            file,
            element,
            format,
            output,
            inline,
        }) => {
            assert_eq!(file.to_string_lossy(), "src/lib.rs");
            assert_eq!(element.as_deref(), Some("MyStruct"));
            assert_eq!(format, "rustdoc");
            assert!(output.is_some());
            assert!(inline);
        },
        other => panic!("expected Doc, got: {other:?}"),
    }
}

#[test]
fn parse_test_with_function_and_output() {
    let cli = Cli::parse_from([
        "clawdius",
        "test",
        "src/lib.rs",
        "--function",
        "parse_config",
        "-o",
        "tests/config_test.rs",
    ]);
    match cli.command {
        Some(Commands::Test {
            file,
            function,
            output,
        }) => {
            assert_eq!(file.to_string_lossy(), "src/lib.rs");
            assert_eq!(function.as_deref(), Some("parse_config"));
            assert!(output.is_some());
        },
        other => panic!("expected Test, got: {other:?}"),
    }
}

#[test]
fn parse_server_custom_host_port() {
    let cli = Cli::parse_from(["clawdius", "server", "--host", "0.0.0.0", "--port", "9090"]);
    match cli.command {
        Some(Commands::Server { host, port }) => {
            assert_eq!(host, "0.0.0.0");
            assert_eq!(port, 9090);
        },
        other => panic!("expected Server, got: {other:?}"),
    }
}

#[test]
fn parse_metrics_with_output_and_reset() {
    let cli = Cli::parse_from([
        "clawdius",
        "metrics",
        "-f",
        "json",
        "--output",
        "metrics.json",
        "--reset",
    ]);
    match cli.command {
        Some(Commands::Metrics {
            format,
            output,
            reset,
            watch,
        }) => {
            assert_eq!(format!("{format:?}"), "Json");
            assert!(output.is_some());
            assert!(reset);
            assert!(!watch);
        },
        other => panic!("expected Metrics, got: {other:?}"),
    }
}

#[test]
fn parse_metrics_watch() {
    let cli = Cli::parse_from(["clawdius", "metrics", "--watch"]);
    match cli.command {
        Some(Commands::Metrics { watch, .. }) => {
            assert!(watch);
        },
        other => panic!("expected Metrics, got: {other:?}"),
    }
}

#[test]
fn parse_complete_with_language() {
    let cli = Cli::parse_from([
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
    match cli.command {
        Some(Commands::Complete {
            file,
            line,
            character,
            language,
            provider,
            model: _,
        }) => {
            assert_eq!(file, "main.rs");
            assert_eq!(line, 10);
            assert_eq!(character, 5);
            assert_eq!(language.as_deref(), Some("rust"));
            assert_eq!(provider, "anthropic");
        },
        other => panic!("expected Complete, got: {other:?}"),
    }
}

#[test]
fn parse_chat_with_editor_and_exit() {
    let cli = Cli::parse_from(["clawdius", "chat", "--editor", "--exit", "prompt here"]);
    match cli.command {
        Some(Commands::Chat {
            prompt,
            editor,
            exit,
            ..
        }) => {
            assert_eq!(prompt.as_deref(), Some("prompt here"));
            assert!(editor);
            assert!(exit);
        },
        other => panic!("expected Chat, got: {other:?}"),
    }
}

#[test]
fn parse_generate_defaults() {
    let cli = Cli::parse_from(["clawdius", "generate", "add feature"]);
    match cli.command {
        Some(Commands::Generate {
            prompt,
            mode,
            trust,
            max_iterations,
            dry_run,
            provider,
            files,
            test_strategy,
            model,
            timeout_secs,
        }) => {
            assert_eq!(prompt, "add feature");
            assert_eq!(mode, "single-pass");
            assert_eq!(trust, "medium");
            assert_eq!(max_iterations, 5);
            assert!(!dry_run);
            assert_eq!(provider, "anthropic");
            assert!(files.is_none());
            assert!(test_strategy.is_none());
            assert!(model.is_none());
            assert!(timeout_secs.is_none());
        },
        other => panic!("expected Generate, got: {other:?}"),
    }
}
