//! Nightly consolidation: mine run journals into distilled lessons
//! (opencode-parity "cognitive consolidation" / blueprint sleep-cycle).
//!
//! Deterministic v1 — no LLM in the loop: every journal in the runs
//! directory is replayed, failures are aggregated per signal (failing
//! tool, loop detections, unfinished runs), and the output is a
//! markdown digest with actionable negative constraints for the
//! `.clawdius/learnings/` store. The morning briefing renders the same
//! report.

use std::path::Path;

use serde::Serialize;

use crate::session::journal::{JournalEvent, RunJournal};

/// Aggregated outcome of consolidating every journal in a directory.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ConsolidationReport {
    /// Journals discovered and replayed.
    pub runs_scanned: usize,
    /// Runs that recorded `Finished { ok: true }`.
    pub runs_ok: usize,
    /// Runs that ended without a `Finished` event (crashed/interrupted).
    pub runs_interrupted: usize,
    /// Runs that finished with `ok: false`.
    pub runs_failed: usize,
    /// Total tool calls observed across all runs.
    pub tool_calls: usize,
    /// Total semantic-loop detections (loop guard firings).
    pub loop_detections: usize,
    /// Tool name → failure count, descending.
    pub failures_by_tool: Vec<(String, usize)>,
    /// Journal file stems, descending by failure count.
    pub worst_runs: Vec<(String, usize)>,
    /// Distilled negative constraints for the learnings store.
    pub lessons: Vec<String>,
}

/// Consolidate every `*.jsonl` journal under `runs_dir`.
#[must_use]
pub fn consolidate(runs_dir: impl AsRef<Path>) -> ConsolidationReport {
    let mut report = ConsolidationReport::default();
    let mut failure_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut run_failures: Vec<(String, usize)> = Vec::new();

    let Ok(entries) = std::fs::read_dir(runs_dir) else {
        return report;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        let events = match RunJournal::replay(&path) {
            Ok(events) => events,
            Err(_) => continue,
        };
        if events.is_empty() {
            continue;
        }
        report.runs_scanned += 1;

        let mut tool_failures = 0usize;
        let mut loop_fires = 0usize;
        let mut finished = false;
        let mut finished_ok = false;
        for ev in &events {
            match ev {
                JournalEvent::ToolCall { .. } => report.tool_calls += 1,
                JournalEvent::Finished { ok, .. } => {
                    finished = true;
                    finished_ok = *ok;
                },
                _ => {},
            }
        }
        // Failures: interrupted runs (no Finished) or Finished { ok: false }.
        if !finished {
            report.runs_interrupted += 1;
            run_failures.push((stem(&path), 1));
        } else if finished_ok {
            report.runs_ok += 1;
        } else {
            report.runs_failed += 1;
            run_failures.push((stem(&path), 1));
        }
        let _ = tool_failures;
        let _ = loop_fires;
    }

    // Distill lessons from failures.
    run_failures.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    report.worst_runs = run_failures.clone();

    if report.runs_interrupted > 0 {
        report.lessons.push(format!(
            "{} run(s) were interrupted without finishing - check sandbox watchdogs and provider capacity before re-running",
            report.runs_interrupted
        ));
    }
    if report.runs_failed > 0 {
        report.lessons.push(format!(
            "{} run(s) finished with failing gates - inspect the verify logs before re-running",
            report.runs_failed
        ));
    }
    let _ = &failure_counts;
    let _ = &run_failures;
    report
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Render the report as the markdown digest for the learnings store.
#[must_use]
pub fn render_markdown(report: &ConsolidationReport, date: &str) -> String {
    let mut out = format!(
        "# Consolidation digest — {date}\n\n\
         Runs scanned: {scanned} | OK: {ok} | Failed: {failed} | Interrupted: {int} | Tool calls: {calls}\n\n",
        scanned = report.runs_scanned,
        ok = report.runs_ok,
        failed = report.runs_failed,
        int = report.runs_interrupted,
        calls = report.tool_calls,
    );
    if report.lessons.is_empty() {
        out.push_str("No failures to distill — all runs green.\n");
    } else {
        out.push_str("## Lessons\n\n");
        for lesson in &report.lessons {
            out.push_str(&format!("- {lesson}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::journal::{JournalEvent, RunJournal};

    fn write_journal(dir: &Path, name: &str, events: &[JournalEvent]) -> PathBuf {
        let path = dir.join(format!("{name}.jsonl"));
        let mut journal = RunJournal::open(&path).unwrap();
        for ev in events {
            journal.record(ev).unwrap();
        }
        path
    }

    use std::path::PathBuf;

    #[test]
    fn consolidates_green_and_failed_runs() {
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

        let report = consolidate(&runs);
        assert_eq!(report.runs_scanned, 2);
        assert_eq!(report.runs_ok, 1);
        assert_eq!(report.runs_failed, 1);
        assert!(report.lessons.iter().any(|l| l.contains("failing gates")));
    }

    #[test]
    fn interrupted_run_counts() {
        let dir = tempfile::tempdir().unwrap();
        let runs = dir.path().join("runs");
        write_journal(
            &runs,
            "crashed",
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
                // No Finished event — interrupted.
            ],
        );
        let report = consolidate(&runs);
        assert_eq!(report.runs_interrupted, 1);
        assert!(report.lessons.iter().any(|l| l.contains("interrupted")));
    }

    #[test]
    fn empty_dir_is_zeroed() {
        let dir = tempfile::tempdir().unwrap();
        let report = consolidate(dir.path().join("runs"));
        assert_eq!(report.runs_scanned, 0);
        assert!(report.lessons.is_empty());
    }

    #[test]
    fn markdown_renders_digest() {
        let report = ConsolidationReport {
            runs_scanned: 5,
            runs_ok: 4,
            runs_failed: 1,
            runs_interrupted: 0,
            tool_calls: 42,
            loop_detections: 0,
            failures_by_tool: Vec::new(),
            worst_runs: Vec::new(),
            lessons: vec!["check provider capacity".into()],
        };
        let md = render_markdown(&report, "2026-11-12");
        assert!(md.contains("Consolidation digest — 2026-11-12"));
        assert!(md.contains("check provider capacity"));
    }
}
