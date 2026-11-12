//! Morning briefing: renders run journals into a Telegram-ready digest.
//!
//! The briefing is the human-facing half of the consolidation loop
//! (blueprint "morning briefing"): what ran overnight, what failed, what
//! needs a human verdict. v1 is a pure renderer over the same journal
//! replay the consolidation uses — delivery (Telegram via the gateway)
//! is a separate, opt-in step.

use std::path::Path;

use serde::Serialize;

use crate::session::consolidation::consolidate;
use crate::session::journal::JournalEvent;

/// Per-run status line for the briefing.
#[derive(Debug, Clone, Serialize)]
pub struct RunStatus {
    /// Journal file stem (run name).
    pub name: String,
    /// `ok` / `failed` / `interrupted`.
    pub status: &'static str,
    /// Tool calls observed in the run.
    pub tool_calls: usize,
    /// Last `Finished` note, when present.
    pub note: Option<String>,
}

/// The rendered briefing: header, digest, per-run table.
#[derive(Debug, Clone, Serialize)]
pub struct Briefing {
    /// Date stamp (YYYY-MM-DD).
    pub date: String,
    /// Aggregated counts from consolidation.
    pub runs_ok: usize,
    /// Failed runs (Finished with ok: false).
    pub runs_failed: usize,
    /// Interrupted runs (no Finished event).
    pub runs_interrupted: usize,
    /// Total tool calls across runs.
    pub tool_calls: usize,
    /// Per-run status lines, worst first.
    pub runs: Vec<RunStatus>,
    /// Distilled lessons (negative constraints).
    pub lessons: Vec<String>,
}

#[must_use]
pub fn build(runs_dir: impl AsRef<Path>, date: &str) -> Briefing {
    let report = consolidate(runs_dir.as_ref());
    let mut runs = Vec::new();

    // Re-walk journals for per-run status (consolidate() aggregates; the
    // briefing needs the per-run split).
    if let Ok(entries) = std::fs::read_dir(runs_dir.as_ref()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "jsonl") {
                continue;
            }
            let Ok(events) = crate::session::journal::RunJournal::replay(&path) else {
                continue;
            };
            if events.is_empty() {
                continue;
            }
            let mut status = RunStatus {
                name: path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                status: "interrupted",
                tool_calls: 0,
                note: None,
            };
            for ev in &events {
                match ev {
                    JournalEvent::ToolCall { .. } => status.tool_calls += 1,
                    JournalEvent::Finished { ok, note } => {
                        status.status = if *ok { "ok" } else { "failed" };
                        status.note = Some(note.clone());
                    },
                    _ => {},
                }
            }
            runs.push(status);
        }
    }
    runs.sort_by_key(|r| match r.status {
        "interrupted" => 0,
        "failed" => 1,
        _ => 2,
    });

    Briefing {
        date: date.to_string(),
        runs_ok: report.runs_ok,
        runs_failed: report.runs_failed,
        runs_interrupted: report.runs_interrupted,
        tool_calls: report.tool_calls,
        runs,
        lessons: report.lessons,
    }
}

/// Telegram-ready markdown (message-length safe: the gateway chunks it).
#[must_use]
pub fn render_markdown(b: &Briefing) -> String {
    let mut out = format!("*Morning briefing — {date}*\n\n", date = b.date);
    out.push_str(&format!(
        "Runs: {ok} ok / {failed} failed / {interrupted} interrupted · tool calls: {calls}\n",
        ok = b.runs_ok,
        failed = b.runs_failed,
        interrupted = b.runs_interrupted,
        calls = b.tool_calls,
    ));
    if !b.runs.is_empty() {
        out.push_str("\n*Runs*\n");
        for r in b.runs.iter().take(10) {
            let icon = match r.status {
                "ok" => "✅",
                "failed" => "❌",
                _ => "⚠️",
            };
            let note = r
                .note
                .as_deref()
                .map(|n| {
                    let n = n.chars().take(80).collect::<String>();
                    format!(" — {n}")
                })
                .unwrap_or_default();
            out.push_str(&format!(
                "{icon} `{name}` · {status} · {calls} tools{note}\n",
                name = r.name,
                status = r.status,
                calls = r.tool_calls,
                note = note
            ));
        }
    }
    if !b.lessons.is_empty() {
        out.push_str("\n*Needs attention*\n");
        for l in &b.lessons {
            out.push_str(&format!("- {l}\n"));
        }
    } else {
        out.push_str("\nAll runs green — nothing needs a verdict.\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::journal::{JournalEvent, RunJournal};
    use std::path::PathBuf;

    fn write_journal(dir: &Path, name: &str, events: &[JournalEvent]) -> PathBuf {
        let path = dir.join(format!("{name}.jsonl"));
        let mut journal = RunJournal::open(&path).unwrap();
        for ev in events {
            journal.record(ev).unwrap();
        }
        path
    }

    #[test]
    fn briefing_orders_worst_first() {
        let dir = tempfile::tempdir().unwrap();
        let runs = dir.path().join("runs");
        write_journal(
            &runs,
            "green",
            &[
                JournalEvent::Started {
                    task: "t".into(),
                    provider: "google".into(),
                },
                JournalEvent::Finished {
                    ok: true,
                    note: "done".into(),
                },
            ],
        );
        write_journal(
            &runs,
            "broken",
            &[
                JournalEvent::Started {
                    task: "t".into(),
                    provider: "google".into(),
                },
                JournalEvent::Finished {
                    ok: false,
                    note: "tests red".into(),
                },
            ],
        );
        write_journal(
            &runs,
            "crashed",
            &[JournalEvent::Started {
                task: "t".into(),
                provider: "google".into(),
            }],
        );

        let b = build(&runs, "2026-11-12");
        assert_eq!(b.runs_ok, 1);
        assert_eq!(b.runs_failed, 1);
        assert_eq!(b.runs_interrupted, 1);
        assert_eq!(b.runs[0].status, "interrupted");
        assert_eq!(b.runs[1].status, "failed");
        assert_eq!(b.runs[2].status, "ok");
        assert_eq!(b.runs[2].note.as_deref(), Some("done"));
    }

    #[test]
    fn markdown_renders_telegram_shape() {
        let dir = tempfile::tempdir().unwrap();
        let runs = dir.path().join("runs");
        write_journal(
            &runs,
            "green",
            &[
                JournalEvent::Started {
                    task: "t".into(),
                    provider: "google".into(),
                },
                JournalEvent::ToolCall {
                    n: 1,
                    name: "shell".into(),
                    args: "{}".into(),
                },
                JournalEvent::Finished {
                    ok: true,
                    note: "repaired".into(),
                },
            ],
        );
        let b = build(&runs, "2026-11-12");
        let md = render_markdown(&b);
        assert!(md.contains("*Morning briefing — 2026-11-12*"));
        assert!(md.contains("1 ok / 0 failed / 0 interrupted"));
        assert!(md.contains("✅ `green` · ok · 1 tools — repaired"));
        assert!(md.contains("All runs green"));
    }

    #[test]
    fn empty_runs_dir_renders_clean_briefing() {
        let dir = tempfile::tempdir().unwrap();
        let b = build(dir.path().join("runs"), "2026-11-12");
        assert_eq!(b.runs_ok + b.runs_failed + b.runs_interrupted, 0);
        assert!(render_markdown(&b).contains("All runs green"));
    }
}
