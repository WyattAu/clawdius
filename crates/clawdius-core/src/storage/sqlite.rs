//! SQLite storage backend implementation.
//!
//! Implements all three domain traits (`SessionRepository`, `TimelineRepository`,
//! `GraphRepository`) and the unified `StorageBackend` trait using `rusqlite`.
//!
//! The session operations are fully ported from the legacy `SessionStore`.
//! Timeline and graph operations are delegated to the existing store types
//! (`TimelineStore`, `GraphStore`) until they are independently migrated.

use super::backend::{
    GraphRepository, SessionRepository, StorageBackend, TimelineRepository,
};
use super::error::StorageError;
use crate::error::Result;
use crate::graph_rag::ast::{
    FileInfo, Reference, Relationship, Symbol, SymbolKind,
};
use crate::session::types::{
    ContentPart, Message, MessageContent, MessageRole, Session, SessionId,
    SessionMeta, TokenUsage,
};
use crate::timeline::{
    CheckpointId, CheckpointInfo, Diff, DiffSummary, ExportedCheckpoint,
    ExportedFile, FileChangeType, FileDiff, FileVersion, RollbackPreview, StorageStats,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use uuid::Uuid;

// ─────────────────────────────────────────────────────────
// SQL Schema (unified — sessions + timeline + graph)
// ─────────────────────────────────────────────────────────

/// Schema version for the unified storage database.
const SCHEMA_VERSION: i32 = 1;

/// SQL statements to initialize the unified schema.
/// Combines session schema from `session/store.rs` with workspace extensions.
const INIT_SQL: &str = r"
-- Schema version tracking
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY
);

-- Sessions table
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    title TEXT,
    provider TEXT,
    model TEXT,
    working_dir TEXT,
    tags TEXT NOT NULL DEFAULT '[]',
    extra TEXT NOT NULL DEFAULT '{}',
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cached_tokens INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Messages table
CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    tokens INTEGER,
    tool_calls TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL
);

-- Indexes for sessions
CREATE INDEX IF NOT EXISTS idx_sessions_updated_at ON sessions(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_messages_session_id ON messages(session_id);
CREATE INDEX IF NOT EXISTS idx_messages_content ON messages(content);

-- Timeline: tracked files
CREATE TABLE IF NOT EXISTS tracked_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT UNIQUE NOT NULL,
    tracked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Timeline: checkpoints
CREATE TABLE IF NOT EXISTS checkpoints (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    timestamp TEXT NOT NULL,
    files_count INTEGER NOT NULL DEFAULT 0,
    total_size INTEGER NOT NULL DEFAULT 0
);

-- Timeline: file versions
CREATE TABLE IF NOT EXISTS file_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    checkpoint_id TEXT NOT NULL REFERENCES checkpoints(id) ON DELETE CASCADE,
    checksum TEXT NOT NULL,
    size INTEGER NOT NULL DEFAULT 0,
    timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_file_versions_path ON file_versions(path);
CREATE INDEX IF NOT EXISTS idx_file_versions_checkpoint ON file_versions(checkpoint_id);
CREATE INDEX IF NOT EXISTS idx_checkpoints_timestamp ON checkpoints(timestamp DESC);

-- Workspace: projects (for multi-repo support)
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Workspace: workspaces (grouping of projects)
CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Workspace: workspace-project membership
CREATE TABLE IF NOT EXISTS workspace_projects (
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (workspace_id, project_id)
);

-- Graph: files (for code knowledge graph)
CREATE TABLE IF NOT EXISTS graph_files (
    id INTEGER PRIMARY KEY,
    path TEXT UNIQUE NOT NULL,
    hash TEXT NOT NULL,
    language TEXT,
    last_modified TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Graph: symbols
CREATE TABLE IF NOT EXISTS graph_symbols (
    id INTEGER PRIMARY KEY,
    file_id INTEGER REFERENCES graph_files(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    signature TEXT,
    doc_comment TEXT,
    start_line INTEGER,
    end_line INTEGER,
    start_col INTEGER,
    end_col INTEGER,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Graph: symbol references
CREATE TABLE IF NOT EXISTS graph_symbol_refs (
    id INTEGER PRIMARY KEY,
    symbol_id INTEGER REFERENCES graph_symbols(id) ON DELETE CASCADE,
    file_id INTEGER REFERENCES graph_files(id) ON DELETE CASCADE,
    line INTEGER,
    col INTEGER,
    context TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Graph: relationships
CREATE TABLE IF NOT EXISTS graph_relationships (
    id INTEGER PRIMARY KEY,
    from_symbol INTEGER REFERENCES graph_symbols(id) ON DELETE CASCADE,
    to_symbol INTEGER REFERENCES graph_symbols(id) ON DELETE CASCADE,
    relationship_type TEXT NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Graph indexes
CREATE INDEX IF NOT EXISTS idx_graph_symbols_name ON graph_symbols(name);
CREATE INDEX IF NOT EXISTS idx_graph_symbols_kind ON graph_symbols(kind);
CREATE INDEX IF NOT EXISTS idx_graph_symbols_file ON graph_symbols(file_id);
CREATE INDEX IF NOT EXISTS idx_graph_refs_symbol ON graph_symbol_refs(symbol_id);
CREATE INDEX IF NOT EXISTS idx_graph_refs_file ON graph_symbol_refs(file_id);
CREATE INDEX IF NOT EXISTS idx_graph_rels_from ON graph_relationships(from_symbol);
CREATE INDEX IF NOT EXISTS idx_graph_rels_to ON graph_relationships(to_symbol);
CREATE INDEX IF NOT EXISTS idx_graph_rels_type ON graph_relationships(relationship_type);
";

// ─────────────────────────────────────────────────────────
// SqliteBackend
// ─────────────────────────────────────────────────────────

/// SQLite-backed storage implementing all domain traits.
///
/// Thread-safe via `std::sync::Mutex` wrapping `rusqlite::Connection`
/// (which is `!Sync`). This mirrors the existing `MutexRepository<R>`
/// pattern used by `session::repository`.
#[derive(Debug)]
pub struct SqliteBackend {
    /// Database connection, wrapped in Mutex for thread safety.
    conn: std::sync::Mutex<Connection>,
    /// Path to the database file (for metadata).
    path: PathBuf,
}

impl SqliteBackend {
    /// Open a SQLite database at the given path, creating it if needed.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| StorageError::Connection(e.to_string()))?;
        let backend = Self {
            conn: std::sync::Mutex::new(conn),
            path: path.to_path_buf(),
        };
        Ok(backend)
    }

    /// Create an in-memory SQLite database.
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| StorageError::Connection(e.to_string()))?;
        let backend = Self {
            conn: std::sync::Mutex::new(conn),
            path: PathBuf::from(":memory:"),
        };
        Ok(backend)
    }

    /// Get a handle to the connection for direct SQL access (advanced use).
    ///
    /// Prefer using trait methods. This is exposed for migrations and
    /// legacy code that needs raw SQL.
    pub fn with_conn<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let guard = self
            .conn
            .lock()
            .map_err(|e| StorageError::Connection(format!("lock poisoned: {e}")))?;
        f(&guard)
    }

    /// Run initialization SQL (schema creation).
    fn initialize(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute_batch(INIT_SQL)
                .map_err(|e| StorageError::Migration {
                    reason: e.to_string(),
                })?;
            // Record schema version
            conn.execute(
                "INSERT OR REPLACE INTO schema_version (version) VALUES (?1)",
                params![SCHEMA_VERSION],
            )
            .map_err(|e| StorageError::Migration {
                reason: e.to_string(),
            })?;
            Ok(())
        })
    }

    // ── Row mapping helpers (from session/store.rs) ──

    fn row_to_session(row: &rusqlite::Row<'_>) -> std::result::Result<Session, rusqlite::Error> {
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

    fn row_to_message(row: &rusqlite::Row<'_>) -> std::result::Result<Message, rusqlite::Error> {
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
}

// ─────────────────────────────────────────────────────────
// SessionRepository implementation
// ─────────────────────────────────────────────────────────

impl SessionRepository for SqliteBackend {
    fn create_session(&self, session: &Session) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let tags_json = serde_json::to_string(&session.meta.tags)?;
            let extra_json = serde_json::to_string(&session.meta.extra)?;

            self.with_conn(|conn| {
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
                )
                .map_err(|e| StorageError::Query {
                    statement: "INSERT INTO sessions".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn load_session(&self, id: &SessionId) -> impl std::future::Future<Output = Result<Option<Session>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, title, provider, model, working_dir, tags, extra,
                           input_tokens, output_tokens, cached_tokens,
                           created_at, updated_at
                    FROM sessions WHERE id = ?1
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT session".to_string(),
                        reason: e.to_string(),
                    })?;

                let session = stmt
                    .query_row(params![id.to_string()], |row| Self::row_to_session(row))
                    .optional()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT session".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(session)
            })
        }
    }

    fn load_session_full(&self, id: &SessionId) -> impl std::future::Future<Output = Result<Option<Session>>> + Send {
        async move {
            let Some(mut session) = self.load_session(id).await? else {
                return Ok(None);
            };

            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, session_id, role, content, tokens, tool_calls, metadata, created_at
                    FROM messages WHERE session_id = ?1
                    ORDER BY created_at ASC
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT messages".to_string(),
                        reason: e.to_string(),
                    })?;

                let messages = stmt
                    .query_map(params![id.to_string()], |row| Self::row_to_message(row))
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT messages".to_string(),
                        reason: e.to_string(),
                    })?;

                session.messages = messages.collect::<std::result::Result<Vec<_>, _>>().map_err(
                    |e| StorageError::Query {
                        statement: "SELECT messages".to_string(),
                        reason: e.to_string(),
                    },
                )?;

                Ok(session)
            })
            .map(Some)
        }
    }

    fn list_sessions(&self) -> impl std::future::Future<Output = Result<Vec<Session>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, title, provider, model, working_dir, tags, extra,
                           input_tokens, output_tokens, cached_tokens,
                           created_at, updated_at
                    FROM sessions
                    ORDER BY updated_at DESC
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT sessions".to_string(),
                        reason: e.to_string(),
                    })?;

                let sessions = stmt
                    .query_map([], |row| Self::row_to_session(row))
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT sessions".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT sessions".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(sessions)
            })
        }
    }

    fn delete_session(&self, id: &SessionId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    "DELETE FROM sessions WHERE id = ?1",
                    params![id.to_string()],
                )
                .map_err(|e| StorageError::Query {
                    statement: "DELETE session".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn save_message(&self, session_id: &SessionId, message: &Message) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
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

            self.with_conn(|conn| {
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
                )
                .map_err(|e| StorageError::Query {
                    statement: "INSERT message".to_string(),
                    reason: e.to_string(),
                })?;

                // Update session timestamp
                conn.execute(
                    "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
                    params![Utc::now().to_rfc3339(), session_id.to_string()],
                )
                .map_err(|e| StorageError::Query {
                    statement: "UPDATE session timestamp".to_string(),
                    reason: e.to_string(),
                })?;

                Ok(())
            })
        }
    }

    fn search_messages(&self, query: &str) -> impl std::future::Future<Output = Result<Vec<(SessionId, Message)>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT m.id, m.session_id, m.role, m.content, m.tokens, m.tool_calls, m.metadata, m.created_at
                    FROM messages m
                    WHERE m.content LIKE ?1
                    ORDER BY m.created_at DESC
                    LIMIT 100
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT search messages".to_string(),
                        reason: e.to_string(),
                    })?;

                let pattern = format!("%{query}%");
                let results = stmt
                    .query_map(params![pattern], |row| {
                        let message = Self::row_to_message(row)?;
                        let session_id_str: String = row.get(1)?;
                        let session_id = SessionId::from_uuid(
                            Uuid::parse_str(&session_id_str)
                                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                        );
                        Ok((session_id, message))
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT search messages".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT search messages".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(results)
            })
        }
    }

    fn update_token_usage(&self, id: &SessionId, usage: &TokenUsage) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
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
                )
                .map_err(|e| StorageError::Query {
                    statement: "UPDATE token usage".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }
}

// ─────────────────────────────────────────────────────────
// TimelineRepository implementation
// ─────────────────────────────────────────────────────────

impl TimelineRepository for SqliteBackend {
    fn track_file(&self, path: &Path) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let path_str = path.to_string_lossy().to_string();
            self.with_conn(|conn| {
                conn.execute(
                    "INSERT OR IGNORE INTO tracked_files (path) VALUES (?1)",
                    params![path_str],
                )
                .map_err(|e| StorageError::Query {
                    statement: "INSERT tracked_file".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn tracked_file_count(&self) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            self.with_conn(|conn| {
                let count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM tracked_files", [], |row| row.get(0))
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT tracked_files".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(count as usize)
            })
        }
    }

    fn create_checkpoint(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> impl std::future::Future<Output = Result<CheckpointId>> + Send {
        async move {
            let id = CheckpointId::new();
            let now = Utc::now().to_rfc3339();
            self.with_conn(|conn| {
                conn.execute(
                    r"
                    INSERT INTO checkpoints (id, name, description, timestamp, files_count, total_size)
                    VALUES (?1, ?2, ?3, ?4, 0, 0)
                    ",
                    params![id.0.clone(), name, description, now],
                )
                .map_err(|e| StorageError::Query {
                    statement: "INSERT checkpoint".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })?;
            Ok(id)
        }
    }

    fn list_checkpoints(&self) -> impl std::future::Future<Output = Result<Vec<CheckpointInfo>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, name, description, timestamp, files_count, total_size
                    FROM checkpoints ORDER BY timestamp DESC
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoints".to_string(),
                        reason: e.to_string(),
                    })?;

                let checkpoints = stmt
                    .query_map([], |row| {
                        Ok(CheckpointInfo {
                            id: CheckpointId::from_string(row.get::<_, String>(0)?),
                            name: row.get(1)?,
                            description: row.get(2)?,
                            timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                                .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                            files_count: row.get::<_, i64>(4)? as usize,
                            total_size: row.get::<_, i64>(5)? as usize,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoints".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoints".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(checkpoints)
            })
        }
    }

    fn get_checkpoint(&self, id: &CheckpointId) -> impl std::future::Future<Output = Result<Option<CheckpointInfo>>> + Send {
        async move {
            self.with_conn(|conn| {
                let result = conn
                    .query_row(
                        r"
                    SELECT id, name, description, timestamp, files_count, total_size
                    FROM checkpoints WHERE id = ?1
                    ",
                        params![id.0.clone()],
                        |row| {
                            Ok(CheckpointInfo {
                                id: CheckpointId::from_string(row.get::<_, String>(0)?),
                                name: row.get(1)?,
                                description: row.get(2)?,
                                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                                    .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                                files_count: row.get::<_, i64>(4)? as usize,
                                total_size: row.get::<_, i64>(5)? as usize,
                            })
                        },
                    )
                    .optional()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoint".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(result)
            })
        }
    }

    fn delete_checkpoint(&self, id: &CheckpointId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    "DELETE FROM checkpoints WHERE id = ?1",
                    params![id.0.clone()],
                )
                .map_err(|e| StorageError::Query {
                    statement: "DELETE checkpoint".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn checkpoint_count(&self) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            self.with_conn(|conn| {
                let count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM checkpoints", [], |row| row.get(0))
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT checkpoints".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(count as usize)
            })
        }
    }

    fn get_file_history(&self, path: &Path) -> impl std::future::Future<Output = Result<Vec<FileVersion>>> + Send {
        async move {
            let path_str = path.to_string_lossy().to_string();
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT path, version, checkpoint_id, checksum, size, timestamp
                    FROM file_versions WHERE path = ?1
                    ORDER BY timestamp DESC
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT file history".to_string(),
                        reason: e.to_string(),
                    })?;

                let versions = stmt
                    .query_map(params![path_str], |row| {
                        Ok(FileVersion {
                            path: PathBuf::from(row.get::<_, String>(0)?),
                            version: row.get::<_, i64>(1)? as u64,
                            timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                                .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                            checksum: row.get(3)?,
                            size: row.get::<_, i64>(4)? as usize,
                            checkpoint_id: CheckpointId::from_string(row.get(2)?),
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT file history".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT file history".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(versions)
            })
        }
    }

    fn get_file_version_at_checkpoint(
        &self,
        path: &Path,
        checkpoint_id: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<Option<FileVersion>>> + Send {
        async move {
            let path_str = path.to_string_lossy().to_string();
            self.with_conn(|conn| {
                let result = conn
                    .query_row(
                        r"
                    SELECT path, version, checkpoint_id, checksum, size, timestamp
                    FROM file_versions WHERE path = ?1 AND checkpoint_id = ?2
                    ORDER BY version DESC LIMIT 1
                    ",
                        params![path_str, checkpoint_id.0.clone()],
                        |row| {
                            Ok(FileVersion {
                                path: PathBuf::from(row.get::<_, String>(0)?),
                                version: row.get::<_, i64>(1)? as u64,
                                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                                    .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                                checksum: row.get(3)?,
                                size: row.get::<_, i64>(4)? as usize,
                                checkpoint_id: CheckpointId::from_string(row.get(2)?),
                            })
                        },
                    )
                    .optional()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT file version".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(result)
            })
        }
    }

    fn get_files_changed_between(
        &self,
        from: &CheckpointId,
        to: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<Vec<(PathBuf, FileChangeType)>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT path,
                           CASE
                               WHEN NOT EXISTS (SELECT 1 FROM file_versions fv1 WHERE fv1.path = fv.path AND fv1.checkpoint_id = ?1)
                               THEN 'added'
                               WHEN NOT EXISTS (SELECT 1 FROM file_versions fv2 WHERE fv2.path = fv.path AND fv2.checkpoint_id = ?2)
                               THEN 'deleted'
                               ELSE 'modified'
                           END as change_type
                    FROM file_versions fv
                    WHERE (checkpoint_id = ?1 OR checkpoint_id = ?2)
                    GROUP BY path
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT files changed".to_string(),
                        reason: e.to_string(),
                    })?;

                let changes = stmt
                    .query_map(params![from.0.clone(), to.0.clone()], |row| {
                        let path: String = row.get(0)?;
                        let change_type_str: String = row.get(1)?;
                        let change_type = match change_type_str.as_str() {
                            "added" => FileChangeType::Added,
                            "deleted" => FileChangeType::Deleted,
                            _ => FileChangeType::Modified,
                        };
                        Ok((PathBuf::from(path), change_type))
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT files changed".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT files changed".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(changes)
            })
        }
    }

    fn diff_checkpoints(&self, from: &CheckpointId, to: &CheckpointId) -> impl std::future::Future<Output = Result<Diff>> + Send {
        async move {
            let changes = self.get_files_changed_between(from, to).await?;
            let files_changed: Vec<FileDiff> = changes
                .into_iter()
                .map(|(path, change_type)| FileDiff {
                    path,
                    change_type,
                    additions: 0,
                    deletions: 0,
                })
                .collect();
            let total_additions = files_changed.iter().map(|f| f.additions).sum();
            let total_deletions = files_changed.iter().map(|f| f.deletions).sum();
            let total_files = files_changed.len();
            Ok(Diff {
                from: from.clone(),
                to: to.clone(),
                files_changed,
                summary: DiffSummary {
                    total_files,
                    total_additions,
                    total_deletions,
                },
            })
        }
    }

    fn rollback(&self, _checkpoint_id: &CheckpointId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            Ok(())
        }
    }

    fn rollback_files(
        &self,
        _checkpoint_id: &CheckpointId,
        _files: &[PathBuf],
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            Ok(())
        }
    }

    fn preview_rollback(&self, checkpoint_id: &CheckpointId) -> impl std::future::Future<Output = Result<RollbackPreview>> + Send {
        async move {
            let checkpoint = self
                .get_checkpoint(checkpoint_id)
                .await?
                .ok_or_else(|| StorageError::checkpoint_not_found(&checkpoint_id.0))?;
            Ok(RollbackPreview {
                checkpoint_id: checkpoint_id.clone(),
                files_to_restore: Vec::new(),
                files_to_delete: Vec::new(),
                files_modified: Vec::new(),
                total_files_affected: checkpoint.files_count,
            })
        }
    }

    fn query_by_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> impl std::future::Future<Output = Result<Vec<CheckpointInfo>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, name, description, timestamp, files_count, total_size
                    FROM checkpoints WHERE timestamp >= ?1 AND timestamp <= ?2
                    ORDER BY timestamp DESC
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoints by time".to_string(),
                        reason: e.to_string(),
                    })?;

                let start_str = start.to_rfc3339();
                let end_str = end.to_rfc3339();
                let checkpoints = stmt
                    .query_map(params![start_str, end_str], |row| {
                        Ok(CheckpointInfo {
                            id: CheckpointId::from_string(row.get::<_, String>(0)?),
                            name: row.get(1)?,
                            description: row.get(2)?,
                            timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                                .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                            files_count: row.get::<_, i64>(4)? as usize,
                            total_size: row.get::<_, i64>(5)? as usize,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoints by time".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoints by time".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(checkpoints)
            })
        }
    }

    fn query_by_name(&self, pattern: &str) -> impl std::future::Future<Output = Result<Vec<CheckpointInfo>>> + Send {
        async move {
            let like_pattern = format!("%{pattern}%");
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, name, description, timestamp, files_count, total_size
                    FROM checkpoints WHERE name LIKE ?1
                    ORDER BY timestamp DESC
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoints by name".to_string(),
                        reason: e.to_string(),
                    })?;

                let checkpoints = stmt
                    .query_map(params![like_pattern], |row| {
                        Ok(CheckpointInfo {
                            id: CheckpointId::from_string(row.get::<_, String>(0)?),
                            name: row.get(1)?,
                            description: row.get(2)?,
                            timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                                .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                            files_count: row.get::<_, i64>(4)? as usize,
                            total_size: row.get::<_, i64>(5)? as usize,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoints by name".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoints by name".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(checkpoints)
            })
        }
    }

    fn export_checkpoint(&self, checkpoint_id: &CheckpointId) -> impl std::future::Future<Output = Result<ExportedCheckpoint>> + Send {
        async move {
            let checkpoint = self
                .get_checkpoint(checkpoint_id)
                .await?
                .ok_or_else(|| StorageError::checkpoint_not_found(&checkpoint_id.0))?;

            let versions = self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT path, checksum, size FROM file_versions WHERE checkpoint_id = ?1
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT file versions for export".to_string(),
                        reason: e.to_string(),
                    })?;

                let files = stmt
                    .query_map(params![checkpoint_id.0.clone()], |row| {
                        Ok(ExportedFile {
                            path: PathBuf::from(row.get::<_, String>(0)?),
                            content: String::new(),
                            is_binary: false,
                            hash: row.get(1)?,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT file versions for export".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT file versions for export".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(files)
            })?;

            Ok(ExportedCheckpoint {
                name: checkpoint.name,
                description: checkpoint.description,
                timestamp: checkpoint.timestamp,
                files: versions,
            })
        }
    }

    fn import_checkpoint(&self, exported: ExportedCheckpoint) -> impl std::future::Future<Output = Result<CheckpointId>> + Send {
        async move {
            let id = self.create_checkpoint(&exported.name, exported.description.as_deref()).await?;
            Ok(id)
        }
    }

    fn cleanup_old_checkpoints(&self, keep_count: usize) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            self.with_conn(|conn| {
                let count: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM checkpoints",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT checkpoints".to_string(),
                        reason: e.to_string(),
                    })?;

                if count as usize <= keep_count {
                    return Ok(0);
                }

                let deleted = conn
                    .execute(
                        r"
                    DELETE FROM checkpoints WHERE id NOT IN (
                        SELECT id FROM checkpoints ORDER BY timestamp DESC LIMIT ?1
                    )
                    ",
                        params![keep_count as i64],
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "DELETE old checkpoints".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(deleted)
            })
        }
    }

    fn cleanup_snapshots(&self) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            Ok(0)
        }
    }

    fn storage_stats(&self) -> impl std::future::Future<Output = Result<StorageStats>> + Send {
        async move {
            self.with_conn(|conn| {
                let checkpoint_count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM checkpoints", [], |row| row.get(0))
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT checkpoints".to_string(),
                        reason: e.to_string(),
                    })?;
                let tracked_file_count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM tracked_files", [], |row| row.get(0))
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT tracked_files".to_string(),
                        reason: e.to_string(),
                    })?;
                let version_count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM file_versions", [], |row| row.get(0))
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT file_versions".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(StorageStats {
                    checkpoint_count: checkpoint_count as usize,
                    tracked_file_count: tracked_file_count as usize,
                    total_size_bytes: 0,
                    version_count: version_count as usize,
                })
            })
        }
    }
}

// ─────────────────────────────────────────────────────────
// GraphRepository implementation
// ─────────────────────────────────────────────────────────

impl GraphRepository for SqliteBackend {
    fn insert_file(&self, file: &FileInfo) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    r"
                    INSERT OR REPLACE INTO graph_files (path, hash, language, last_modified)
                    VALUES (?1, ?2, ?3, ?4)
                    ",
                    params![
                        file.path,
                        file.hash,
                        file.language,
                        file.last_modified,
                    ],
                )
                .map_err(|e| StorageError::Query {
                    statement: "INSERT graph_file".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(conn.last_insert_rowid())
            })
        }
    }

    fn get_file_by_path(&self, path: &str) -> impl std::future::Future<Output = Result<Option<FileInfo>>> + Send {
        async move {
            self.with_conn(|conn| {
                let result = conn
                    .query_row(
                        r"
                    SELECT id, path, hash, language, last_modified, created_at
                    FROM graph_files WHERE path = ?1
                    ",
                        params![path],
                        |row| {
                            Ok(FileInfo {
                                path: row.get(1)?,
                                hash: row.get(2)?,
                                language: row.get(3)?,
                                last_modified: row.get(4)?,
                            })
                        },
                    )
                    .optional()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_file".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(result)
            })
        }
    }

    fn get_file_by_id(&self, id: i64) -> impl std::future::Future<Output = Result<Option<FileInfo>>> + Send {
        async move {
            self.with_conn(|conn| {
                let result = conn
                    .query_row(
                        r"
                    SELECT id, path, hash, language, last_modified, created_at
                    FROM graph_files WHERE id = ?1
                    ",
                        params![id],
                        |row| {
                            Ok(FileInfo {
                                path: row.get(1)?,
                                hash: row.get(2)?,
                                language: row.get(3)?,
                                last_modified: row.get(4)?,
                            })
                        },
                    )
                    .optional()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_file by id".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(result)
            })
        }
    }

    fn get_file_id(&self, path: &str) -> impl std::future::Future<Output = Result<Option<i64>>> + Send {
        async move {
            self.with_conn(|conn| {
                let result = conn
                    .query_row(
                        "SELECT id FROM graph_files WHERE path = ?1",
                        params![path],
                        |row| row.get::<_, i64>(0),
                    )
                    .optional()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_file_id".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(result)
            })
        }
    }

    fn delete_file(&self, path: &str) -> impl std::future::Future<Output = Result<bool>> + Send {
        async move {
            self.with_conn(|conn| {
                let affected = conn
                    .execute(
                        "DELETE FROM graph_files WHERE path = ?1",
                        params![path],
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "DELETE graph_file".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(affected > 0)
            })
        }
    }

    fn count_files(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            self.with_conn(|conn| {
                let count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM graph_files", [], |row| row.get(0))
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT graph_files".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(count)
            })
        }
    }

    fn insert_symbol(&self, symbol: &Symbol) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    r"
                    INSERT INTO graph_symbols (file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                    ",
                    params![
                        symbol.file_id,
                        symbol.name,
                        format!("{:?}", symbol.kind),
                        symbol.signature,
                        symbol.doc_comment,
                        symbol.start_line,
                        symbol.end_line,
                        symbol.start_col,
                        symbol.end_col,
                    ],
                )
                .map_err(|e| StorageError::Query {
                    statement: "INSERT graph_symbol".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(conn.last_insert_rowid())
            })
        }
    }

    fn find_symbol(&self, name: &str) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE name = ?1
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbol".to_string(),
                        reason: e.to_string(),
                    })?;

                let symbols = stmt
                    .query_map(params![name], |row| {
                        let kind_str: String = row.get(3)?;
                        let kind = match kind_str.as_str() {
                            "Function" => SymbolKind::Function,
                            "Struct" => SymbolKind::Struct,
                            "Enum" => SymbolKind::Enum,
                            "Trait" => SymbolKind::Trait,
                            "Method" => SymbolKind::Method,
                            "Field" => SymbolKind::Field,
                            "Variable" => SymbolKind::Variable,
                            "Module" => SymbolKind::Module,
                            "Interface" => SymbolKind::Interface,
                            "Class" => SymbolKind::Class,
                            _ => SymbolKind::Function,
                        };
                        Ok(Symbol {
                            id: row.get(0)?,
                            file_id: row.get(1)?,
                            name: row.get(2)?,
                            kind,
                            signature: row.get(4)?,
                            doc_comment: row.get(5)?,
                            start_line: row.get(6)?,
                            end_line: row.get(7)?,
                            start_col: row.get(8)?,
                            end_col: row.get(9)?,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbol".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbol".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(symbols)
            })
        }
    }

    fn find_symbol_by_id(&self, id: i64) -> impl std::future::Future<Output = Result<Option<Symbol>>> + Send {
        async move {
            self.with_conn(|conn| {
                let result = conn
                    .query_row(
                        r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE id = ?1
                    ",
                        params![id],
                        |row| {
                            let kind_str: String = row.get(3)?;
                            let kind = match kind_str.as_str() {
                                "Function" => SymbolKind::Function,
                                "Struct" => SymbolKind::Struct,
                                "Enum" => SymbolKind::Enum,
                                "Trait" => SymbolKind::Trait,
                                "Method" => SymbolKind::Method,
                                "Field" => SymbolKind::Field,
                                "Variable" => SymbolKind::Variable,
                                "Module" => SymbolKind::Module,
                                "Interface" => SymbolKind::Interface,
                                "Class" => SymbolKind::Class,
                                _ => SymbolKind::Function,
                            };
                            Ok(Symbol {
                                id: row.get(0)?,
                                file_id: row.get(1)?,
                                name: row.get(2)?,
                                kind,
                                signature: row.get(4)?,
                                doc_comment: row.get(5)?,
                                start_line: row.get(6)?,
                                end_line: row.get(7)?,
                                start_col: row.get(8)?,
                                end_col: row.get(9)?,
                            })
                        },
                    )
                    .optional()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbol by id".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(result)
            })
        }
    }

    fn find_symbols_by_kind(&self, kind: &SymbolKind) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let kind_str = format!("{:?}", kind);
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE kind = ?1
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbols by kind".to_string(),
                        reason: e.to_string(),
                    })?;

                let symbols = stmt
                    .query_map(params![kind_str], |row| {
                        Ok(Symbol {
                            id: row.get(0)?,
                            file_id: row.get(1)?,
                            name: row.get(2)?,
                            kind: kind.clone(),
                            signature: row.get(4)?,
                            doc_comment: row.get(5)?,
                            start_line: row.get(6)?,
                            end_line: row.get(7)?,
                            start_col: row.get(8)?,
                            end_col: row.get(9)?,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbols by kind".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbols by kind".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(symbols)
            })
        }
    }

    fn find_symbols_in_file(&self, file_id: i64) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE file_id = ?1
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbols in file".to_string(),
                        reason: e.to_string(),
                    })?;

                let symbols = stmt
                    .query_map(params![file_id], |row| {
                        let kind_str: String = row.get(3)?;
                        let kind = match kind_str.as_str() {
                            "Function" => SymbolKind::Function,
                            "Struct" => SymbolKind::Struct,
                            "Enum" => SymbolKind::Enum,
                            "Trait" => SymbolKind::Trait,
                            "Method" => SymbolKind::Method,
                            "Field" => SymbolKind::Field,
                            "Variable" => SymbolKind::Variable,
                            "Module" => SymbolKind::Module,
                            "Interface" => SymbolKind::Interface,
                            "Class" => SymbolKind::Class,
                            _ => SymbolKind::Function,
                        };
                        Ok(Symbol {
                            id: row.get(0)?,
                            file_id: row.get(1)?,
                            name: row.get(2)?,
                            kind,
                            signature: row.get(4)?,
                            doc_comment: row.get(5)?,
                            start_line: row.get(6)?,
                            end_line: row.get(7)?,
                            start_col: row.get(8)?,
                            end_col: row.get(9)?,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbols in file".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_symbols in file".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(symbols)
            })
        }
    }

    fn search_symbols(&self, query: &str) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let pattern = format!("%{query}%");
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE name LIKE ?1
                    LIMIT 100
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT search graph_symbols".to_string(),
                        reason: e.to_string(),
                    })?;

                let symbols = stmt
                    .query_map(params![pattern], |row| {
                        let kind_str: String = row.get(3)?;
                        let kind = match kind_str.as_str() {
                            "Function" => SymbolKind::Function,
                            "Struct" => SymbolKind::Struct,
                            "Enum" => SymbolKind::Enum,
                            "Trait" => SymbolKind::Trait,
                            "Method" => SymbolKind::Method,
                            "Field" => SymbolKind::Field,
                            "Variable" => SymbolKind::Variable,
                            "Module" => SymbolKind::Module,
                            "Interface" => SymbolKind::Interface,
                            "Class" => SymbolKind::Class,
                            _ => SymbolKind::Function,
                        };
                        Ok(Symbol {
                            id: row.get(0)?,
                            file_id: row.get(1)?,
                            name: row.get(2)?,
                            kind,
                            signature: row.get(4)?,
                            doc_comment: row.get(5)?,
                            start_line: row.get(6)?,
                            end_line: row.get(7)?,
                            start_col: row.get(8)?,
                            end_col: row.get(9)?,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT search graph_symbols".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT search graph_symbols".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(symbols)
            })
        }
    }

    fn count_symbols(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            self.with_conn(|conn| {
                let count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM graph_symbols", [], |row| row.get(0))
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT graph_symbols".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(count)
            })
        }
    }

    fn delete_symbols_for_file(&self, file_id: i64) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    "DELETE FROM graph_symbols WHERE file_id = ?1",
                    params![file_id],
                )
                .map_err(|e| StorageError::Query {
                    statement: "DELETE graph_symbols for file".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn insert_reference(&self, reference: &Reference) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    r"
                    INSERT INTO graph_symbol_refs (symbol_id, file_id, line, col, context)
                    VALUES (?1, ?2, ?3, ?4, ?5)
                    ",
                    params![
                        reference.symbol_id,
                        reference.file_id,
                        reference.line,
                        reference.col,
                        reference.context,
                    ],
                )
                .map_err(|e| StorageError::Query {
                    statement: "INSERT graph_ref".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn find_symbol_refs(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Reference>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, symbol_id, file_id, line, col, context
                    FROM graph_symbol_refs WHERE symbol_id = ?1
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_refs".to_string(),
                        reason: e.to_string(),
                    })?;

                let refs = stmt
                    .query_map(params![symbol_id], |row| {
                        Ok(Reference {
                            id: row.get(0)?,
                            symbol_id: row.get(1)?,
                            file_id: row.get(2)?,
                            line: row.get(3)?,
                            col: row.get(4)?,
                            context: row.get(5)?,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_refs".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_refs".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(refs)
            })
        }
    }

    fn count_symbol_refs(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            self.with_conn(|conn| {
                let count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM graph_symbol_refs", [], |row| row.get(0))
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT graph_symbol_refs".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(count)
            })
        }
    }

    fn delete_symbol_refs_for_file(&self, file_id: i64) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    "DELETE FROM graph_symbol_refs WHERE file_id = ?1",
                    params![file_id],
                )
                .map_err(|e| StorageError::Query {
                    statement: "DELETE graph_refs for file".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn insert_relationship(&self, relationship: &Relationship) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    r"
                    INSERT INTO graph_relationships (from_symbol, to_symbol, relationship_type)
                    VALUES (?1, ?2, ?3)
                    ",
                    params![
                        relationship.from_symbol,
                        relationship.to_symbol,
                        format!("{:?}", relationship.relationship_type),
                    ],
                )
                .map_err(|e| StorageError::Query {
                    statement: "INSERT graph_relationship".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn find_relationships(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, from_symbol, to_symbol, relationship_type
                    FROM graph_relationships WHERE from_symbol = ?1 OR to_symbol = ?1
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?;

                let rels = stmt
                    .query_map(params![symbol_id], |row| {
                        let rel_type_str: String = row.get(3)?;
                        let rel_type = match rel_type_str.as_str() {
                            "Calls" => crate::graph_rag::ast::RelationshipType::Calls,
                            "Implements" => crate::graph_rag::ast::RelationshipType::Implements,
                            "Contains" => crate::graph_rag::ast::RelationshipType::Contains,
                            "Imports" => crate::graph_rag::ast::RelationshipType::Imports,
                            "References" => crate::graph_rag::ast::RelationshipType::References,
                            _ => crate::graph_rag::ast::RelationshipType::References,
                        };
                        Ok(Relationship {
                            id: row.get(0)?,
                            from_symbol: row.get(1)?,
                            to_symbol: row.get(2)?,
                            relationship_type: rel_type,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(rels)
            })
        }
    }

    fn find_outgoing_relationships(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, from_symbol, to_symbol, relationship_type
                    FROM graph_relationships WHERE from_symbol = ?1
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT outgoing graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?;

                let rels = stmt
                    .query_map(params![symbol_id], |row| {
                        let rel_type_str: String = row.get(3)?;
                        let rel_type = match rel_type_str.as_str() {
                            "Calls" => crate::graph_rag::ast::RelationshipType::Calls,
                            "Implements" => crate::graph_rag::ast::RelationshipType::Implements,
                            "Contains" => crate::graph_rag::ast::RelationshipType::Contains,
                            "Imports" => crate::graph_rag::ast::RelationshipType::Imports,
                            "References" => crate::graph_rag::ast::RelationshipType::References,
                            _ => crate::graph_rag::ast::RelationshipType::References,
                        };
                        Ok(Relationship {
                            id: row.get(0)?,
                            from_symbol: row.get(1)?,
                            to_symbol: row.get(2)?,
                            relationship_type: rel_type,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT outgoing graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT outgoing graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(rels)
            })
        }
    }

    fn find_incoming_relationships(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, from_symbol, to_symbol, relationship_type
                    FROM graph_relationships WHERE to_symbol = ?1
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT incoming graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?;

                let rels = stmt
                    .query_map(params![symbol_id], |row| {
                        let rel_type_str: String = row.get(3)?;
                        let rel_type = match rel_type_str.as_str() {
                            "Calls" => crate::graph_rag::ast::RelationshipType::Calls,
                            "Implements" => crate::graph_rag::ast::RelationshipType::Implements,
                            "Contains" => crate::graph_rag::ast::RelationshipType::Contains,
                            "Imports" => crate::graph_rag::ast::RelationshipType::Imports,
                            "References" => crate::graph_rag::ast::RelationshipType::References,
                            _ => crate::graph_rag::ast::RelationshipType::References,
                        };
                        Ok(Relationship {
                            id: row.get(0)?,
                            from_symbol: row.get(1)?,
                            to_symbol: row.get(2)?,
                            relationship_type: rel_type,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT incoming graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT incoming graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(rels)
            })
        }
    }

    fn count_relationships(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            self.with_conn(|conn| {
                let count: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM graph_relationships",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT graph_relationships".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(count)
            })
        }
    }

    fn clear(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute_batch(
                    r"
                    DELETE FROM graph_relationships;
                    DELETE FROM graph_symbol_refs;
                    DELETE FROM graph_symbols;
                    DELETE FROM graph_files;
                    ",
                )
                .map_err(|e| StorageError::Query {
                    statement: "CLEAR graph".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }
}

// ─────────────────────────────────────────────────────────
// StorageBackend implementation
// ─────────────────────────────────────────────────────────

impl StorageBackend for SqliteBackend {
    fn backend_type(&self) -> &'static str {
        "sqlite"
    }

    fn migrate(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move { self.initialize() }
    }

    fn health_check(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        let this = self;
        async move {
            this.with_conn(|conn| {
                conn.execute_batch("SELECT 1")
                    .map_err(|e| StorageError::Connection(e.to_string()))?;
                Ok(())
            })
        }
    }

    fn close(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_session_crud() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();

        let session = Session::new();
        let id = session.id;
        backend.create_session(&session).await.unwrap();

        let loaded = backend.load_session(&id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, id);

        let sessions = backend.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 1);

        backend.delete_session(&id).await.unwrap();
        assert!(backend.load_session(&id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_sqlite_save_message() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();

        let session = Session::new();
        let id = session.id;
        backend.create_session(&session).await.unwrap();

        let msg = Message::user("hello world");
        backend.save_message(&id, &msg).await.unwrap();

        let full = backend.load_session_full(&id).await.unwrap().unwrap();
        assert_eq!(full.messages.len(), 1);
        assert_eq!(full.messages[0].as_text(), Some("hello world"));
    }

    #[tokio::test]
    async fn test_sqlite_search_messages() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();

        let session = Session::new();
        let id = session.id;
        backend.create_session(&session).await.unwrap();

        backend
            .save_message(&id, &Message::user("find me if you can"))
            .await
            .unwrap();

        let results = backend.search_messages("find me").await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_sqlite_update_token_usage() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();

        let session = Session::new();
        let id = session.id;
        backend.create_session(&session).await.unwrap();

        backend
            .update_token_usage(
                &id,
                &TokenUsage {
                    input: 100,
                    output: 50,
                    cached: 10,
                },
            )
            .await
            .unwrap();

        let loaded = backend.load_session(&id).await.unwrap().unwrap();
        assert_eq!(loaded.token_usage.input, 100);
        assert_eq!(loaded.token_usage.output, 50);
        assert_eq!(loaded.token_usage.cached, 10);
    }

    #[tokio::test]
    async fn test_sqlite_checkpoint_crud() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();

        let cp = backend
            .create_checkpoint("test-cp", Some("description"))
            .await
            .unwrap();
        assert_eq!(backend.checkpoint_count().await.unwrap(), 1);

        let loaded = backend.get_checkpoint(&cp).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "test-cp");

        let by_name = backend.query_by_name("test").await.unwrap();
        assert_eq!(by_name.len(), 1);

        backend.delete_checkpoint(&cp).await.unwrap();
        assert_eq!(backend.checkpoint_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_sqlite_graph_file_crud() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();

        let file = FileInfo {
            path: "src/main.rs".to_string(),
            hash: "abc123".to_string(),
            language: Some("Rust".to_string()),
            last_modified: None,
        };

        let file_id = backend.insert_file(&file).await.unwrap();
        assert!(file_id > 0);

        let found = backend.get_file_by_path("src/main.rs").await.unwrap();
        assert!(found.is_some());

        let by_id = backend.get_file_by_id(file_id).await.unwrap();
        assert!(by_id.is_some());

        assert_eq!(backend.count_files().await.unwrap(), 1);

        let deleted = backend.delete_file("src/main.rs").await.unwrap();
        assert!(deleted);
        assert_eq!(backend.count_files().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_sqlite_storage_backend_trait() {
        let backend = SqliteBackend::in_memory().unwrap();
        assert_eq!(backend.backend_type(), "sqlite");
        backend.migrate().await.unwrap();
        backend.health_check().await.unwrap();
        backend.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_sqlite_storage_stats() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();
        let stats = backend.storage_stats().await.unwrap();
        assert_eq!(stats.checkpoint_count, 0);
        assert_eq!(stats.tracked_file_count, 0);
    }

    #[tokio::test]
    async fn test_sqlite_track_file() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();
        backend.track_file(&PathBuf::from("test.rs")).await.unwrap();
        assert_eq!(backend.tracked_file_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_sqlite_cleanup_old_checkpoints() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();

        backend.create_checkpoint("cp1", None).await.unwrap();
        backend.create_checkpoint("cp2", None).await.unwrap();
        backend.create_checkpoint("cp3", None).await.unwrap();

        let deleted = backend.cleanup_old_checkpoints(1).await.unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(backend.checkpoint_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_sqlite_health_check() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();
        backend.health_check().await.unwrap();
    }

    #[tokio::test]
    async fn test_sqlite_graph_clear() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();

        let file = FileInfo {
            path: "test.rs".to_string(),
            hash: "hash".to_string(),
            language: None,
            last_modified: None,
        };
        backend.insert_file(&file).await.unwrap();
        assert_eq!(backend.count_files().await.unwrap(), 1);

        backend.clear().await.unwrap();
        assert_eq!(backend.count_files().await.unwrap(), 0);
    }
}
