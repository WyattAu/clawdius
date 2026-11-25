//! Durable chat state for the gateway handler.
//!
//! The handler keeps per-chat session mappings and message history in
//! memory; without persistence every restart (deploy, babysitter blip,
//! crash) amnesiacs every conversation. This module serializes both maps
//! to one JSON file and restores them at startup — write-through on every
//! mutation, load-once at construction.
//!
//! Format: `{ "sessions": { chat_key: session_id }, "history": { chat_key:
//! [ChatMessage…] } }`. A missing file starts empty; a corrupt file starts
//! empty too (chat state is precious but not worth a boot failure).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::adapter::Platform;

/// One persisted chat: the clawdius session id and its message history.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ChatStateFile {
    /// `"<platform>:<chat_id>"` → clawdius session id.
    #[serde(default)]
    pub sessions: HashMap<String, String>,
    /// Same key → conversation history for the LLM context window.
    #[serde(default)]
    pub history: HashMap<String, Vec<clawdius_core::llm::ChatMessage>>,
}

impl ChatStateFile {
    /// Load from `path`; missing or corrupt file → empty state.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Write-through save (atomic replace to avoid torn reads).
    pub fn save(&self, path: &Path) {
        if let Ok(json) = serde_json::to_vec(self) {
            let tmp = path.with_extension("tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, path);
            }
        }
    }

    /// The persisted key for one chat.
    #[must_use]
    pub fn key(platform: &Platform, chat_id: &str) -> String {
        format!("{}:{}", platform.as_str(), chat_id)
    }
}

/// The handler-side wrapper: path + the loaded maps, mirroring the
/// handler's in-memory types.
#[derive(Debug, Default)]
pub struct ChatPersist {
    path: Option<PathBuf>,
    pub sessions: HashMap<String, String>,
    pub history: HashMap<String, Vec<clawdius_core::llm::ChatMessage>>,
}

impl ChatPersist {
    /// Load from `path` (None = persistence disabled).
    #[must_use]
    pub fn load(path: Option<PathBuf>) -> Self {
        let Some(path) = path else {
            return Self::default();
        };
        let file = ChatStateFile::load(&path);
        Self {
            path: Some(path),
            sessions: file.sessions,
            history: file.history,
        }
    }

    /// Write the current maps to disk (no-op when persistence is off).
    pub fn save(&self) {
        if let Some(path) = &self.path {
            ChatStateFile {
                sessions: self.sessions.clone(),
                history: self.history.clone(),
            }
            .save(path);
        }
    }

    /// Record one session mapping.
    pub fn insert_session(&mut self, key: String, session_id: String) {
        self.sessions.insert(key, session_id);
        self.save();
    }

    /// Replace one chat's history.
    pub fn set_history(&mut self, key: String, history: Vec<clawdius_core::llm::ChatMessage>) {
        self.history.insert(key, history);
        self.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clawdius_core::llm::{ChatMessage, ChatRole};

    fn tmp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "gw-chat-state-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    #[test]
    fn round_trip_preserves_sessions_and_history() {
        let path = tmp_path("roundtrip");
        {
            let mut p = ChatPersist::load(Some(path.clone()));
            p.insert_session("telegram:42".into(), "ses_1".into());
            p.set_history(
                "telegram:42".into(),
                vec![
                    ChatMessage {
                        role: ChatRole::User,
                        content: "fleet status".into(),
                    },
                    ChatMessage {
                        role: ChatRole::Assistant,
                        content: "lanes: idle".into(),
                    },
                ],
            );
        }
        let reopened = ChatPersist::load(Some(path.clone()));
        assert_eq!(
            reopened.sessions.get("telegram:42").map(String::as_str),
            Some("ses_1")
        );
        let h = reopened.history.get("telegram:42").expect("history");
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].content, "fleet status");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn corrupt_file_starts_empty() {
        let path = tmp_path("corrupt");
        std::fs::write(&path, b"{not json").unwrap();
        let mut p = ChatPersist::load(Some(path.clone()));
        assert!(p.sessions.is_empty());
        assert!(p.history.is_empty());
        // And a save over the corrupt file heals it.
        p.insert_session("telegram:1".into(), "ses_x".into());
        assert!(ChatStateFile::load(&path)
            .sessions
            .contains_key("telegram:1"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn key_format_matches_handler_session_key() {
        assert_eq!(ChatStateFile::key(&Platform::Telegram, "42"), "telegram:42");
    }

    #[test]
    fn none_path_is_a_noop() {
        let mut p = ChatPersist::load(None);
        p.insert_session("k".into(), "v".into());
        p.set_history("k".into(), vec![]);
        assert_eq!(p.sessions.len(), 1);
    }
}
