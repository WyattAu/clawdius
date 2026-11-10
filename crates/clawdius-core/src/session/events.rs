//! Session event log: crash-safe, append-only JSONL trail of
//! conversation messages, independent of the SQLite store.
//!
//! opencode-parity (durable sessions): every user/assistant message is
//! appended to `.clawdius/sessions/<id>/events.jsonl` best-effort at the
//! moment it happens. If SQLite is corrupted or lost, the JSONL replays
//! into a rebuilt session. Writes are flushed per event; a torn final
//! line (crash mid-write) is skipped on replay.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::session::{Message, MessageRole};

/// One logged conversation message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEvent {
    /// Role: `user`, `assistant`, `system`, or `tool`.
    pub role: String,
    /// Text content (multi-part messages render their text part).
    pub content: String,
    /// RFC 3339 timestamp at capture.
    pub timestamp: String,
}

fn role_name(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}

/// Default path for a session id: `.clawdius/sessions/<id>/events.jsonl`.
#[must_use]
pub fn default_path(session_id: &str, base: &Path) -> PathBuf {
    base.join(".clawdius")
        .join("sessions")
        .join(session_id)
        .join("events.jsonl")
}

/// Append-only event log for one session.
pub struct SessionEventLog {
    writer: BufWriter<File>,
    path: PathBuf,
}

impl SessionEventLog {
    /// Open (create) the log; parent dirs are created.
    ///
    /// # Errors
    ///
    /// IO failures propagate.
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

    /// Where this log persists.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one message event and flush.
    ///
    /// # Errors
    ///
    /// IO/serialization failures propagate.
    pub fn append_message(&mut self, message: &Message) -> std::io::Result<()> {
        let event = SessionEvent {
            role: role_name(message.role).to_string(),
            content: message.as_text().unwrap_or_default().to_string(),
            timestamp: message.created_at.to_rfc3339(),
        };
        let mut line = serde_json::to_string(&event).map_err(std::io::Error::other)?;
        line.push('\n');
        self.writer.write_all(line.as_bytes())?;
        self.writer.flush()
    }

    /// Replay all well-formed events; a torn final line is skipped.
    ///
    /// # Errors
    ///
    /// IO failures reading the file propagate.
    pub fn replay(path: &Path) -> std::io::Result<Vec<SessionEvent>> {
        let Ok(file) = File::open(path) else {
            return Ok(Vec::new());
        };
        let mut events = Vec::new();
        for line in BufReader::new(file).lines() {
            let Ok(line) = line else { continue };
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(ev) = serde_json::from_str::<SessionEvent>(&line) {
                events.push(ev);
            }
        }
        Ok(events)
    }
}

/// Message-role to stable role-name mapping (test-visible).
#[cfg(test)]
pub(crate) fn role_name_test(role: MessageRole) -> &'static str {
    role_name(role)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(role: MessageRole, text: &str) -> Message {
        let mut m = match role {
            MessageRole::Assistant => Message::assistant(text),
            _ => Message::user(text),
        };
        m.role = role;
        m
    }

    #[test]
    fn append_replay_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = default_path("s1", dir.path());
        let mut log = SessionEventLog::open(&path).unwrap();
        log.append_message(&sample(MessageRole::User, "fix the tests"))
            .unwrap();
        log.append_message(&sample(MessageRole::Assistant, "done, 2 green"))
            .unwrap();
        drop(log);

        let events = SessionEventLog::replay(&path).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].role, "user");
        assert_eq!(events[0].content, "fix the tests");
        assert_eq!(events[1].role, "assistant");
    }

    #[test]
    fn torn_final_line_is_tolerated() {
        let dir = tempfile::tempdir().unwrap();
        let path = default_path("s2", dir.path());
        {
            let mut log = SessionEventLog::open(&path).unwrap();
            log.append_message(&sample(MessageRole::User, "hello"))
                .unwrap();
        }
        // Simulate a crash mid-write.
        let mut raw = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        raw.write_all(br##"{"role":"user","content":"cut o"##)
            .unwrap();
        drop(raw);

        let events = SessionEventLog::replay(&path).unwrap();
        assert_eq!(events.len(), 1, "torn line skipped");
        assert_eq!(events[0].content, "hello");
    }

    #[test]
    fn default_path_layout() {
        let p = default_path("abc-123", Path::new("/repo"));
        assert!(p
            .to_string_lossy()
            .contains(".clawdius/sessions/abc-123/events.jsonl"));
    }
}
