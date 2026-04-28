//! PostgreSQL storage backend implementation.
//!
//! Implements all three domain traits (`SessionRepository`, `TimelineRepository`,
//! `GraphRepository`) and the unified `StorageBackend` trait using `tokio-postgres`
//! with `deadpool_postgres` connection pooling.
//!
//! Enable with the `postgres` feature flag.

use super::backend::{
    GraphRepository, SessionRepository, StorageBackend, TimelineRepository, WorkspaceRepository,
};
use super::error::StorageError;
use crate::error::Result;
use crate::graph_rag::ast::{
    FileInfo, Reference, Relationship, RelationshipType, Symbol, SymbolKind,
};
use crate::session::types::{
    ContentPart, Message, MessageContent, MessageRole, Session, SessionId,
    SessionMeta, TokenUsage,
};
use crate::timeline::{
    CheckpointId, CheckpointInfo, Diff, DiffSummary, ExportedCheckpoint,
    ExportedFile, FileChangeType, FileDiff, FileVersion, RollbackPreview, StorageStats,
};
use crate::workspace::{Project, ProjectId, Workspace, WorkspaceId};
use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;
use std::path::{Path, PathBuf};
use tokio_postgres::types::ToSql;
use uuid::Uuid;

// ─────────────────────────────────────────────────────────
// SQL Schema (PostgreSQL adaptation)
// ─────────────────────────────────────────────────────────

const SCHEMA_VERSION: i32 = 1;

const INIT_SQL: &str = r"
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    title TEXT,
    provider TEXT,
    model TEXT,
    working_dir TEXT,
    tags TEXT NOT NULL DEFAULT '[]',
    extra TEXT NOT NULL DEFAULT '{}',
    input_tokens BIGINT NOT NULL DEFAULT 0,
    output_tokens BIGINT NOT NULL DEFAULT 0,
    cached_tokens BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    tokens INTEGER,
    tool_calls TEXT,
    metadata TEXT,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_updated_at ON sessions(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_messages_session_id ON messages(session_id);
CREATE INDEX IF NOT EXISTS idx_messages_content ON messages(content);

CREATE TABLE IF NOT EXISTS tracked_files (
    id BIGSERIAL PRIMARY KEY,
    path TEXT UNIQUE NOT NULL,
    tracked_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS checkpoints (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    timestamp TIMESTAMPTZ NOT NULL,
    files_count INTEGER NOT NULL DEFAULT 0,
    total_size BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS file_versions (
    id BIGSERIAL PRIMARY KEY,
    path TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    checkpoint_id TEXT NOT NULL REFERENCES checkpoints(id) ON DELETE CASCADE,
    checksum TEXT NOT NULL,
    size BIGINT NOT NULL DEFAULT 0,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_file_versions_path ON file_versions(path);
CREATE INDEX IF NOT EXISTS idx_file_versions_checkpoint ON file_versions(checkpoint_id);
CREATE INDEX IF NOT EXISTS idx_checkpoints_timestamp ON checkpoints(timestamp DESC);

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    default_project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS workspace_projects (
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    added_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (workspace_id, project_id)
);

CREATE TABLE IF NOT EXISTS graph_files (
    id BIGSERIAL PRIMARY KEY,
    path TEXT UNIQUE NOT NULL,
    hash TEXT NOT NULL,
    language TEXT,
    last_modified BIGINT,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS graph_symbols (
    id BIGSERIAL PRIMARY KEY,
    file_id BIGINT REFERENCES graph_files(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    signature TEXT,
    doc_comment TEXT,
    start_line INTEGER,
    end_line INTEGER,
    start_col INTEGER,
    end_col INTEGER,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS graph_symbol_refs (
    id BIGSERIAL PRIMARY KEY,
    symbol_id BIGINT REFERENCES graph_symbols(id) ON DELETE CASCADE,
    file_id BIGINT REFERENCES graph_files(id) ON DELETE CASCADE,
    line INTEGER,
    col INTEGER,
    context TEXT,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS graph_relationships (
    id BIGSERIAL PRIMARY KEY,
    from_symbol BIGINT REFERENCES graph_symbols(id) ON DELETE CASCADE,
    to_symbol BIGINT REFERENCES graph_symbols(id) ON DELETE CASCADE,
    relationship_type TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

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
// PostgresBackend
// ─────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct PostgresBackend {
    pool: Pool,
}

impl PostgresBackend {
    /// Connect to a PostgreSQL database.
    ///
    /// `config_str` is a PostgreSQL connection string, e.g.:
    /// "host=localhost user=clawdius dbname=clawdius"
    pub async fn connect(config_str: &str) -> Result<Self> {
        let mut config: tokio_postgres::Config =
            config_str.parse().map_err(|e: tokio_postgres::Error| {
                StorageError::Connection(format!("failed to parse postgres config: {e}"))
            })?;

        let mgr_config = deadpool_postgres::ManagerConfig {
            recycling_method: deadpool_postgres::RecyclingMethod::Fast,
        };
        let mgr =
            deadpool_postgres::Manager::from_config(config, tokio_postgres::NoTls, mgr_config);

        let pool = Pool::builder(mgr)
            .build()
            .map_err(|e| StorageError::Connection(format!("failed to build pool: {e}")))?;

        Ok(Self { pool })
    }

    /// Create from an existing pool (for testing or custom config).
    pub fn from_pool(pool: Pool) -> Self {
        Self { pool }
    }

    async fn get_client(
        &self,
    ) -> std::result::Result<deadpool_postgres::Client, StorageError> {
        self.pool
            .get()
            .await
            .map_err(|e| StorageError::Connection(format!("failed to get connection from pool: {e}")))
    }

    // ── Row mapping helpers ──

    fn row_to_session(row: &tokio_postgres::Row) -> std::result::Result<Session, StorageError> {
        let id_str: String = row.get(0);
        let title: Option<String> = row.get(1);
        let provider: Option<String> = row.get(2);
        let model: Option<String> = row.get(3);
        let working_dir: Option<String> = row.get(4);
        let tags_json: String = row.get(5);
        let extra_json: String = row.get(6);
        let input_tokens: i64 = row.get(7);
        let output_tokens: i64 = row.get(8);
        let cached_tokens: i64 = row.get(9);
        let created_at: DateTime<Utc> = row.get(10);
        let updated_at: DateTime<Utc> = row.get(11);

        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        let extra: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&extra_json).unwrap_or_default();

        Ok(Session {
            id: SessionId::from_uuid(
                Uuid::parse_str(&id_str).map_err(|e| StorageError::RowConversion {
                    reason: format!("invalid session UUID: {e}"),
                })?,
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
            created_at,
            updated_at,
        })
    }

    fn row_to_message(row: &tokio_postgres::Row) -> std::result::Result<Message, StorageError> {
        let id_str: String = row.get(0);
        let _session_id: String = row.get(1);
        let role_str: String = row.get(2);
        let content_json: String = row.get(3);
        let tokens: Option<i32> = row.get(4);
        let tool_calls_json: Option<String> = row.get(5);
        let metadata_json: Option<String> = row.get(6);
        let created_at: DateTime<Utc> = row.get(7);

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
            id: Uuid::parse_str(&id_str).map_err(|e| StorageError::RowConversion {
                reason: format!("invalid message UUID: {e}"),
            })?,
            role: MessageRole::parse_role(&role_str),
            content,
            tokens: tokens.map(|t| t as usize),
            created_at,
            tool_calls,
            metadata,
        })
    }

    fn row_to_checkpoint_info(row: &tokio_postgres::Row) -> CheckpointInfo {
        CheckpointInfo {
            id: CheckpointId::from_string(row.get::<_, String>(0)),
            name: row.get(1),
            description: row.get(2),
            timestamp: row.get(3),
            files_count: row.get::<_, i32>(4) as usize,
            total_size: row.get::<_, i64>(5) as usize,
        }
    }

    fn row_to_file_version(row: &tokio_postgres::Row) -> FileVersion {
        FileVersion {
            path: PathBuf::from(row.get::<_, String>(0)),
            version: row.get::<_, i32>(1) as u64,
            timestamp: row.get(5),
            checksum: row.get(3),
            size: row.get::<_, i64>(4) as usize,
            checkpoint_id: CheckpointId::from_string(row.get(2)),
        }
    }

    fn row_to_symbol(row: &tokio_postgres::Row) -> std::result::Result<Symbol, StorageError> {
        let kind_str: String = row.get(3);
        let kind = Self::parse_symbol_kind(&kind_str);
        Ok(Symbol {
            id: Some(row.get(0)),
            file_id: row.get(1),
            name: row.get(2),
            kind,
            signature: row.get(4),
            doc_comment: row.get(5),
            start_line: row.get(6),
            end_line: row.get(7),
            start_col: row.get(8),
            end_col: row.get(9),
        })
    }

    fn row_to_reference(row: &tokio_postgres::Row) -> Reference {
        Reference {
            id: Some(row.get(0)),
            symbol_id: row.get(1),
            file_id: row.get(2),
            line: row.get(3),
            col: row.get(4),
            context: row.get(5),
        }
    }

    fn row_to_relationship(row: &tokio_postgres::Row) -> Relationship {
        let rel_type_str: String = row.get(3);
        let rel_type = Self::parse_relationship_type(&rel_type_str);
        Relationship {
            id: Some(row.get(0)),
            from_symbol: row.get(1),
            to_symbol: row.get(2),
            relationship_type: rel_type,
        }
    }

    fn parse_symbol_kind(s: &str) -> SymbolKind {
        match s {
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
            "Constant" => SymbolKind::Constant,
            "Type" => SymbolKind::Type,
            "Macro" => SymbolKind::Macro,
            other => SymbolKind::Other(other.to_string()),
        }
    }

    fn parse_relationship_type(s: &str) -> RelationshipType {
        match s {
            "Calls" => RelationshipType::Calls,
            "Implements" => RelationshipType::Implements,
            "Contains" => RelationshipType::Contains,
            "Imports" => RelationshipType::Imports,
            "References" => RelationshipType::References,
            "Extends" => RelationshipType::Extends,
            "DependsOn" => RelationshipType::DependsOn,
            other => RelationshipType::Other(other.to_string()),
        }
    }
}

// ─────────────────────────────────────────────────────────
// SessionRepository implementation
// ─────────────────────────────────────────────────────────

impl SessionRepository for PostgresBackend {
    fn create_session(&self, session: &Session) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            let tags_json = serde_json::to_string(&session.meta.tags)?;
            let extra_json = serde_json::to_string(&session.meta.extra)?;

            client
                .execute(
                    r"
                    INSERT INTO sessions (
                        id, title, provider, model, working_dir, tags, extra,
                        input_tokens, output_tokens, cached_tokens,
                        created_at, updated_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                    ",
                    &[
                        &session.id.to_string() as &(dyn ToSql + Sync),
                        &session.title,
                        &session.meta.provider,
                        &session.meta.model,
                        &session.meta.working_dir,
                        &tags_json,
                        &extra_json,
                        &(session.token_usage.input as i64),
                        &(session.token_usage.output as i64),
                        &(session.token_usage.cached as i64),
                        &session.created_at,
                        &session.updated_at,
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT INTO sessions".to_string(),
                    reason: e.to_string(),
                })?;

            Ok(())
        }
    }

    fn load_session(&self, id: &SessionId) -> impl std::future::Future<Output = Result<Option<Session>>> + Send {
        async move {
            let client = self.get_client().await?;
            let result = client
                .query_opt(
                    r"
                    SELECT id, title, provider, model, working_dir, tags, extra,
                           input_tokens, output_tokens, cached_tokens,
                           created_at, updated_at
                    FROM sessions WHERE id = $1
                    ",
                    &[&id.to_string() as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT session".to_string(),
                    reason: e.to_string(),
                })?;

            match result {
                Some(row) => Ok(Some(Self::row_to_session(&row)?)),
                None => Ok(None),
            }
        }
    }

    fn load_session_full(&self, id: &SessionId) -> impl std::future::Future<Output = Result<Option<Session>>> + Send {
        async move {
            let Some(mut session) = self.load_session(id).await? else {
                return Ok(None);
            };

            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT id, session_id, role, content, tokens, tool_calls, metadata, created_at
                    FROM messages WHERE session_id = $1
                    ORDER BY created_at ASC
                    ",
                    &[&id.to_string() as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT messages".to_string(),
                    reason: e.to_string(),
                })?;

            let messages: Vec<Message> = rows
                .iter()
                .map(|row| Self::row_to_message(row))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| StorageError::RowConversion {
                    reason: e.to_string(),
                })?;

            session.messages = messages;
            Ok(Some(session))
        }
    }

    fn list_sessions(&self) -> impl std::future::Future<Output = Result<Vec<Session>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT id, title, provider, model, working_dir, tags, extra,
                           input_tokens, output_tokens, cached_tokens,
                           created_at, updated_at
                    FROM sessions
                    ORDER BY updated_at DESC
                    ",
                    &[],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT sessions".to_string(),
                    reason: e.to_string(),
                })?;

            let sessions: Vec<Session> = rows
                .iter()
                .map(|row| Self::row_to_session(row))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| StorageError::RowConversion {
                    reason: e.to_string(),
                })?;

            Ok(sessions)
        }
    }

    fn delete_session(&self, id: &SessionId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "DELETE FROM sessions WHERE id = $1",
                    &[&id.to_string() as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "DELETE session".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
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

            let client = self.get_client().await?;
            client
                .execute(
                    r"
                    INSERT INTO messages (
                        id, session_id, role, content, tokens, tool_calls, metadata, created_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                    ",
                    &[
                        &message.id.to_string() as &(dyn ToSql + Sync),
                        &session_id.to_string(),
                        &message.role.as_str(),
                        &content_json,
                        &message.tokens.map(|t| t as i32),
                        &tool_calls_json,
                        &metadata_json,
                        &message.created_at,
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT message".to_string(),
                    reason: e.to_string(),
                })?;

            client
                .execute(
                    "UPDATE sessions SET updated_at = $1 WHERE id = $2",
                    &[&Utc::now() as &(dyn ToSql + Sync), &session_id.to_string()],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "UPDATE session timestamp".to_string(),
                    reason: e.to_string(),
                })?;

            Ok(())
        }
    }

    fn search_messages(&self, query: &str) -> impl std::future::Future<Output = Result<Vec<(SessionId, Message)>>> + Send {
        async move {
            let client = self.get_client().await?;
            let pattern = format!("%{query}%");
            let rows = client
                .query(
                    r"
                    SELECT m.id, m.session_id, m.role, m.content, m.tokens, m.tool_calls, m.metadata, m.created_at
                    FROM messages m
                    WHERE m.content LIKE $1
                    ORDER BY m.created_at DESC
                    LIMIT 100
                    ",
                    &[&pattern as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT search messages".to_string(),
                    reason: e.to_string(),
                })?;

            let mut results = Vec::new();
            for row in &rows {
                let message = Self::row_to_message(row)?;
                let session_id_str: String = row.get(1);
                let session_id = SessionId::from_uuid(
                    Uuid::parse_str(&session_id_str).map_err(|e| StorageError::RowConversion {
                        reason: format!("invalid session UUID: {e}"),
                    })?,
                );
                results.push((session_id, message));
            }

            Ok(results)
        }
    }

    fn update_token_usage(&self, id: &SessionId, usage: &TokenUsage) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    r"
                    UPDATE sessions SET
                        input_tokens = $1,
                        output_tokens = $2,
                        cached_tokens = $3,
                        updated_at = $4
                    WHERE id = $5
                    ",
                    &[
                        &(usage.input as i64) as &(dyn ToSql + Sync),
                        &(usage.output as i64),
                        &(usage.cached as i64),
                        &Utc::now(),
                        &id.to_string(),
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "UPDATE token usage".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }
}

// ─────────────────────────────────────────────────────────
// TimelineRepository implementation
// ─────────────────────────────────────────────────────────

impl TimelineRepository for PostgresBackend {
    fn track_file(&self, path: &Path) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            let path_str = path.to_string_lossy().to_string();
            client
                .execute(
                    "INSERT INTO tracked_files (path) VALUES ($1) ON CONFLICT (path) DO NOTHING",
                    &[&path_str as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT tracked_file".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn tracked_file_count(&self) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_one("SELECT COUNT(*) FROM tracked_files", &[])
                .await
                .map_err(|e| StorageError::Query {
                    statement: "COUNT tracked_files".to_string(),
                    reason: e.to_string(),
                })?;
            let count: i64 = row.get(0);
            Ok(count as usize)
        }
    }

    fn create_checkpoint(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> impl std::future::Future<Output = Result<CheckpointId>> + Send {
        async move {
            let id = CheckpointId::new();
            let now = Utc::now();
            let client = self.get_client().await?;
            client
                .execute(
                    r"
                    INSERT INTO checkpoints (id, name, description, timestamp, files_count, total_size)
                    VALUES ($1, $2, $3, $4, 0, 0)
                    ",
                    &[
                        &id.0 as &(dyn ToSql + Sync),
                        &name,
                        &description,
                        &now,
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT checkpoint".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(id)
        }
    }

    fn list_checkpoints(&self) -> impl std::future::Future<Output = Result<Vec<CheckpointInfo>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT id, name, description, timestamp, files_count, total_size
                    FROM checkpoints ORDER BY timestamp DESC
                    ",
                    &[],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT checkpoints".to_string(),
                    reason: e.to_string(),
                })?;

            let checkpoints: Vec<CheckpointInfo> = rows.iter().map(Self::row_to_checkpoint_info).collect();
            Ok(checkpoints)
        }
    }

    fn get_checkpoint(&self, id: &CheckpointId) -> impl std::future::Future<Output = Result<Option<CheckpointInfo>>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_opt(
                    r"
                    SELECT id, name, description, timestamp, files_count, total_size
                    FROM checkpoints WHERE id = $1
                    ",
                    &[&id.0 as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT checkpoint".to_string(),
                    reason: e.to_string(),
                })?;

            Ok(row.map(|r| Self::row_to_checkpoint_info(&r)))
        }
    }

    fn delete_checkpoint(&self, id: &CheckpointId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "DELETE FROM checkpoints WHERE id = $1",
                    &[&id.0 as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "DELETE checkpoint".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn checkpoint_count(&self) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_one("SELECT COUNT(*) FROM checkpoints", &[])
                .await
                .map_err(|e| StorageError::Query {
                    statement: "COUNT checkpoints".to_string(),
                    reason: e.to_string(),
                })?;
            let count: i64 = row.get(0);
            Ok(count as usize)
        }
    }

    fn get_file_history(&self, path: &Path) -> impl std::future::Future<Output = Result<Vec<FileVersion>>> + Send {
        async move {
            let client = self.get_client().await?;
            let path_str = path.to_string_lossy().to_string();
            let rows = client
                .query(
                    r"
                    SELECT path, version, checkpoint_id, checksum, size, timestamp
                    FROM file_versions WHERE path = $1
                    ORDER BY timestamp DESC
                    ",
                    &[&path_str as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT file history".to_string(),
                    reason: e.to_string(),
                })?;

            let versions: Vec<FileVersion> = rows.iter().map(Self::row_to_file_version).collect();
            Ok(versions)
        }
    }

    fn get_file_version_at_checkpoint(
        &self,
        path: &Path,
        checkpoint_id: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<Option<FileVersion>>> + Send {
        async move {
            let client = self.get_client().await?;
            let path_str = path.to_string_lossy().to_string();
            let row = client
                .query_opt(
                    r"
                    SELECT path, version, checkpoint_id, checksum, size, timestamp
                    FROM file_versions WHERE path = $1 AND checkpoint_id = $2
                    ORDER BY version DESC LIMIT 1
                    ",
                    &[&path_str as &(dyn ToSql + Sync), &checkpoint_id.0],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT file version".to_string(),
                    reason: e.to_string(),
                })?;

            Ok(row.map(|r| Self::row_to_file_version(&r)))
        }
    }

    fn get_files_changed_between(
        &self,
        from: &CheckpointId,
        to: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<Vec<(PathBuf, FileChangeType)>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT path,
                           CASE
                               WHEN NOT EXISTS (SELECT 1 FROM file_versions fv1 WHERE fv1.path = fv.path AND fv1.checkpoint_id = $1)
                               THEN 'added'
                               WHEN NOT EXISTS (SELECT 1 FROM file_versions fv2 WHERE fv2.path = fv.path AND fv2.checkpoint_id = $2)
                               THEN 'deleted'
                               ELSE 'modified'
                           END as change_type
                    FROM file_versions fv
                    WHERE (checkpoint_id = $1 OR checkpoint_id = $2)
                    GROUP BY path
                    ",
                    &[&from.0 as &(dyn ToSql + Sync), &to.0],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT files changed".to_string(),
                    reason: e.to_string(),
                })?;

            let mut changes = Vec::new();
            for row in &rows {
                let path: String = row.get(0);
                let change_type_str: String = row.get(1);
                let change_type = match change_type_str.as_str() {
                    "added" => FileChangeType::Added,
                    "deleted" => FileChangeType::Deleted,
                    _ => FileChangeType::Modified,
                };
                changes.push((PathBuf::from(path), change_type));
            }

            Ok(changes)
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
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT id, name, description, timestamp, files_count, total_size
                    FROM checkpoints WHERE timestamp >= $1 AND timestamp <= $2
                    ORDER BY timestamp DESC
                    ",
                    &[&start as &(dyn ToSql + Sync), &end],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT checkpoints by time".to_string(),
                    reason: e.to_string(),
                })?;

            let checkpoints: Vec<CheckpointInfo> = rows.iter().map(Self::row_to_checkpoint_info).collect();
            Ok(checkpoints)
        }
    }

    fn query_by_name(&self, pattern: &str) -> impl std::future::Future<Output = Result<Vec<CheckpointInfo>>> + Send {
        async move {
            let client = self.get_client().await?;
            let like_pattern = format!("%{pattern}%");
            let rows = client
                .query(
                    r"
                    SELECT id, name, description, timestamp, files_count, total_size
                    FROM checkpoints WHERE name LIKE $1
                    ORDER BY timestamp DESC
                    ",
                    &[&like_pattern as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT checkpoints by name".to_string(),
                    reason: e.to_string(),
                })?;

            let checkpoints: Vec<CheckpointInfo> = rows.iter().map(Self::row_to_checkpoint_info).collect();
            Ok(checkpoints)
        }
    }

    fn export_checkpoint(&self, checkpoint_id: &CheckpointId) -> impl std::future::Future<Output = Result<ExportedCheckpoint>> + Send {
        async move {
            let checkpoint = self
                .get_checkpoint(checkpoint_id)
                .await?
                .ok_or_else(|| StorageError::checkpoint_not_found(&checkpoint_id.0))?;

            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT path, checksum, size FROM file_versions WHERE checkpoint_id = $1
                    ",
                    &[&checkpoint_id.0 as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT file versions for export".to_string(),
                    reason: e.to_string(),
                })?;

            let files: Vec<ExportedFile> = rows
                .iter()
                .map(|row| ExportedFile {
                    path: PathBuf::from(row.get::<_, String>(0)),
                    content: String::new(),
                    is_binary: false,
                    hash: row.get(1),
                })
                .collect();

            Ok(ExportedCheckpoint {
                name: checkpoint.name,
                description: checkpoint.description,
                timestamp: checkpoint.timestamp,
                files,
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
            let client = self.get_client().await?;
            let row = client
                .query_one(
                    "SELECT COUNT(*) FROM checkpoints",
                    &[],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "COUNT checkpoints".to_string(),
                    reason: e.to_string(),
                })?;
            let count: i64 = row.get(0);

            if count as usize <= keep_count {
                return Ok(0);
            }

            let result = client
                .execute(
                    r"
                    DELETE FROM checkpoints WHERE id NOT IN (
                        SELECT id FROM checkpoints ORDER BY timestamp DESC LIMIT $1
                    )
                    ",
                    &[&(keep_count as i64) as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "DELETE old checkpoints".to_string(),
                    reason: e.to_string(),
                })?;

            Ok(result as usize)
        }
    }

    fn cleanup_snapshots(&self) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            Ok(0)
        }
    }

    fn storage_stats(&self) -> impl std::future::Future<Output = Result<StorageStats>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_one(
                    r"
                    SELECT
                        (SELECT COUNT(*) FROM checkpoints) AS checkpoint_count,
                        (SELECT COUNT(*) FROM tracked_files) AS tracked_file_count,
                        (SELECT COUNT(*) FROM file_versions) AS version_count
                    ",
                    &[],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT storage stats".to_string(),
                    reason: e.to_string(),
                })?;

            Ok(StorageStats {
                checkpoint_count: row.get::<_, i64>(0) as usize,
                tracked_file_count: row.get::<_, i64>(1) as usize,
                total_size_bytes: 0,
                version_count: row.get::<_, i64>(2) as usize,
            })
        }
    }
}

// ─────────────────────────────────────────────────────────
// GraphRepository implementation
// ─────────────────────────────────────────────────────────

impl GraphRepository for PostgresBackend {
    fn insert_file(&self, file: &FileInfo) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let client = self.get_client().await?;
            let result = client
                .query_one(
                    r"
                    INSERT INTO graph_files (path, hash, language, last_modified)
                    VALUES ($1, $2, $3, $4)
                    ON CONFLICT (path) DO UPDATE SET hash = EXCLUDED.hash, language = EXCLUDED.language, last_modified = EXCLUDED.last_modified
                    RETURNING id
                    ",
                    &[
                        &file.path as &(dyn ToSql + Sync),
                        &file.hash,
                        &file.language,
                        &file.last_modified,
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT graph_file".to_string(),
                    reason: e.to_string(),
                })?;
            let id: i64 = result.get(0);
            Ok(id)
        }
    }

    fn get_file_by_path(&self, path: &str) -> impl std::future::Future<Output = Result<Option<FileInfo>>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_opt(
                    r"
                    SELECT id, path, hash, language, last_modified, created_at
                    FROM graph_files WHERE path = $1
                    ",
                    &[&path as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT graph_file".to_string(),
                    reason: e.to_string(),
                })?;

            match row {
                Some(r) => Ok(Some(FileInfo {
                    path: r.get(1),
                    hash: r.get(2),
                    language: r.get(3),
                    last_modified: r.get(4),
                })),
                None => Ok(None),
            }
        }
    }

    fn get_file_by_id(&self, id: i64) -> impl std::future::Future<Output = Result<Option<FileInfo>>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_opt(
                    r"
                    SELECT id, path, hash, language, last_modified, created_at
                    FROM graph_files WHERE id = $1
                    ",
                    &[&id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT graph_file by id".to_string(),
                    reason: e.to_string(),
                })?;

            match row {
                Some(r) => Ok(Some(FileInfo {
                    path: r.get(1),
                    hash: r.get(2),
                    language: r.get(3),
                    last_modified: r.get(4),
                })),
                None => Ok(None),
            }
        }
    }

    fn get_file_id(&self, path: &str) -> impl std::future::Future<Output = Result<Option<i64>>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_opt(
                    "SELECT id FROM graph_files WHERE path = $1",
                    &[&path as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT graph_file_id".to_string(),
                    reason: e.to_string(),
                })?;

            Ok(row.map(|r| r.get::<_, i64>(0)))
        }
    }

    fn delete_file(&self, path: &str) -> impl std::future::Future<Output = Result<bool>> + Send {
        async move {
            let client = self.get_client().await?;
            let result = client
                .execute(
                    "DELETE FROM graph_files WHERE path = $1",
                    &[&path as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "DELETE graph_file".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(result > 0)
        }
    }

    fn count_files(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_one("SELECT COUNT(*) FROM graph_files", &[])
                .await
                .map_err(|e| StorageError::Query {
                    statement: "COUNT graph_files".to_string(),
                    reason: e.to_string(),
                })?;
            let count: i64 = row.get(0);
            Ok(count)
        }
    }

    fn insert_symbol(&self, symbol: &Symbol) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let client = self.get_client().await?;
            let result = client
                .query_one(
                    r"
                    INSERT INTO graph_symbols (file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                    RETURNING id
                    ",
                    &[
                        &symbol.file_id as &(dyn ToSql + Sync),
                        &symbol.name,
                        &format!("{:?}", symbol.kind),
                        &symbol.signature,
                        &symbol.doc_comment,
                        &symbol.start_line,
                        &symbol.end_line,
                        &symbol.start_col,
                        &symbol.end_col,
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT graph_symbol".to_string(),
                    reason: e.to_string(),
                })?;
            let id: i64 = result.get(0);
            Ok(id)
        }
    }

    fn find_symbol(&self, name: &str) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE name = $1
                    ",
                    &[&name as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT graph_symbol".to_string(),
                    reason: e.to_string(),
                })?;

            let symbols: Vec<Symbol> = rows
                .iter()
                .map(|row| Self::row_to_symbol(row))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| StorageError::RowConversion {
                    reason: e.to_string(),
                })?;

            Ok(symbols)
        }
    }

    fn find_symbol_by_id(&self, id: i64) -> impl std::future::Future<Output = Result<Option<Symbol>>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_opt(
                    r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE id = $1
                    ",
                    &[&id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT graph_symbol by id".to_string(),
                    reason: e.to_string(),
                })?;

            match row {
                Some(r) => Ok(Some(Self::row_to_symbol(&r)?)),
                None => Ok(None),
            }
        }
    }

    fn find_symbols_by_kind(&self, kind: &SymbolKind) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let client = self.get_client().await?;
            let kind_str = format!("{:?}", kind);
            let rows = client
                .query(
                    r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE kind = $1
                    ",
                    &[&kind_str as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT graph_symbols by kind".to_string(),
                    reason: e.to_string(),
                })?;

            let symbols: Vec<Symbol> = rows
                .iter()
                .map(|row| Self::row_to_symbol(row))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| StorageError::RowConversion {
                    reason: e.to_string(),
                })?;

            Ok(symbols)
        }
    }

    fn find_symbols_in_file(&self, file_id: i64) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE file_id = $1
                    ",
                    &[&file_id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT graph_symbols in file".to_string(),
                    reason: e.to_string(),
                })?;

            let symbols: Vec<Symbol> = rows
                .iter()
                .map(|row| Self::row_to_symbol(row))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| StorageError::RowConversion {
                    reason: e.to_string(),
                })?;

            Ok(symbols)
        }
    }

    fn search_symbols(&self, query: &str) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let client = self.get_client().await?;
            let pattern = format!("%{query}%");
            let rows = client
                .query(
                    r"
                    SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                    FROM graph_symbols WHERE name LIKE $1
                    LIMIT 100
                    ",
                    &[&pattern as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT search graph_symbols".to_string(),
                    reason: e.to_string(),
                })?;

            let symbols: Vec<Symbol> = rows
                .iter()
                .map(|row| Self::row_to_symbol(row))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| StorageError::RowConversion {
                    reason: e.to_string(),
                })?;

            Ok(symbols)
        }
    }

    fn count_symbols(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_one("SELECT COUNT(*) FROM graph_symbols", &[])
                .await
                .map_err(|e| StorageError::Query {
                    statement: "COUNT graph_symbols".to_string(),
                    reason: e.to_string(),
                })?;
            let count: i64 = row.get(0);
            Ok(count)
        }
    }

    fn delete_symbols_for_file(&self, file_id: i64) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "DELETE FROM graph_symbols WHERE file_id = $1",
                    &[&file_id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "DELETE graph_symbols for file".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn insert_reference(&self, reference: &Reference) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    r"
                    INSERT INTO graph_symbol_refs (symbol_id, file_id, line, col, context)
                    VALUES ($1, $2, $3, $4, $5)
                    ",
                    &[
                        &reference.symbol_id as &(dyn ToSql + Sync),
                        &reference.file_id,
                        &reference.line,
                        &reference.col,
                        &reference.context,
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT graph_ref".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn find_symbol_refs(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Reference>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT id, symbol_id, file_id, line, col, context
                    FROM graph_symbol_refs WHERE symbol_id = $1
                    ",
                    &[&symbol_id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT graph_refs".to_string(),
                    reason: e.to_string(),
                })?;

            let refs: Vec<Reference> = rows.iter().map(Self::row_to_reference).collect();
            Ok(refs)
        }
    }

    fn count_symbol_refs(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_one("SELECT COUNT(*) FROM graph_symbol_refs", &[])
                .await
                .map_err(|e| StorageError::Query {
                    statement: "COUNT graph_symbol_refs".to_string(),
                    reason: e.to_string(),
                })?;
            let count: i64 = row.get(0);
            Ok(count)
        }
    }

    fn delete_symbol_refs_for_file(&self, file_id: i64) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "DELETE FROM graph_symbol_refs WHERE file_id = $1",
                    &[&file_id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "DELETE graph_refs for file".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn insert_relationship(&self, relationship: &Relationship) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    r"
                    INSERT INTO graph_relationships (from_symbol, to_symbol, relationship_type)
                    VALUES ($1, $2, $3)
                    ",
                    &[
                        &relationship.from_symbol as &(dyn ToSql + Sync),
                        &relationship.to_symbol,
                        &format!("{:?}", relationship.relationship_type),
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT graph_relationship".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn find_relationships(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT id, from_symbol, to_symbol, relationship_type
                    FROM graph_relationships WHERE from_symbol = $1 OR to_symbol = $1
                    ",
                    &[&symbol_id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT graph_relationships".to_string(),
                    reason: e.to_string(),
                })?;

            let rels: Vec<Relationship> = rows.iter().map(Self::row_to_relationship).collect();
            Ok(rels)
        }
    }

    fn find_outgoing_relationships(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT id, from_symbol, to_symbol, relationship_type
                    FROM graph_relationships WHERE from_symbol = $1
                    ",
                    &[&symbol_id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT outgoing graph_relationships".to_string(),
                    reason: e.to_string(),
                })?;

            let rels: Vec<Relationship> = rows.iter().map(Self::row_to_relationship).collect();
            Ok(rels)
        }
    }

    fn find_incoming_relationships(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT id, from_symbol, to_symbol, relationship_type
                    FROM graph_relationships WHERE to_symbol = $1
                    ",
                    &[&symbol_id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT incoming graph_relationships".to_string(),
                    reason: e.to_string(),
                })?;

            let rels: Vec<Relationship> = rows.iter().map(Self::row_to_relationship).collect();
            Ok(rels)
        }
    }

    fn count_relationships(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_one("SELECT COUNT(*) FROM graph_relationships", &[])
                .await
                .map_err(|e| StorageError::Query {
                    statement: "COUNT graph_relationships".to_string(),
                    reason: e.to_string(),
                })?;
            let count: i64 = row.get(0);
            Ok(count)
        }
    }

    fn clear(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .batch_execute(
                    r"
                    DELETE FROM graph_relationships;
                    DELETE FROM graph_symbol_refs;
                    DELETE FROM graph_symbols;
                    DELETE FROM graph_files;
                    ",
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "CLEAR graph".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }
}

// ─────────────────────────────────────────────────────────
// WorkspaceRepository implementation
// ─────────────────────────────────────────────────────────

impl WorkspaceRepository for PostgresBackend {
    fn create_workspace(&self, workspace: &Workspace) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "INSERT INTO workspaces (id, name, default_project_id, created_at) VALUES ($1, $2, $3, $4)",
                    &[
                        &workspace.id.0 as &(dyn ToSql + Sync),
                        &workspace.name,
                        &workspace.default_project_id.as_ref().map(|pid| pid.0.clone()),
                        &workspace.created_at,
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT INTO workspaces".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn load_workspace(&self, id: &WorkspaceId) -> impl std::future::Future<Output = Result<Option<Workspace>>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_opt(
                    "SELECT id, name, default_project_id, created_at FROM workspaces WHERE id = $1",
                    &[&id.0 as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT workspaces".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(row.map(|r| Workspace {
                id: WorkspaceId(r.get(0)),
                name: r.get(1),
                default_project_id: r.get::<_, Option<String>>(2).map(ProjectId),
                created_at: r.get(3),
            }))
        }
    }

    fn list_workspaces(&self) -> impl std::future::Future<Output = Result<Vec<Workspace>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    "SELECT id, name, default_project_id, created_at FROM workspaces ORDER BY created_at DESC",
                    &[],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT workspaces list".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(rows
                .iter()
                .map(|r| Workspace {
                    id: WorkspaceId(r.get(0)),
                    name: r.get(1),
                    default_project_id: r.get::<_, Option<String>>(2).map(ProjectId),
                    created_at: r.get(3),
                })
                .collect())
        }
    }

    fn delete_workspace(&self, id: &WorkspaceId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            // workspace_projects rows cascade-deleted via ON DELETE CASCADE
            client
                .execute("DELETE FROM workspaces WHERE id = $1", &[&id.0 as &(dyn ToSql + Sync)])
                .await
                .map_err(|e| StorageError::Query {
                    statement: "DELETE FROM workspaces".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn add_project(&self, project: &Project) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "INSERT INTO projects (id, name, root_path, created_at) VALUES ($1, $2, $3, $4)",
                    &[
                        &project.id.0 as &(dyn ToSql + Sync),
                        &project.name,
                        &project.root_path.to_string_lossy().as_ref(),
                        &project.created_at,
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT INTO projects".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn load_project(&self, id: &ProjectId) -> impl std::future::Future<Output = Result<Option<Project>>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_opt(
                    "SELECT id, name, root_path, created_at FROM projects WHERE id = $1",
                    &[&id.0 as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT projects".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(row.map(|r| Project {
                id: ProjectId(r.get(0)),
                name: r.get(1),
                root_path: PathBuf::from(r.get::<_, String>(2)),
                created_at: r.get(3),
            }))
        }
    }

    fn load_project_by_path(&self, path: &Path) -> impl std::future::Future<Output = Result<Option<Project>>> + Send {
        async move {
            let client = self.get_client().await?;
            let path_str = path.to_string_lossy().to_string();
            let row = client
                .query_opt(
                    "SELECT id, name, root_path, created_at FROM projects WHERE root_path = $1",
                    &[&path_str as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT projects by path".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(row.map(|r| Project {
                id: ProjectId(r.get(0)),
                name: r.get(1),
                root_path: PathBuf::from(r.get::<_, String>(2)),
                created_at: r.get(3),
            }))
        }
    }

    fn list_projects(&self) -> impl std::future::Future<Output = Result<Vec<Project>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    "SELECT id, name, root_path, created_at FROM projects ORDER BY created_at DESC",
                    &[],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT projects list".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(rows
                .iter()
                .map(|r| Project {
                    id: ProjectId(r.get(0)),
                    name: r.get(1),
                    root_path: PathBuf::from(r.get::<_, String>(2)),
                    created_at: r.get(3),
                })
                .collect())
        }
    }

    fn update_project(&self, project: &Project) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "UPDATE projects SET name = $2, root_path = $3 WHERE id = $1",
                    &[
                        &project.id.0 as &(dyn ToSql + Sync),
                        &project.name,
                        &project.root_path.to_string_lossy().as_ref(),
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "UPDATE projects".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn remove_project(&self, id: &ProjectId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            // workspace_projects rows cascade-deleted via ON DELETE CASCADE
            client
                .execute("DELETE FROM projects WHERE id = $1", &[&id.0 as &(dyn ToSql + Sync)])
                .await
                .map_err(|e| StorageError::Query {
                    statement: "DELETE FROM projects".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn add_project_to_workspace(
        &self,
        workspace_id: &WorkspaceId,
        project_id: &ProjectId,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "INSERT INTO workspace_projects (workspace_id, project_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                    &[
                        &workspace_id.0 as &(dyn ToSql + Sync),
                        &project_id.0 as &(dyn ToSql + Sync),
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT INTO workspace_projects".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn remove_project_from_workspace(
        &self,
        workspace_id: &WorkspaceId,
        project_id: &ProjectId,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "DELETE FROM workspace_projects WHERE workspace_id = $1 AND project_id = $2",
                    &[
                        &workspace_id.0 as &(dyn ToSql + Sync),
                        &project_id.0 as &(dyn ToSql + Sync),
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "DELETE FROM workspace_projects".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn list_workspace_projects(
        &self,
        workspace_id: &WorkspaceId,
    ) -> impl std::future::Future<Output = Result<Vec<Project>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"SELECT p.id, p.name, p.root_path, p.created_at
                       FROM projects p
                       INNER JOIN workspace_projects wp ON wp.project_id = p.id
                       WHERE wp.workspace_id = $1
                       ORDER BY wp.added_at DESC",
                    &[&workspace_id.0 as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT workspace projects".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(rows
                .iter()
                .map(|r| Project {
                    id: ProjectId(r.get(0)),
                    name: r.get(1),
                    root_path: PathBuf::from(r.get::<_, String>(2)),
                    created_at: r.get(3),
                })
                .collect())
        }
    }

    fn set_default_project(
        &self,
        workspace_id: &WorkspaceId,
        project_id: &ProjectId,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "UPDATE workspaces SET default_project_id = $2 WHERE id = $1",
                    &[
                        &workspace_id.0 as &(dyn ToSql + Sync),
                        &project_id.0 as &(dyn ToSql + Sync),
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "UPDATE workspaces default_project_id".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn get_default_project(
        &self,
        workspace_id: &WorkspaceId,
    ) -> impl std::future::Future<Output = Result<Option<Project>>> + Send {
        async move {
            let client = self.get_client().await?;
            let default_id: Option<String> = client
                .query_opt(
                    "SELECT default_project_id FROM workspaces WHERE id = $1",
                    &[&workspace_id.0 as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT workspaces default_project_id".to_string(),
                    reason: e.to_string(),
                })?
                .and_then(|r| r.get(0));

            let Some(default_id) = default_id else {
                return Ok(None);
            };

            let row = client
                .query_opt(
                    "SELECT id, name, root_path, created_at FROM projects WHERE id = $1",
                    &[&default_id as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT projects by id".to_string(),
                    reason: e.to_string(),
                })?;

            Ok(row.map(|r| Project {
                id: ProjectId(r.get(0)),
                name: r.get(1),
                root_path: PathBuf::from(r.get::<_, String>(2)),
                created_at: r.get(3),
            }))
        }
    }
}

// ─────────────────────────────────────────────────────────
// StorageBackend implementation
// ─────────────────────────────────────────────────────────

impl StorageBackend for PostgresBackend {
    fn backend_type(&self) -> &'static str {
        "postgres"
    }

    fn migrate(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .batch_execute(INIT_SQL)
                .await
                .map_err(|e| StorageError::Migration {
                    reason: e.to_string(),
                })?;

            client
                .execute(
                    "INSERT INTO schema_version (version) VALUES ($1) ON CONFLICT (version) DO UPDATE SET version = EXCLUDED.version",
                    &[&SCHEMA_VERSION as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Migration {
                    reason: e.to_string(),
                })?;

            Ok(())
        }
    }

    fn health_check(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute("SELECT 1", &[])
                .await
                .map_err(|e| StorageError::Connection(format!("health check failed: {e}")))?;
            Ok(())
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
    #[ignore = "requires a running PostgreSQL instance"]
    async fn test_postgres_session_crud() {
        let backend = PostgresBackend::connect("host=localhost user=postgres dbname=clawdius_test")
            .await
            .unwrap();
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
    #[ignore = "requires a running PostgreSQL instance"]
    async fn test_postgres_save_message() {
        let backend = PostgresBackend::connect("host=localhost user=postgres dbname=clawdius_test")
            .await
            .unwrap();
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
    #[ignore = "requires a running PostgreSQL instance"]
    async fn test_postgres_search_messages() {
        let backend = PostgresBackend::connect("host=localhost user=postgres dbname=clawdius_test")
            .await
            .unwrap();
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
    #[ignore = "requires a running PostgreSQL instance"]
    async fn test_postgres_update_token_usage() {
        let backend = PostgresBackend::connect("host=localhost user=postgres dbname=clawdius_test")
            .await
            .unwrap();
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
    #[ignore = "requires a running PostgreSQL instance"]
    async fn test_postgres_checkpoint_crud() {
        let backend = PostgresBackend::connect("host=localhost user=postgres dbname=clawdius_test")
            .await
            .unwrap();
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
    #[ignore = "requires a running PostgreSQL instance"]
    async fn test_postgres_graph_file_crud() {
        let backend = PostgresBackend::connect("host=localhost user=postgres dbname=clawdius_test")
            .await
            .unwrap();
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
    #[ignore = "requires a running PostgreSQL instance"]
    async fn test_postgres_storage_backend_trait() {
        let backend = PostgresBackend::connect("host=localhost user=postgres dbname=clawdius_test")
            .await
            .unwrap();
        assert_eq!(backend.backend_type(), "postgres");
        backend.migrate().await.unwrap();
        backend.health_check().await.unwrap();
        backend.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires a running PostgreSQL instance"]
    async fn test_postgres_storage_stats() {
        let backend = PostgresBackend::connect("host=localhost user=postgres dbname=clawdius_test")
            .await
            .unwrap();
        backend.migrate().await.unwrap();
        let stats = backend.storage_stats().await.unwrap();
        assert_eq!(stats.checkpoint_count, 0);
        assert_eq!(stats.tracked_file_count, 0);
    }

    #[tokio::test]
    #[ignore = "requires a running PostgreSQL instance"]
    async fn test_postgres_track_file() {
        let backend = PostgresBackend::connect("host=localhost user=postgres dbname=clawdius_test")
            .await
            .unwrap();
        backend.migrate().await.unwrap();
        backend.track_file(&PathBuf::from("test.rs")).await.unwrap();
        assert_eq!(backend.tracked_file_count().await.unwrap(), 1);
    }

    #[tokio::test]
    #[ignore = "requires a running PostgreSQL instance"]
    async fn test_postgres_cleanup_old_checkpoints() {
        let backend = PostgresBackend::connect("host=localhost user=postgres dbname=clawdius_test")
            .await
            .unwrap();
        backend.migrate().await.unwrap();

        backend.create_checkpoint("cp1", None).await.unwrap();
        backend.create_checkpoint("cp2", None).await.unwrap();
        backend.create_checkpoint("cp3", None).await.unwrap();

        let deleted = backend.cleanup_old_checkpoints(1).await.unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(backend.checkpoint_count().await.unwrap(), 1);
    }

    #[tokio::test]
    #[ignore = "requires a running PostgreSQL instance"]
    async fn test_postgres_health_check() {
        let backend = PostgresBackend::connect("host=localhost user=postgres dbname=clawdius_test")
            .await
            .unwrap();
        backend.migrate().await.unwrap();
        backend.health_check().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires a running PostgreSQL instance"]
    async fn test_postgres_graph_clear() {
        let backend = PostgresBackend::connect("host=localhost user=postgres dbname=clawdius_test")
            .await
            .unwrap();
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
