//! Run journal: crash-safe, append-only JSONL record of one agentic run.
//!
//! opencode-parity row (durable runs). Every agent run writes
//! `Started / ToolCall / ToolResult / Finished` events to a JSONL file;
//! a crash mid-run leaves a readable trail and [`RunJournal::resume_hint`]
//! reports the last state so the operator (or the fleet) can decide whether
//! to re-run. Appends are flushed per event; a torn final line is tolerated
//! on replay.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One journal event, tagged for JSONL replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum JournalEvent {
    /// Run started.
    Started {
        /// The delegated task text.
        task: String,
        /// Provider used.
        provider: String,
    },
    /// An LLM iteration began.
    Iteration {
        /// 1-based iteration number.
        n: usize,
    },
    /// A tool call was dispatched.
    ToolCall {
        /// Iteration number.
        n: usize,
        /// Tool name.
        name: String,
        /// JSON arguments as received.
        args: String,
    },
    /// A tool call completed.
    ToolResult {
        /// Iteration number.
        n: usize,
        /// Tool name.
        name: String,
        /// Whether the tool reported an error.
        is_error: bool,
    },
    /// Run ended.
    Finished {
        /// Whether the run completed successfully.
        ok: bool,
        /// Human-readable outcome.
        note: String,
    },
}

/// Append-only journal for one run.
pub struct RunJournal {
    writer: BufWriter<File>,
    path: PathBuf,
}

impl RunJournal {
    /// Open (create) a journal at `path`; parent dirs are created.
    ///
    /// # Errors
    ///
    /// IO failures creating the file propagate.
    pub fn open(path: &Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self {
            writer: BufWriter::new(file),
            path: path.to_path_buf(),
        })
    }

    /// Where this journal persists.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one event and flush.
    ///
    /// # Errors
    ///
    /// IO failures propagate.
    pub fn record(&mut self, event: &JournalEvent) -> std::io::Result<()> {
        let mut line = serde_json::to_string(event).map_err(std::io::Error::other)?;
        line.push('\n');
        self.writer.write_all(line.as_bytes())?;
        self.writer.flush()
    }

    /// Replay all well-formed events; a torn final line (crash mid-write)
    /// is skipped silently.
    ///
    /// # Errors
    ///
    /// IO failures reading the file propagate.
    pub fn replay(path: &Path) -> std::io::Result<Vec<JournalEvent>> {
        let body = std::fs::read_to_string(path)?;
        let mut events = Vec::new();
        for line in body.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(ev) = serde_json::from_str::<JournalEvent>(line) {
                events.push(ev);
            }
        }
        Ok(events)
    }

    /// Human-readable resume hint from the journal: what the run last did.
    ///
    /// `None` when the journal is empty or unreadable.
    #[must_use]
    pub fn resume_hint(path: &Path) -> Option<String> {
        let events = Self::replay(path).ok()?;
        let mut last_tool = String::new();
        let mut started = false;
        let mut finished_note = None;
        for ev in events {
            match ev {
                JournalEvent::Started { .. } => started = true,
                JournalEvent::ToolCall { name, .. } => last_tool = name,
                JournalEvent::Finished { ok, note } => {
                    finished_note = Some(format!("{} ({})", note, if ok { "ok" } else { "failed" }));
                },
                _ => {},
            }
        }
        if !started {
            return None;
        }
        Some(match finished_note {
            Some(note) => format!("finished: {note}"),
            None => format!("interrupted after tool '{last_tool}'"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("runs").join("auto-1.jsonl")
    }

    #[test]
    fn roundtrip_preserves_events() {
        let dir = tempfile::tempdir().unwrap();
        let path = tmp_path(&dir);
        let mut journal = RunJournal::open(&path).unwrap();
        journal
            .record(&JournalEvent::Started {
                task: "fix tests".into(),
                provider: "google".into(),
            })
            .unwrap();
        journal
            .record(&JournalEvent::ToolCall {
                n: 1,
                name: "shell".into(),
                args: r#"{"command":"cargo test"}"#.into(),
            })
            .unwrap();
        journal
            .record(&JournalEvent::Finished {
                ok: true,
                note: "tests green".into(),
            })
            .unwrap();
        drop(journal);

        let events = RunJournal::replay(&path).unwrap();
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], JournalEvent::Started { .. }));
        assert!(RunJournal::resume_hint(&path)
            .unwrap()
            .starts_with("finished: tests green"));
    }

    #[test]
    fn torn_final_line_is_tolerated() {
        let dir = tempfile::tempdir().unwrap();
        let path = tmp_path(&dir);
        {
            let mut journal = RunJournal::open(&path).unwrap();
            journal
                .record(&JournalEvent::Started {
                    task: "t".into(),
                    provider: "google".into(),
                })
                .unwrap();
            // Simulate a crash mid-write: append a torn line.
            let mut raw = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
            raw.write_all(br##"{"t":"tool_ca"##).unwrap();
        }
        let events = RunJournal::replay(&path).unwrap();
        assert_eq!(events.len(), 1);
        assert!(RunJournal::resume_hint(&path)
            .unwrap()
            .starts_with("interrupted"));
    }

    #[test]
    fn empty_journal_has_no_hint() {
        let dir = tempfile::tempdir().unwrap();
        assert!(RunJournal::resume_hint(&tmp_path(&dir)).is_none());
    }
}
