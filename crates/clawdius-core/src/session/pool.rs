//! `SQLite` connection pool for concurrent session access.
//!
//! Provides a thread-safe connection pool wrapper around `r2d2` + `r2d2_sqlite`
//! so that multiple tokio tasks can check out connections without blocking each
//! other or opening a new file handle per request.

use std::path::Path;

use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, OptionalExtension, Row};
use uuid::Uuid;

use super::repository::SessionRepository;
use super::types::{ContentPart, TokenUsage};
use super::{Message, MessageContent, MessageRole, Session, SessionId, SessionMeta};
use crate::error::Result;

pub type SessionPool = Pool<SqliteConnectionManager>;

/// Configuration for the connection pool.
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    /// Maximum number of connections in the pool.
    pub max_size: u32,
    /// Minimum number of idle connections to maintain.
    pub min_idle: u32,
    /// Connection acquisition timeout in seconds.
    pub connection_timeout_secs: u64,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_size: 32,
            min_idle: 4,
            connection_timeout_secs: 5,
        }
    }
}

/// Create a new connection pool backed by the `SQLite` database at `db_path`.
pub fn create_pool(db_path: &Path, config: &ConnectionPoolConfig) -> Result<SessionPool> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let manager = SqliteConnectionManager::file(db_path).with_init(|conn| {
        conn.execute_batch(
            r"
                CREATE TABLE IF NOT EXISTS sessions (
                    id TEXT PRIMARY KEY,
                    title TEXT,
                    provider TEXT,
                    model TEXT,
                    working_dir TEXT,
                    tags TEXT,
                    extra TEXT,
                    input_tokens INTEGER DEFAULT 0,
                    output_tokens INTEGER DEFAULT 0,
                    cached_tokens INTEGER DEFAULT 0,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS messages (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                    role TEXT NOT NULL CHECK(role IN ('system', 'user', 'assistant', 'tool')),
                    content TEXT NOT NULL,
                    tokens INTEGER,
                    tool_calls TEXT,
                    metadata TEXT,
                    created_at TEXT NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_messages_session
                ON messages(session_id, created_at);

                CREATE TABLE IF NOT EXISTS checkpoints (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                    message_id TEXT REFERENCES messages(id),
                    description TEXT,
                    workspace_snapshot BLOB,
                    created_at TEXT NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_checkpoints_session
                ON checkpoints(session_id, created_at DESC);
                ",
        )?;
        // Enable WAL mode for better concurrent read performance.
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        Ok(())
    });

    let pool = Pool::builder()
        .max_size(config.max_size)
        .min_idle(Some(config.min_idle))
        .connection_timeout(std::time::Duration::from_secs(
            config.connection_timeout_secs,
        ))
        .build(manager)
        .map_err(|e| crate::Error::Other(format!("Failed to create connection pool: {e}")))?;

    Ok(pool)
}

/// Create an in-memory connection pool (for testing).
pub fn create_in_memory_pool(config: &ConnectionPoolConfig) -> Result<SessionPool> {
    let manager = SqliteConnectionManager::memory().with_init(|conn| {
        conn.execute_batch(
            r"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                title TEXT,
                provider TEXT,
                model TEXT,
                working_dir TEXT,
                tags TEXT,
                extra TEXT,
                input_tokens INTEGER DEFAULT 0,
                output_tokens INTEGER DEFAULT 0,
                cached_tokens INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                role TEXT NOT NULL CHECK(role IN ('system', 'user', 'assistant', 'tool')),
                content TEXT NOT NULL,
                tokens INTEGER,
                tool_calls TEXT,
                metadata TEXT,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_messages_session
            ON messages(session_id, created_at);

            CREATE TABLE IF NOT EXISTS checkpoints (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                message_id TEXT REFERENCES messages(id),
                description TEXT,
                workspace_snapshot BLOB,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_checkpoints_session
            ON checkpoints(session_id, created_at DESC);
            ",
        )?;
        Ok(())
    });

    let pool = Pool::builder()
        .max_size(config.max_size)
        .min_idle(Some(config.min_idle))
        .connection_timeout(std::time::Duration::from_secs(
            config.connection_timeout_secs,
        ))
        .build(manager)
        .map_err(|e| crate::Error::Other(format!("Failed to create in-memory pool: {e}")))?;

    Ok(pool)
}

/// A session store backed by a connection pool.
///
/// Each operation checks out a connection from the pool and returns it
/// automatically when the operation completes (via `Drop`).
#[derive(Debug)]
pub struct PooledSessionStore {
    pool: SessionPool,
}

impl PooledSessionStore {
    /// Create a new pooled session store from an existing pool.
    #[must_use]
    pub const fn new(pool: SessionPool) -> Self {
        Self { pool }
    }

    /// Create a pooled session store backed by the database at `db_path`.
    pub fn open(db_path: &Path, config: &ConnectionPoolConfig) -> Result<Self> {
        let pool = create_pool(db_path, config)?;
        Ok(Self::new(pool))
    }

    /// Create an in-memory pooled session store (for testing).
    pub fn in_memory(config: &ConnectionPoolConfig) -> Result<Self> {
        let pool = create_in_memory_pool(config)?;
        Ok(Self::new(pool))
    }

    /// Get a reference to the underlying pool.
    #[must_use]
    pub const fn pool(&self) -> &SessionPool {
        &self.pool
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| crate::Error::Other(format!("Pool exhausted: {e}")))
    }

    /// Create a new session.
    pub fn create_session(&self, session: &Session) -> Result<()> {
        let conn = self.conn()?;
        let tags_json = serde_json::to_string(&session.meta.tags)?;
        let extra_json = serde_json::to_string(&session.meta.extra)?;

        conn.execute(
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

    /// Load a session by ID (without messages).
    pub fn load_session(&self, id: &SessionId) -> Result<Option<Session>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            r"
            SELECT id, title, provider, model, working_dir, tags, extra,
                   input_tokens, output_tokens, cached_tokens,
                   created_at, updated_at
            FROM sessions WHERE id = ?1
            ",
        )?;

        let session = stmt
            .query_row(params![id.to_string()], row_to_session)
            .optional()?;

        Ok(session)
    }

    /// Load a session with all messages.
    pub fn load_session_full(&self, id: &SessionId) -> Result<Option<Session>> {
        let conn = self.conn()?;
        let mut session = {
            let mut stmt = conn.prepare(
                r"
                SELECT id, title, provider, model, working_dir, tags, extra,
                       input_tokens, output_tokens, cached_tokens,
                       created_at, updated_at
                FROM sessions WHERE id = ?1
                ",
            )?;
            match stmt
                .query_row(params![id.to_string()], row_to_session)
                .optional()?
            {
                Some(s) => s,
                None => return Ok(None),
            }
        };

        let mut stmt = conn.prepare(
            r"
            SELECT id, session_id, role, content, tokens, tool_calls, metadata, created_at
            FROM messages WHERE session_id = ?1
            ORDER BY created_at ASC
            ",
        )?;

        let messages = stmt.query_map(params![id.to_string()], row_to_message)?;
        session.messages = messages.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(Some(session))
    }

    /// Save a message to a session.
    pub fn save_message(&self, session_id: &SessionId, message: &Message) -> Result<()> {
        let conn = self.conn()?;
        let content_json = match &message.content {
            MessageContent::Text(text) => serde_json::to_string(text)?,
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

        conn.execute(
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

        conn.execute(
            "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), session_id.to_string()],
        )?;

        Ok(())
    }

    /// Update session token usage.
    pub fn update_token_usage(&self, id: &SessionId, usage: &TokenUsage) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
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

    /// List all sessions (without messages).
    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            r"
            SELECT id, title, provider, model, working_dir, tags, extra,
                   input_tokens, output_tokens, cached_tokens,
                   created_at, updated_at
            FROM sessions
            ORDER BY updated_at DESC
            ",
        )?;

        let sessions = stmt
            .query_map([], row_to_session)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(sessions)
    }

    /// Delete a session.
    pub fn delete_session(&self, id: &SessionId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM sessions WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(())
    }

    /// Search messages by content.
    pub fn search_messages(&self, query: &str) -> Result<Vec<(SessionId, Message)>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
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
                let message = row_to_message(row)?;
                let session_id_str: String = row.get(1)?;
                let session_id = SessionId::from_uuid(
                    Uuid::parse_str(&session_id_str)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                );
                Ok((session_id, message))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(results)
    }
}

impl SessionRepository for PooledSessionStore {
    fn create_session(&self, session: &Session) -> Result<()> {
        self.create_session(session)
    }

    fn load_session(&self, id: &SessionId) -> Result<Option<Session>> {
        self.load_session(id)
    }

    fn load_session_full(&self, id: &SessionId) -> Result<Option<Session>> {
        self.load_session_full(id)
    }

    fn save_message(&self, session_id: &SessionId, message: &Message) -> Result<()> {
        self.save_message(session_id, message)
    }

    fn update_token_usage(&self, id: &SessionId, usage: &TokenUsage) -> Result<()> {
        self.update_token_usage(id, usage)
    }

    fn list_sessions(&self) -> Result<Vec<Session>> {
        self.list_sessions()
    }

    fn delete_session(&self, id: &SessionId) -> Result<()> {
        self.delete_session(id)
    }

    fn search_messages(&self, query: &str) -> Result<Vec<(SessionId, Message)>> {
        self.search_messages(query)
    }
}

fn row_to_session(row: &Row<'_>) -> std::result::Result<Session, rusqlite::Error> {
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
        id: SessionId::from_uuid(
            Uuid::parse_str(&id_str)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
        ),
        title,
        messages: Vec::new(),
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

fn row_to_message(row: &Row<'_>) -> std::result::Result<Message, rusqlite::Error> {
    let id_str: String = row.get(0)?;
    let role_str: String = row.get(2)?;
    let content_json: String = row.get(3)?;
    let tokens: Option<i64> = row.get(4)?;
    let tool_calls_json: Option<String> = row.get(5)?;
    let metadata_json: Option<String> = row.get(6)?;
    let created_at_str: String = row.get(7)?;

    let content = if content_json.starts_with('"') {
        MessageContent::Text(serde_json::from_str(&content_json).unwrap_or_default())
    } else if content_json.starts_with('[') {
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
        id: Uuid::parse_str(&id_str)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
        role: MessageRole::parse_role(&role_str),
        content,
        tokens: tokens.map(|t| t as usize),
        created_at: DateTime::parse_from_rfc3339(&created_at_str)
            .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
        tool_calls,
        metadata,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn test_config() -> ConnectionPoolConfig {
        ConnectionPoolConfig {
            max_size: 4,
            min_idle: 1,
            connection_timeout_secs: 5,
        }
    }

    #[test]
    fn test_pool_creation_and_checkout() {
        let config = test_config();
        let pool = create_in_memory_pool(&config).expect("pool creation should succeed");
        let _conn = pool.get().expect("should get a connection from pool");
    }

    #[test]
    fn test_pool_exhaustion_returns_error() {
        let config = ConnectionPoolConfig {
            max_size: 1,
            min_idle: 0,
            connection_timeout_secs: 1,
        };
        let pool = create_in_memory_pool(&config).expect("pool creation should succeed");

        // Check out the only connection
        let _conn1 = pool.get().expect("first checkout should succeed");

        // Set a very short timeout so the second get() fails quickly
        let pool2 = Pool::builder()
            .max_size(1)
            .connection_timeout(std::time::Duration::from_millis(50))
            .build(SqliteConnectionManager::memory().with_init(|conn| {
                conn.execute_batch("CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY);")?;
                Ok(())
            }))
            .expect("pool build should succeed");

        let _c1 = pool2.get().expect("first checkout should succeed");
        let result = pool2.get();
        assert!(
            result.is_err(),
            "second checkout should fail when pool is exhausted"
        );
    }

    #[test]
    fn test_pooled_session_crud() -> Result<()> {
        let config = test_config();
        let store = PooledSessionStore::in_memory(&config)?;

        let mut session = Session::new();
        session.title = Some("Pooled Test Session".to_string());
        session.meta.provider = Some("anthropic".to_string());
        session.meta.model = Some("claude-3-5-sonnet".to_string());

        store.create_session(&session)?;

        let loaded = store
            .load_session(&session.id)?
            .expect("session should exist");
        assert_eq!(loaded.title, Some("Pooled Test Session".to_string()));

        let msg = Message::user("Hello from pool!");
        store.save_message(&session.id, &msg)?;

        let full = store
            .load_session_full(&session.id)?
            .expect("session should exist");
        assert_eq!(full.messages.len(), 1);
        assert_eq!(full.messages[0].as_text(), Some("Hello from pool!"));

        let sessions = store.list_sessions()?;
        assert_eq!(sessions.len(), 1);

        store.delete_session(&session.id)?;
        let sessions = store.list_sessions()?;
        assert!(sessions.is_empty());

        Ok(())
    }

    #[test]
    fn test_concurrent_session_access() -> Result<()> {
        let config = ConnectionPoolConfig {
            max_size: 8,
            min_idle: 2,
            connection_timeout_secs: 5,
        };
        let store = PooledSessionStore::in_memory(&config)?;

        let mut session = Session::new();
        session.title = Some("Concurrent Test".to_string());
        store.create_session(&session)?;

        std::thread::scope(|s| {
            let handles: Vec<_> = (0..4)
                .map(|i| {
                    let store_ref = &store;
                    let sid = session.id;
                    s.spawn(move || {
                        let msg = Message::user(format!("Message {i}"));
                        store_ref.save_message(&sid, &msg).unwrap();
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });

        let full = store
            .load_session_full(&session.id)?
            .expect("session should exist");
        assert_eq!(full.messages.len(), 4);

        Ok(())
    }

    #[test]
    fn test_pooled_store_file_based() -> Result<()> {
        let temp = NamedTempFile::new()?;
        let config = test_config();
        let store = PooledSessionStore::open(temp.path(), &config)?;

        let session = Session::new();
        store.create_session(&session)?;

        let loaded = store
            .load_session(&session.id)?
            .expect("session should exist");
        assert_eq!(loaded.id, session.id);

        Ok(())
    }

    #[test]
    fn test_tenant_isolation() -> Result<()> {
        let config = test_config();
        let store = PooledSessionStore::in_memory(&config)?;

        let mut session_a = Session::new();
        session_a.title = Some("Tenant A Session".to_string());
        session_a.meta.extra.insert(
            "tenant_id".to_string(),
            serde_json::Value::String("tenant-a".to_string()),
        );

        let mut session_b = Session::new();
        session_b.title = Some("Tenant B Session".to_string());
        session_b.meta.extra.insert(
            "tenant_id".to_string(),
            serde_json::Value::String("tenant-b".to_string()),
        );

        store.create_session(&session_a)?;
        store.create_session(&session_b)?;

        let sessions = store.list_sessions()?;
        assert_eq!(sessions.len(), 2);

        let loaded_a = store
            .load_session(&session_a.id)?
            .expect("session a should exist");
        assert_eq!(
            loaded_a
                .meta
                .extra
                .get("tenant_id")
                .and_then(|v| v.as_str()),
            Some("tenant-a")
        );

        let loaded_b = store
            .load_session(&session_b.id)?
            .expect("session b should exist");
        assert_eq!(
            loaded_b
                .meta
                .extra
                .get("tenant_id")
                .and_then(|v| v.as_str()),
            Some("tenant-b")
        );

        store.delete_session(&session_a.id)?;
        let sessions = store.list_sessions()?;
        assert_eq!(sessions.len(), 1);
        assert_eq!(
            sessions[0]
                .meta
                .extra
                .get("tenant_id")
                .and_then(|v| v.as_str()),
            Some("tenant-b")
        );

        Ok(())
    }
}
