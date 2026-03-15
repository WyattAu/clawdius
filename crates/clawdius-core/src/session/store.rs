//! Session persistence using `SQLite`

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;
use uuid::Uuid;

use super::types::{ContentPart, TokenUsage};
use super::{Message, MessageContent, MessageRole, Session, SessionId, SessionMeta};
use crate::error::Result;

/// Session storage backend
pub struct SessionStore {
    conn: Connection,
}

impl SessionStore {
    /// Open or create session store at path
    pub fn open(path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.initialize()?;
        Ok(store)
    }

    /// Open in-memory store (for testing)
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.initialize()?;
        Ok(store)
    }

    /// Initialize database schema
    fn initialize(&self) -> Result<()> {
        self.conn.execute_batch(
            r"
            -- Sessions table
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                title TEXT,
                provider TEXT,
                model TEXT,
                working_dir TEXT,
                tags TEXT, -- JSON array
                extra TEXT, -- JSON object
                input_tokens INTEGER DEFAULT 0,
                output_tokens INTEGER DEFAULT 0,
                cached_tokens INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            
            -- Messages table
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                role TEXT NOT NULL CHECK(role IN ('system', 'user', 'assistant', 'tool')),
                content TEXT NOT NULL,
                tokens INTEGER,
                tool_calls TEXT, -- JSON array
                metadata TEXT, -- JSON object
                created_at TEXT NOT NULL
            );
            
            -- Index for faster session message lookups
            CREATE INDEX IF NOT EXISTS idx_messages_session 
            ON messages(session_id, created_at);
            
            -- Checkpoints table
            CREATE TABLE IF NOT EXISTS checkpoints (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                message_id TEXT REFERENCES messages(id),
                description TEXT,
                workspace_snapshot BLOB,
                created_at TEXT NOT NULL
            );
            
            -- Index for checkpoint lookups
            CREATE INDEX IF NOT EXISTS idx_checkpoints_session 
            ON checkpoints(session_id, created_at DESC);
            ",
        )?;
        Ok(())
    }

    /// Create a new session
    pub fn create_session(&self, session: &Session) -> Result<()> {
        let tags_json = serde_json::to_string(&session.meta.tags)?;
        let extra_json = serde_json::to_string(&session.meta.extra)?;

        self.conn.execute(
            r"
            INSERT INTO sessions (
                id, title, provider, model, working_dir, tags, extra,
                input_tokens, output_tokens, cached_tokens,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ",
            params![
                session.id.to_string(),
                session.title,
                session.meta.provider,
                session.meta.model,
                session.meta.working_dir,
                tags_json,
                extra_json,
                session.token_usage.input as i64,
                session.token_usage.output as i64,
                session.token_usage.cached as i64,
                session.created_at.to_rfc3339(),
                session.updated_at.to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    /// Load a session by ID
    pub fn load_session(&self, id: &SessionId) -> Result<Option<Session>> {
        let mut stmt = self.conn.prepare(
            r"
            SELECT id, title, provider, model, working_dir, tags, extra,
                   input_tokens, output_tokens, cached_tokens,
                   created_at, updated_at
            FROM sessions WHERE id = ?1
            ",
        )?;

        let session = stmt
            .query_row(params![id.to_string()], |row| self.row_to_session(row))
            .optional()?;

        Ok(session)
    }

    /// Load a session with all messages
    pub fn load_session_full(&self, id: &SessionId) -> Result<Option<Session>> {
        let Some(mut session) = self.load_session(id)? else {
            return Ok(None);
        };

        // Load messages
        let mut stmt = self.conn.prepare(
            r"
            SELECT id, session_id, role, content, tokens, tool_calls, metadata, created_at
            FROM messages WHERE session_id = ?1
            ORDER BY created_at ASC
            ",
        )?;

        let messages = stmt.query_map(params![id.to_string()], |row| self.row_to_message(row))?;

        session.messages = messages.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(Some(session))
    }

    /// Save a message to a session
    pub fn save_message(&self, session_id: &SessionId, message: &Message) -> Result<()> {
        let content_json = match &message.content {
            MessageContent::Text(text) => serde_json::to_string(&text)?,
            MessageContent::MultiPart(parts) => serde_json::to_string(parts)?,
        };
        let tool_calls_json = if message.tool_calls.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&message.tool_calls)?)
        };
        let metadata_json = if message.metadata.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&message.metadata)?)
        };

        self.conn.execute(
            r"
            INSERT INTO messages (
                id, session_id, role, content, tokens, tool_calls, metadata, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                message.id.to_string(),
                session_id.to_string(),
                message.role.as_str(),
                content_json,
                message.tokens.map(|t| t as i64),
                tool_calls_json,
                metadata_json,
                message.created_at.to_rfc3339(),
            ],
        )?;

        // Update session timestamp
        self.conn.execute(
            "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), session_id.to_string()],
        )?;

        Ok(())
    }

    /// Update session token usage
    pub fn update_token_usage(&self, id: &SessionId, usage: &TokenUsage) -> Result<()> {
        self.conn.execute(
            r"
            UPDATE sessions SET 
                input_tokens = ?1,
                output_tokens = ?2,
                cached_tokens = ?3,
                updated_at = ?4
            WHERE id = ?5
            ",
            params![
                usage.input as i64,
                usage.output as i64,
                usage.cached as i64,
                Utc::now().to_rfc3339(),
                id.to_string(),
            ],
        )?;

        Ok(())
    }

    /// List all sessions (without messages)
    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        let mut stmt = self.conn.prepare(
            r"
            SELECT id, title, provider, model, working_dir, tags, extra,
                   input_tokens, output_tokens, cached_tokens,
                   created_at, updated_at
            FROM sessions
            ORDER BY updated_at DESC
            ",
        )?;

        let sessions = stmt
            .query_map([], |row| self.row_to_session(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(sessions)
    }

    /// Delete a session
    pub fn delete_session(&self, id: &SessionId) -> Result<()> {
        self.conn.execute(
            "DELETE FROM sessions WHERE id = ?1",
            params![id.to_string()],
        )?;

        Ok(())
    }

    /// Search messages by content
    pub fn search_messages(&self, query: &str) -> Result<Vec<(SessionId, Message)>> {
        let mut stmt = self.conn.prepare(
            r"
            SELECT m.id, m.session_id, m.role, m.content, m.tokens, m.tool_calls, m.metadata, m.created_at
            FROM messages m
            WHERE m.content LIKE ?1
            ORDER BY m.created_at DESC
            LIMIT 100
            ",
        )?;

        let pattern = format!("%{query}%");
        let results = stmt
            .query_map(params![pattern], |row| {
                let message = self.row_to_message(row)?;
                let session_id_str: String = row.get(1)?;
                let session_id = SessionId::from_uuid(Uuid::parse_str(&session_id_str).unwrap());
                Ok((session_id, message))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(results)
    }

    /// Convert row to Session
    fn row_to_session(&self, row: &Row<'_>) -> std::result::Result<Session, rusqlite::Error> {
        let id_str: String = row.get(0)?;
        let title: Option<String> = row.get(1)?;
        let provider: Option<String> = row.get(2)?;
        let model: Option<String> = row.get(3)?;
        let working_dir: Option<String> = row.get(4)?;
        let tags_json: String = row.get(5)?;
        let extra_json: String = row.get(6)?;
        let input_tokens: i64 = row.get(7)?;
        let output_tokens: i64 = row.get(8)?;
        let cached_tokens: i64 = row.get(9)?;
        let created_at_str: String = row.get(10)?;
        let updated_at_str: String = row.get(11)?;

        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        let extra: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&extra_json).unwrap_or_default();

        Ok(Session {
            id: SessionId::from_uuid(Uuid::parse_str(&id_str).unwrap()),
            title,
            messages: Vec::new(), // Loaded separately
            meta: SessionMeta {
                provider,
                model,
                working_dir,
                tags,
                extra,
            },
            token_usage: TokenUsage {
                input: input_tokens as usize,
                output: output_tokens as usize,
                cached: cached_tokens as usize,
            },
            created_at: DateTime::parse_from_rfc3339(&created_at_str)
                .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
        })
    }

    /// Convert row to Message
    fn row_to_message(&self, row: &Row<'_>) -> std::result::Result<Message, rusqlite::Error> {
        let id_str: String = row.get(0)?;
        let role_str: String = row.get(2)?;
        let content_json: String = row.get(3)?;
        let tokens: Option<i64> = row.get(4)?;
        let tool_calls_json: Option<String> = row.get(5)?;
        let metadata_json: Option<String> = row.get(6)?;
        let created_at_str: String = row.get(7)?;

        // Parse content - could be string or multipart
        let content = if content_json.starts_with('"') {
            // Plain string
            MessageContent::Text(serde_json::from_str(&content_json).unwrap_or_default())
        } else if content_json.starts_with('[') {
            // Multi-part
            let parts: Vec<ContentPart> = serde_json::from_str(&content_json).unwrap_or_default();
            MessageContent::MultiPart(parts)
        } else {
            MessageContent::Text(content_json)
        };

        let tool_calls = tool_calls_json
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();

        let metadata = metadata_json
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();

        Ok(Message {
            id: Uuid::parse_str(&id_str).unwrap(),
            role: MessageRole::parse_role(&role_str),
            content,
            tokens: tokens.map(|t| t as usize),
            created_at: DateTime::parse_from_rfc3339(&created_at_str)
                .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
            tool_calls,
            metadata,
        })
    }
}

impl MessageRole {
    /// Convert to string for database
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }

    /// Parse from string
    #[must_use]
    pub fn parse_role(s: &str) -> Self {
        match s {
            "system" => Self::System,
            "user" => Self::User,
            "assistant" => Self::Assistant,
            "tool" => Self::Tool,
            _ => Self::User,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_session_store_crud() -> Result<()> {
        let temp = NamedTempFile::new()?;
        let store = SessionStore::open(temp.path())?;

        // Create session
        let mut session = Session::new();
        session.title = Some("Test Session".to_string());
        session.meta.provider = Some("anthropic".to_string());
        session.meta.model = Some("claude-3-5-sonnet".to_string());

        store.create_session(&session)?;

        // Load session
        let loaded = store
            .load_session(&session.id)?
            .expect("session should exist");
        assert_eq!(loaded.title, Some("Test Session".to_string()));

        // Add message
        let msg = Message::user("Hello, world!");
        store.save_message(&session.id, &msg)?;

        // Load full session
        let full = store
            .load_session_full(&session.id)?
            .expect("session should exist");
        assert_eq!(full.messages.len(), 1);
        assert_eq!(full.messages[0].as_text(), Some("Hello, world!"));

        // List sessions
        let sessions = store.list_sessions()?;
        assert_eq!(sessions.len(), 1);

        // Delete session
        store.delete_session(&session.id)?;
        let sessions = store.list_sessions()?;
        assert!(sessions.is_empty());

        Ok(())
    }
}
