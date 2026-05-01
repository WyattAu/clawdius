//! MariaDB storage backend implementation.
//!
//! Implements all three domain traits (`SessionRepository`, `TimelineRepository`,
//! `GraphRepository`) and the unified `StorageBackend` trait using `mysql_async`
//! with connection pooling.
//!
//! Enable with the `mariadb` feature flag.

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
use mysql_async::prelude::*;
use mysql_async::{Opts, Pool};
use std::path::{Path, PathBuf};
use uuid::Uuid;

// ─────────────────────────────────────────────────────────
// SQL Schema (MariaDB adaptation)
// ─────────────────────────────────────────────────────────

const SCHEMA_VERSION: i32 = 1;

const INIT_SQL: &str = r"
CREATE TABLE IF NOT EXISTS schema_version (
    version INT PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS sessions (
    id VARCHAR(36) PRIMARY KEY,
    title TEXT,
    provider TEXT,
    model TEXT,
    working_dir TEXT,
    tags TEXT NOT NULL DEFAULT '[]',
    extra TEXT NOT NULL DEFAULT '{}',
    input_tokens BIGINT NOT NULL DEFAULT 0,
    output_tokens BIGINT NOT NULL DEFAULT 0,
    cached_tokens BIGINT NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id VARCHAR(36) PRIMARY KEY,
    session_id VARCHAR(36) NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    tokens INT,
    tool_calls TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL,
    INDEX idx_messages_session_id (session_id),
    INDEX idx_messages_content (content(255))
);

CREATE INDEX IF NOT EXISTS idx_sessions_updated_at ON sessions(updated_at DESC);

CREATE TABLE IF NOT EXISTS tracked_files (
    id INT AUTO_INCREMENT PRIMARY KEY,
    path TEXT UNIQUE NOT NULL,
    tracked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS checkpoints (
    id VARCHAR(36) PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    timestamp TEXT NOT NULL,
    files_count INT NOT NULL DEFAULT 0,
    total_size BIGINT NOT NULL DEFAULT 0,
    INDEX idx_checkpoints_timestamp (timestamp DESC)
);

CREATE TABLE IF NOT EXISTS file_versions (
    id INT AUTO_INCREMENT PRIMARY KEY,
    path TEXT NOT NULL,
    version INT NOT NULL DEFAULT 1,
    checkpoint_id VARCHAR(36) NOT NULL,
    checksum TEXT NOT NULL,
    size BIGINT NOT NULL DEFAULT 0,
    timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_file_versions_path (path(255)),
    INDEX idx_file_versions_checkpoint (checkpoint_id)
);

CREATE TABLE IF NOT EXISTS projects (
    id VARCHAR(36) PRIMARY KEY,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS workspaces (
    id VARCHAR(36) PRIMARY KEY,
    name TEXT NOT NULL,
    default_project_id VARCHAR(36),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS workspace_projects (
    workspace_id VARCHAR(36) NOT NULL,
    project_id VARCHAR(36) NOT NULL,
    added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (workspace_id, project_id)
);

CREATE TABLE IF NOT EXISTS graph_files (
    id INT AUTO_INCREMENT PRIMARY KEY,
    path TEXT UNIQUE NOT NULL,
    hash TEXT NOT NULL,
    language TEXT,
    last_modified BIGINT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS graph_symbols (
    id INT AUTO_INCREMENT PRIMARY KEY,
    file_id INT,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    signature TEXT,
    doc_comment TEXT,
    start_line INT,
    end_line INT,
    start_col INT,
    end_col INT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_graph_symbols_name (name(255)),
    INDEX idx_graph_symbols_kind (kind(50)),
    INDEX idx_graph_symbols_file (file_id)
);

CREATE TABLE IF NOT EXISTS graph_symbol_refs (
    id INT AUTO_INCREMENT PRIMARY KEY,
    symbol_id INT,
    file_id INT,
    line INT,
    col INT,
    context TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_graph_refs_symbol (symbol_id),
    INDEX idx_graph_refs_file (file_id)
);

CREATE TABLE IF NOT EXISTS graph_relationships (
    id INT AUTO_INCREMENT PRIMARY KEY,
    from_symbol INT,
    to_symbol INT,
    relationship_type TEXT NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_graph_rels_from (from_symbol),
    INDEX idx_graph_rels_to (to_symbol),
    INDEX idx_graph_rels_type (relationship_type(50))
);
";

fn parse_dt(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc))
}

// ─────────────────────────────────────────────────────────
// MariaDbBackend
// ─────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct MariaDbBackend {
    pool: Pool,
}

impl MariaDbBackend {
    pub async fn connect(url: &str) -> Result<Self> {
        let opts = Opts::from_url(url)
            .map_err(|e| StorageError::Connection(format!("invalid MariaDB URL: {e}")))?;
        let pool = Pool::new(opts);
        Ok(Self { pool })
    }

    pub fn from_pool(pool: Pool) -> Self {
        Self { pool }
    }

    fn row_to_session(row: &mysql_async::Row) -> std::result::Result<Session, StorageError> {
        let id_str: String = row.get(0).unwrap_or_default();
        let title: Option<String> = row.get(1);
        let provider: Option<String> = row.get(2);
        let model: Option<String> = row.get(3);
        let working_dir: Option<String> = row.get(4);
        let tags_json: String = row.get(5).unwrap_or_else(|| "[]".to_string());
        let extra_json: String = row.get(6).unwrap_or_else(|| "{}".to_string());
        let input_tokens: i64 = row.get(7).unwrap_or(0);
        let output_tokens: i64 = row.get(8).unwrap_or(0);
        let cached_tokens: i64 = row.get(9).unwrap_or(0);
        let created_at_str: String = row.get(10).unwrap_or_default();
        let updated_at_str: String = row.get(11).unwrap_or_default();

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
            created_at: parse_dt(&created_at_str),
            updated_at: parse_dt(&updated_at_str),
        })
    }

    fn row_to_message(row: &mysql_async::Row) -> std::result::Result<Message, StorageError> {
        let id_str: String = row.get(0).unwrap_or_default();
        let _session_id: String = row.get(1).unwrap_or_default();
        let role_str: String = row.get(2).unwrap_or_else(|| "user".to_string());
        let content_json: String = row.get(3).unwrap_or_default();
        let tokens: Option<i32> = row.get(4);
        let tool_calls_json: Option<String> = row.get(5);
        let metadata_json: Option<String> = row.get(6);
        let created_at_str: String = row.get(7).unwrap_or_default();

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
            created_at: parse_dt(&created_at_str),
            tool_calls,
            metadata,
        })
    }

    fn row_to_checkpoint_info(row: &mysql_async::Row) -> CheckpointInfo {
        let id_str: String = row.get(0).unwrap_or_default();
        let name: String = row.get(1).unwrap_or_default();
        let description: Option<String> = row.get(2);
        let timestamp_str: String = row.get(3).unwrap_or_default();
        let files_count: i64 = row.get(4).unwrap_or(0);
        let total_size: i64 = row.get(5).unwrap_or(0);
        CheckpointInfo {
            id: CheckpointId::from_string(id_str),
            name,
            description,
            timestamp: parse_dt(&timestamp_str),
            files_count: files_count as usize,
            total_size: total_size as usize,
        }
    }

    fn row_to_file_version(row: &mysql_async::Row) -> FileVersion {
        let path: String = row.get(0).unwrap_or_default();
        let version: i64 = row.get(1).unwrap_or(1);
        let checkpoint_id: String = row.get(2).unwrap_or_default();
        let checksum: String = row.get(3).unwrap_or_default();
        let size: i64 = row.get(4).unwrap_or(0);
        let timestamp_str: String = row.get(5).unwrap_or_default();
        FileVersion {
            path: PathBuf::from(path),
            version: version as u64,
            timestamp: parse_dt(&timestamp_str),
            checksum,
            size: size as usize,
            checkpoint_id: CheckpointId::from_string(checkpoint_id),
        }
    }

    fn row_to_symbol(row: &mysql_async::Row) -> std::result::Result<Symbol, StorageError> {
        let kind_str: String = row.get(3).unwrap_or_default();
        let kind = Self::parse_symbol_kind(&kind_str);
        Ok(Symbol {
            id: row.get(0),
            file_id: row.get(1).unwrap_or(0),
            name: row.get(2).unwrap_or_default(),
            kind,
            signature: row.get(4),
            doc_comment: row.get(5),
            start_line: row.get(6).unwrap_or(0),
            end_line: row.get(7).unwrap_or(0),
            start_col: row.get(8).unwrap_or(0),
            end_col: row.get(9).unwrap_or(0),
        })
    }

    fn row_to_reference(row: &mysql_async::Row) -> Reference {
        Reference {
            id: row.get(0),
            symbol_id: row.get(1).unwrap_or(0),
            file_id: row.get(2).unwrap_or(0),
            line: row.get(3).unwrap_or(0),
            col: row.get(4).unwrap_or(0),
            context: row.get(5),
        }
    }

    fn row_to_relationship(row: &mysql_async::Row) -> Relationship {
        let rel_type_str: String = row.get(3).unwrap_or_default();
        let rel_type = Self::parse_relationship_type(&rel_type_str);
        Relationship {
            id: row.get(0),
            from_symbol: row.get(1).unwrap_or(0),
            to_symbol: row.get(2).unwrap_or(0),
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

impl SessionRepository for MariaDbBackend {
    fn create_session(&self, session: &Session) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let tags_json = serde_json::to_string(&session.meta.tags)?;
            let extra_json = serde_json::to_string(&session.meta.extra)?;

            conn.exec_drop(
                r"INSERT INTO sessions (
                    id, title, provider, model, working_dir, tags, extra,
                    input_tokens, output_tokens, cached_tokens,
                    created_at, updated_at
                ) VALUES (:id, :title, :provider, :model, :working_dir, :tags, :extra,
                    :input_tokens, :output_tokens, :cached_tokens, :created_at, :updated_at)",
                params! {
                    "id" => session.id.to_string(),
                    "title" => &session.title,
                    "provider" => &session.meta.provider,
                    "model" => &session.meta.model,
                    "working_dir" => &session.meta.working_dir,
                    "tags" => &tags_json,
                    "extra" => &extra_json,
                    "input_tokens" => session.token_usage.input as i64,
                    "output_tokens" => session.token_usage.output as i64,
                    "cached_tokens" => session.token_usage.cached as i64,
                    "created_at" => session.created_at.to_rfc3339(),
                    "updated_at" => session.updated_at.to_rfc3339(),
                },
            ).await.map_err(StorageError::from)?;

            Ok(())
        }
    }

    fn load_session(&self, id: &SessionId) -> impl std::future::Future<Output = Result<Option<Session>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let result = conn.exec_first(
                r"SELECT id, title, provider, model, working_dir, tags, extra,
                         input_tokens, output_tokens, cached_tokens,
                         created_at, updated_at
                  FROM sessions WHERE id = :id",
                params! { "id" => id.to_string() },
            ).await.map_err(StorageError::from)?;

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

            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let rows: Vec<mysql_async::Row> = conn.exec(
                r"SELECT id, session_id, role, content, tokens, tool_calls, metadata, created_at
                 FROM messages WHERE session_id = :session_id
                 ORDER BY created_at ASC",
                params! { "session_id" => id.to_string() },
            ).await.map_err(StorageError::from)?;

            let messages: Vec<Message> = rows
                .iter()
                .map(|row| Self::row_to_message(row))
                .collect::<std::result::Result<Vec<_>, _>>()?;

            session.messages = messages;
            Ok(Some(session))
        }
    }

    fn list_sessions(&self) -> impl std::future::Future<Output = Result<Vec<Session>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let rows: Vec<mysql_async::Row> = conn.query(
                r"SELECT id, title, provider, model, working_dir, tags, extra,
                         input_tokens, output_tokens, cached_tokens,
                         created_at, updated_at
                  FROM sessions
                  ORDER BY updated_at DESC",
            ).await.map_err(StorageError::from)?;

            let sessions: Vec<Session> = rows
                .iter()
                .map(|row| Self::row_to_session(row))
                .collect::<std::result::Result<Vec<_>, _>>()?;

            Ok(sessions)
        }
    }

    fn delete_session(&self, id: &SessionId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.exec_drop(
                "DELETE FROM messages WHERE session_id = :session_id",
                params! { "session_id" => id.to_string() },
            ).await.map_err(StorageError::from)?;
            conn.exec_drop(
                "DELETE FROM sessions WHERE id = :id",
                params! { "id" => id.to_string() },
            ).await.map_err(StorageError::from)?;
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

            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.exec_drop(
                r"INSERT INTO messages (
                    id, session_id, role, content, tokens, tool_calls, metadata, created_at
                ) VALUES (:id, :session_id, :role, :content, :tokens, :tool_calls, :metadata, :created_at)",
                params! {
                    "id" => message.id.to_string(),
                    "session_id" => session_id.to_string(),
                    "role" => message.role.as_str(),
                    "content" => &content_json,
                    "tokens" => message.tokens.map(|t| t as i32),
                    "tool_calls" => &tool_calls_json,
                    "metadata" => &metadata_json,
                    "created_at" => message.created_at.to_rfc3339(),
                },
            ).await.map_err(StorageError::from)?;

            conn.exec_drop(
                "UPDATE sessions SET updated_at = :now WHERE id = :id",
                params! {
                    "now" => Utc::now().to_rfc3339(),
                    "id" => session_id.to_string(),
                },
            ).await.map_err(StorageError::from)?;

            Ok(())
        }
    }

    fn search_messages(&self, query: &str) -> impl std::future::Future<Output = Result<Vec<(SessionId, Message)>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let pattern = format!("%{query}%");
            let rows: Vec<mysql_async::Row> = conn.exec(
                r"SELECT m.id, m.session_id, m.role, m.content, m.tokens, m.tool_calls, m.metadata, m.created_at
                 FROM messages m
                 WHERE m.content LIKE :pattern
                 ORDER BY m.created_at DESC
                 LIMIT 100",
                params! { "pattern" => &pattern },
            ).await.map_err(StorageError::from)?;

            let mut results = Vec::new();
            for row in &rows {
                let message = Self::row_to_message(row)?;
                let session_id_str: String = row.get(1).unwrap_or_default();
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
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.exec_drop(
                r"UPDATE sessions SET
                    input_tokens = :input_tokens,
                    output_tokens = :output_tokens,
                    cached_tokens = :cached_tokens,
                    updated_at = :updated_at
                  WHERE id = :id",
                params! {
                    "input_tokens" => usage.input as i64,
                    "output_tokens" => usage.output as i64,
                    "cached_tokens" => usage.cached as i64,
                    "updated_at" => Utc::now().to_rfc3339(),
                    "id" => id.to_string(),
                },
            ).await.map_err(StorageError::from)?;
            Ok(())
        }
    }
}

// ─────────────────────────────────────────────────────────
// TimelineRepository implementation
// ─────────────────────────────────────────────────────────

impl TimelineRepository for MariaDbBackend {
    fn track_file(&self, path: &Path) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let path_str = path.to_string_lossy().to_string();
            conn.exec_drop(
                r"INSERT IGNORE INTO tracked_files (path) VALUES (:path)",
                params! { "path" => &path_str },
            ).await.map_err(StorageError::from)?;
            Ok(())
        }
    }

    fn tracked_file_count(&self) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let result: Option<i64> = conn.query_first("SELECT COUNT(*) FROM tracked_files")
                .await.map_err(StorageError::from)?;
            Ok(result.unwrap_or(0) as usize)
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
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.exec_drop(
                r"INSERT INTO checkpoints (id, name, description, timestamp, files_count, total_size)
                  VALUES (:id, :name, :description, :timestamp, 0, 0)",
                params! {
                    "id" => id.0.clone(),
                    "name" => name,
                    "description" => description,
                    "timestamp" => now,
                },
            ).await.map_err(StorageError::from)?;
            Ok(id)
        }
    }

    fn list_checkpoints(&self) -> impl std::future::Future<Output = Result<Vec<CheckpointInfo>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let rows: Vec<mysql_async::Row> = conn.query(
                r"SELECT id, name, description, timestamp, files_count, total_size
                  FROM checkpoints ORDER BY timestamp DESC",
            ).await.map_err(StorageError::from)?;

            let checkpoints: Vec<CheckpointInfo> = rows.iter().map(Self::row_to_checkpoint_info).collect();
            Ok(checkpoints)
        }
    }

    fn get_checkpoint(&self, id: &CheckpointId) -> impl std::future::Future<Output = Result<Option<CheckpointInfo>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let result: Option<mysql_async::Row> = conn.exec_first(
                r"SELECT id, name, description, timestamp, files_count, total_size
                  FROM checkpoints WHERE id = :id",
                params! { "id" => &id.0 },
            ).await.map_err(StorageError::from)?;

            Ok(result.map(|r| Self::row_to_checkpoint_info(&r)))
        }
    }

    fn delete_checkpoint(&self, id: &CheckpointId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.exec_drop(
                "DELETE FROM file_versions WHERE checkpoint_id = :id",
                params! { "id" => &id.0 },
            ).await.map_err(StorageError::from)?;
            conn.exec_drop(
                "DELETE FROM checkpoints WHERE id = :id",
                params! { "id" => &id.0 },
            ).await.map_err(StorageError::from)?;
            Ok(())
        }
    }

    fn checkpoint_count(&self) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let result: Option<i64> = conn.query_first("SELECT COUNT(*) FROM checkpoints")
                .await.map_err(StorageError::from)?;
            Ok(result.unwrap_or(0) as usize)
        }
    }

    fn get_file_history(&self, path: &Path) -> impl std::future::Future<Output = Result<Vec<FileVersion>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let path_str = path.to_string_lossy().to_string();
            let rows: Vec<mysql_async::Row> = conn.exec(
                r"SELECT path, version, checkpoint_id, checksum, size, timestamp
                  FROM file_versions WHERE path = :path
                  ORDER BY timestamp DESC",
                params! { "path" => &path_str },
            ).await.map_err(StorageError::from)?;

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
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let path_str = path.to_string_lossy().to_string();
            let result: Option<mysql_async::Row> = conn.exec_first(
                r"SELECT path, version, checkpoint_id, checksum, size, timestamp
                  FROM file_versions WHERE path = :path AND checkpoint_id = :checkpoint_id
                  ORDER BY version DESC LIMIT 1",
                params! {
                    "path" => &path_str,
                    "checkpoint_id" => &checkpoint_id.0,
                },
            ).await.map_err(StorageError::from)?;

            Ok(result.map(|r| Self::row_to_file_version(&r)))
        }
    }

    fn get_files_changed_between(
        &self,
        from: &CheckpointId,
        to: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<Vec<(PathBuf, FileChangeType)>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let rows: Vec<mysql_async::Row> = conn.exec(
                r"SELECT path,
                          CASE
                              WHEN NOT EXISTS (SELECT 1 FROM file_versions fv1 WHERE fv1.path = fv.path AND fv1.checkpoint_id = :from_id)
                              THEN 'added'
                              WHEN NOT EXISTS (SELECT 1 FROM file_versions fv2 WHERE fv2.path = fv.path AND fv2.checkpoint_id = :to_id)
                              THEN 'deleted'
                              ELSE 'modified'
                          END as change_type
                   FROM file_versions fv
                   WHERE (checkpoint_id = :from_id OR checkpoint_id = :to_id)
                   GROUP BY path",
                params! {
                    "from_id" => &from.0,
                    "to_id" => &to.0,
                },
            ).await.map_err(StorageError::from)?;

            let mut changes = Vec::new();
            for row in &rows {
                let path: String = row.get(0).unwrap_or_default();
                let change_type_str: String = row.get(1).unwrap_or_else(|| "modified".to_string());
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
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let rows: Vec<mysql_async::Row> = conn.exec(
                r"SELECT id, name, description, timestamp, files_count, total_size
                  FROM checkpoints WHERE timestamp >= :start AND timestamp <= :end
                  ORDER BY timestamp DESC",
                params! {
                    "start" => start.to_rfc3339(),
                    "end" => end.to_rfc3339(),
                },
            ).await.map_err(StorageError::from)?;

            let checkpoints: Vec<CheckpointInfo> = rows.iter().map(Self::row_to_checkpoint_info).collect();
            Ok(checkpoints)
        }
    }

    fn query_by_name(&self, pattern: &str) -> impl std::future::Future<Output = Result<Vec<CheckpointInfo>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let like_pattern = format!("%{pattern}%");
            let rows: Vec<mysql_async::Row> = conn.exec(
                r"SELECT id, name, description, timestamp, files_count, total_size
                  FROM checkpoints WHERE name LIKE :pattern
                  ORDER BY timestamp DESC",
                params! { "pattern" => &like_pattern },
            ).await.map_err(StorageError::from)?;

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

            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let rows: Vec<mysql_async::Row> = conn.exec(
                r"SELECT path, checksum, size FROM file_versions WHERE checkpoint_id = :checkpoint_id",
                params! { "checkpoint_id" => &checkpoint_id.0 },
            ).await.map_err(StorageError::from)?;

            let files: Vec<ExportedFile> = rows
                .iter()
                .map(|row| {
                    let path: String = row.get(0).unwrap_or_default();
                    let hash: String = row.get(1).unwrap_or_default();
                    ExportedFile {
                        path: PathBuf::from(path),
                        content: String::new(),
                        is_binary: false,
                        hash,
                    }
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
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let result: Option<i64> = conn.query_first("SELECT COUNT(*) FROM checkpoints")
                .await.map_err(StorageError::from)?;
            let count = result.unwrap_or(0);

            if count as usize <= keep_count {
                return Ok(0);
            }

            let keep = keep_count as i64;
            let rows: Vec<mysql_async::Row> = conn.exec(
                r"SELECT id FROM checkpoints ORDER BY timestamp DESC LIMIT :limit",
                params! { "limit" => keep },
            ).await.map_err(StorageError::from)?;

            if rows.is_empty() {
                return Ok(0);
            }

            let ids: Vec<String> = rows
                .iter()
                .map(|r| {
                    let s: String = r.get(0).unwrap_or_default();
                    s
                })
                .filter(|s: &String| !s.is_empty())
                .collect();

            if ids.is_empty() {
                return Ok(0);
            }

            let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let delete_sql = format!(
                "DELETE FROM file_versions WHERE checkpoint_id NOT IN ({placeholders})"
            );
            let vals: Vec<mysql_async::Value> = ids.iter().map(|id: &String| mysql_async::Value::from(id.as_str())).collect();
            conn.exec_drop(delete_sql, vals.clone()).await.map_err(StorageError::from)?;

            let delete_sql2 = format!(
                "DELETE FROM checkpoints WHERE id NOT IN ({placeholders})"
            );
            let deleted = conn.exec_iter(delete_sql2, vals)
                .await.map_err(StorageError::from)?
                .affected_rows();
            Ok(deleted as usize)
        }
    }

    fn cleanup_snapshots(&self) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            Ok(0)
        }
    }

    fn storage_stats(&self) -> impl std::future::Future<Output = Result<StorageStats>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let checkpoint_count: Option<i64> = conn.query_first("SELECT COUNT(*) FROM checkpoints")
                .await.map_err(StorageError::from)?;
            let tracked_file_count: Option<i64> = conn.query_first("SELECT COUNT(*) FROM tracked_files")
                .await.map_err(StorageError::from)?;
            let version_count: Option<i64> = conn.query_first("SELECT COUNT(*) FROM file_versions")
                .await.map_err(StorageError::from)?;

            Ok(StorageStats {
                checkpoint_count: checkpoint_count.unwrap_or(0) as usize,
                tracked_file_count: tracked_file_count.unwrap_or(0) as usize,
                total_size_bytes: 0,
                version_count: version_count.unwrap_or(0) as usize,
            })
        }
    }
}

// ─────────────────────────────────────────────────────────
// GraphRepository implementation
// ─────────────────────────────────────────────────────────

impl GraphRepository for MariaDbBackend {
    fn insert_file(&self, file: &FileInfo) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let existing: Option<i64> = conn.exec_first(
                "SELECT id FROM graph_files WHERE path = :path",
                params! { "path" => &file.path },
            ).await.map_err(StorageError::from)?;

            if let Some(id) = existing {
                conn.exec_drop(
                    r"UPDATE graph_files SET hash = :hash, language = :language, last_modified = :last_modified WHERE id = :id",
                    params! {
                        "hash" => &file.hash,
                        "language" => &file.language,
                        "last_modified" => file.last_modified,
                        "id" => id,
                    },
                ).await.map_err(StorageError::from)?;
                return Ok(id);
            }

            conn.exec_drop(
                r"INSERT INTO graph_files (path, hash, language, last_modified) VALUES (:path, :hash, :language, :last_modified)",
                params! {
                    "path" => &file.path,
                    "hash" => &file.hash,
                    "language" => &file.language,
                    "last_modified" => file.last_modified,
                },
            ).await.map_err(StorageError::from)?;

            let id: Option<i64> = conn.query_first("SELECT LAST_INSERT_ID()")
                .await.map_err(StorageError::from)?;
            Ok(id.unwrap_or(0))
        }
    }

    fn get_file_by_path(&self, path: &str) -> impl std::future::Future<Output = Result<Option<FileInfo>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let result: Option<mysql_async::Row> = conn.exec_first(
                r"SELECT id, path, hash, language, last_modified, created_at
                  FROM graph_files WHERE path = :path",
                params! { "path" => path },
            ).await.map_err(StorageError::from)?;

            match result {
                Some(r) => Ok(Some(FileInfo {
                    path: r.get(1).unwrap_or_default(),
                    hash: r.get(2).unwrap_or_default(),
                    language: r.get(3),
                    last_modified: r.get(4),
                })),
                None => Ok(None),
            }
        }
    }

    fn get_file_by_id(&self, id: i64) -> impl std::future::Future<Output = Result<Option<FileInfo>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let result: Option<mysql_async::Row> = conn.exec_first(
                r"SELECT id, path, hash, language, last_modified, created_at
                  FROM graph_files WHERE id = :id",
                params! { "id" => id },
            ).await.map_err(StorageError::from)?;

            match result {
                Some(r) => Ok(Some(FileInfo {
                    path: r.get(1).unwrap_or_default(),
                    hash: r.get(2).unwrap_or_default(),
                    language: r.get(3),
                    last_modified: r.get(4),
                })),
                None => Ok(None),
            }
        }
    }

    fn get_file_id(&self, path: &str) -> impl std::future::Future<Output = Result<Option<i64>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let result: Option<(i64,)> = conn.exec_first(
                "SELECT id FROM graph_files WHERE path = :path",
                params! { "path" => path },
            ).await.map_err(StorageError::from)?;

            Ok(result.map(|r| r.0))
        }
    }

    fn delete_file(&self, path: &str) -> impl std::future::Future<Output = Result<bool>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let file_id: Option<i64> = conn.exec_first(
                "SELECT id FROM graph_files WHERE path = :path",
                params! { "path" => path },
            ).await.map_err(StorageError::from)?;

            let Some(fid) = file_id else {
                return Ok(false);
            };

            conn.exec_drop(
                "DELETE FROM graph_symbol_refs WHERE file_id = :file_id",
                params! { "file_id" => fid },
            ).await.map_err(StorageError::from)?;
            conn.exec_drop(
                "DELETE FROM graph_symbols WHERE file_id = :file_id",
                params! { "file_id" => fid },
            ).await.map_err(StorageError::from)?;
            conn.exec_drop(
                "DELETE FROM graph_files WHERE path = :path",
                params! { "path" => path },
            ).await.map_err(StorageError::from)?;

            Ok(true)
        }
    }

    fn count_files(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let result: Option<i64> = conn.query_first("SELECT COUNT(*) FROM graph_files")
                .await.map_err(StorageError::from)?;
            Ok(result.unwrap_or(0))
        }
    }

    fn insert_symbol(&self, symbol: &Symbol) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.exec_drop(
                r"INSERT INTO graph_symbols (file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col)
                  VALUES (:file_id, :name, :kind, :signature, :doc_comment, :start_line, :end_line, :start_col, :end_col)",
                params! {
                    "file_id" => symbol.file_id,
                    "name" => &symbol.name,
                    "kind" => format!("{:?}", symbol.kind),
                    "signature" => &symbol.signature,
                    "doc_comment" => &symbol.doc_comment,
                    "start_line" => symbol.start_line,
                    "end_line" => symbol.end_line,
                    "start_col" => symbol.start_col,
                    "end_col" => symbol.end_col,
                },
            ).await.map_err(StorageError::from)?;

            let id: Option<i64> = conn.query_first("SELECT LAST_INSERT_ID()")
                .await.map_err(StorageError::from)?;
            Ok(id.unwrap_or(0))
        }
    }

    fn find_symbol(&self, name: &str) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let rows: Vec<mysql_async::Row> = conn.exec(
                r"SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                  FROM graph_symbols WHERE name = :name",
                params! { "name" => name },
            ).await.map_err(StorageError::from)?;

            let symbols: Vec<Symbol> = rows
                .iter()
                .map(|row| Self::row_to_symbol(row))
                .collect::<std::result::Result<Vec<_>, _>>()?;

            Ok(symbols)
        }
    }

    fn find_symbol_by_id(&self, id: i64) -> impl std::future::Future<Output = Result<Option<Symbol>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let result: Option<mysql_async::Row> = conn.exec_first(
                r"SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                  FROM graph_symbols WHERE id = :id",
                params! { "id" => id },
            ).await.map_err(StorageError::from)?;

            match result {
                Some(r) => Ok(Some(Self::row_to_symbol(&r)?)),
                None => Ok(None),
            }
        }
    }

    fn find_symbols_by_kind(&self, kind: &SymbolKind) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let kind_str = format!("{:?}", kind);
            let rows: Vec<mysql_async::Row> = conn.exec(
                r"SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                  FROM graph_symbols WHERE kind = :kind",
                params! { "kind" => &kind_str },
            ).await.map_err(StorageError::from)?;

            let symbols: Vec<Symbol> = rows
                .iter()
                .map(|row| Self::row_to_symbol(row))
                .collect::<std::result::Result<Vec<_>, _>>()?;

            Ok(symbols)
        }
    }

    fn find_symbols_in_file(&self, file_id: i64) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let rows: Vec<mysql_async::Row> = conn.exec(
                r"SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                  FROM graph_symbols WHERE file_id = :file_id",
                params! { "file_id" => file_id },
            ).await.map_err(StorageError::from)?;

            let symbols: Vec<Symbol> = rows
                .iter()
                .map(|row| Self::row_to_symbol(row))
                .collect::<std::result::Result<Vec<_>, _>>()?;

            Ok(symbols)
        }
    }

    fn search_symbols(&self, query: &str) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let pattern = format!("%{query}%");
            let rows: Vec<mysql_async::Row> = conn.exec(
                r"SELECT id, file_id, name, kind, signature, doc_comment, start_line, end_line, start_col, end_col
                  FROM graph_symbols WHERE name LIKE :pattern
                  LIMIT 100",
                params! { "pattern" => &pattern },
            ).await.map_err(StorageError::from)?;

            let symbols: Vec<Symbol> = rows
                .iter()
                .map(|row| Self::row_to_symbol(row))
                .collect::<std::result::Result<Vec<_>, _>>()?;

            Ok(symbols)
        }
    }

    fn count_symbols(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let result: Option<i64> = conn.query_first("SELECT COUNT(*) FROM graph_symbols")
                .await.map_err(StorageError::from)?;
            Ok(result.unwrap_or(0))
        }
    }

    fn delete_symbols_for_file(&self, file_id: i64) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.exec_drop(
                "DELETE FROM graph_symbol_refs WHERE symbol_id IN (SELECT id FROM graph_symbols WHERE file_id = :file_id)",
                params! { "file_id" => file_id },
            ).await.map_err(StorageError::from)?;
            conn.exec_drop(
                "DELETE FROM graph_symbols WHERE file_id = :file_id",
                params! { "file_id" => file_id },
            ).await.map_err(StorageError::from)?;
            Ok(())
        }
    }

    fn insert_reference(&self, reference: &Reference) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.exec_drop(
                r"INSERT INTO graph_symbol_refs (symbol_id, file_id, line, col, context)
                  VALUES (:symbol_id, :file_id, :line, :col, :context)",
                params! {
                    "symbol_id" => reference.symbol_id,
                    "file_id" => reference.file_id,
                    "line" => reference.line,
                    "col" => reference.col,
                    "context" => &reference.context,
                },
            ).await.map_err(StorageError::from)?;
            Ok(())
        }
    }

    fn find_symbol_refs(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Reference>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let rows: Vec<mysql_async::Row> = conn.exec(
                r"SELECT id, symbol_id, file_id, line, col, context
                  FROM graph_symbol_refs WHERE symbol_id = :symbol_id",
                params! { "symbol_id" => symbol_id },
            ).await.map_err(StorageError::from)?;

            let refs: Vec<Reference> = rows.iter().map(Self::row_to_reference).collect();
            Ok(refs)
        }
    }

    fn count_symbol_refs(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let result: Option<i64> = conn.query_first("SELECT COUNT(*) FROM graph_symbol_refs")
                .await.map_err(StorageError::from)?;
            Ok(result.unwrap_or(0))
        }
    }

    fn delete_symbol_refs_for_file(&self, file_id: i64) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.exec_drop(
                "DELETE FROM graph_symbol_refs WHERE file_id = :file_id",
                params! { "file_id" => file_id },
            ).await.map_err(StorageError::from)?;
            Ok(())
        }
    }

    fn insert_relationship(&self, relationship: &Relationship) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.exec_drop(
                r"INSERT INTO graph_relationships (from_symbol, to_symbol, relationship_type)
                  VALUES (:from_symbol, :to_symbol, :relationship_type)",
                params! {
                    "from_symbol" => relationship.from_symbol,
                    "to_symbol" => relationship.to_symbol,
                    "relationship_type" => format!("{:?}", relationship.relationship_type),
                },
            ).await.map_err(StorageError::from)?;
            Ok(())
        }
    }

    fn find_relationships(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let rows: Vec<mysql_async::Row> = conn.exec(
                r"SELECT id, from_symbol, to_symbol, relationship_type
                  FROM graph_relationships WHERE from_symbol = :symbol_id OR to_symbol = :symbol_id",
                params! { "symbol_id" => symbol_id },
            ).await.map_err(StorageError::from)?;

            let rels: Vec<Relationship> = rows.iter().map(Self::row_to_relationship).collect();
            Ok(rels)
        }
    }

    fn find_outgoing_relationships(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let rows: Vec<mysql_async::Row> = conn.exec(
                r"SELECT id, from_symbol, to_symbol, relationship_type
                  FROM graph_relationships WHERE from_symbol = :symbol_id",
                params! { "symbol_id" => symbol_id },
            ).await.map_err(StorageError::from)?;

            let rels: Vec<Relationship> = rows.iter().map(Self::row_to_relationship).collect();
            Ok(rels)
        }
    }

    fn find_incoming_relationships(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let rows: Vec<mysql_async::Row> = conn.exec(
                r"SELECT id, from_symbol, to_symbol, relationship_type
                  FROM graph_relationships WHERE to_symbol = :symbol_id",
                params! { "symbol_id" => symbol_id },
            ).await.map_err(StorageError::from)?;

            let rels: Vec<Relationship> = rows.iter().map(Self::row_to_relationship).collect();
            Ok(rels)
        }
    }

    fn count_relationships(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let result: Option<i64> = conn.query_first("SELECT COUNT(*) FROM graph_relationships")
                .await.map_err(StorageError::from)?;
            Ok(result.unwrap_or(0))
        }
    }

    fn clear(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.query_drop("DELETE FROM graph_relationships")
                .await.map_err(StorageError::from)?;
            conn.query_drop("DELETE FROM graph_symbol_refs")
                .await.map_err(StorageError::from)?;
            conn.query_drop("DELETE FROM graph_symbols")
                .await.map_err(StorageError::from)?;
            conn.query_drop("DELETE FROM graph_files")
                .await.map_err(StorageError::from)?;
            Ok(())
        }
    }
}

// ─────────────────────────────────────────────────────────
// WorkspaceRepository implementation
// ─────────────────────────────────────────────────────────

impl WorkspaceRepository for MariaDbBackend {
    fn create_workspace(&self, workspace: &Workspace) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.exec_drop(
                r"INSERT INTO workspaces (id, name, default_project_id, created_at)
                  VALUES (:id, :name, :default_project_id, :created_at)",
                params! {
                    "id" => &workspace.id.0,
                    "name" => &workspace.name,
                    "default_project_id" => workspace.default_project_id.as_ref().map(|pid| &pid.0),
                    "created_at" => workspace.created_at.to_rfc3339(),
                },
            ).await.map_err(StorageError::from)?;
            Ok(())
        }
    }

    fn load_workspace(&self, id: &WorkspaceId) -> impl std::future::Future<Output = Result<Option<Workspace>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let row: Option<(String, String, Option<String>, String)> = conn.exec_first(
                "SELECT id, name, default_project_id, created_at FROM workspaces WHERE id = :id",
                params! { "id" => &id.0 },
            ).await.map_err(StorageError::from)?;
            Ok(row.map(|(id, name, default_project_id, created_at)| Workspace {
                id: WorkspaceId(id),
                name,
                default_project_id: default_project_id.map(ProjectId),
                created_at: created_at.parse::<DateTime<Utc>>().unwrap_or_else(|_| Utc::now()),
            }))
        }
    }

    fn list_workspaces(&self) -> impl std::future::Future<Output = Result<Vec<Workspace>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let rows: Vec<(String, String, Option<String>, String)> = conn.query(
                "SELECT id, name, default_project_id, created_at FROM workspaces ORDER BY created_at DESC",
            ).await.map_err(StorageError::from)?;
            Ok(rows
                .into_iter()
                .map(|(id, name, default_project_id, created_at)| Workspace {
                    id: WorkspaceId(id),
                    name,
                    default_project_id: default_project_id.map(ProjectId),
                    created_at: created_at.parse::<DateTime<Utc>>().unwrap_or_else(|_| Utc::now()),
                })
                .collect())
        }
    }

    fn delete_workspace(&self, id: &WorkspaceId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.exec_drop(
                "DELETE FROM workspace_projects WHERE workspace_id = :id",
                params! { "id" => &id.0 },
            ).await.map_err(StorageError::from)?;
            conn.exec_drop(
                "DELETE FROM workspaces WHERE id = :id",
                params! { "id" => &id.0 },
            ).await.map_err(StorageError::from)?;
            Ok(())
        }
    }

    fn add_project(&self, project: &Project) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.exec_drop(
                r"INSERT INTO projects (id, name, root_path, created_at)
                  VALUES (:id, :name, :root_path, :created_at)",
                params! {
                    "id" => &project.id.0,
                    "name" => &project.name,
                    "root_path" => project.root_path.to_string_lossy().as_ref(),
                    "created_at" => project.created_at.to_rfc3339(),
                },
            ).await.map_err(StorageError::from)?;
            Ok(())
        }
    }

    fn load_project(&self, id: &ProjectId) -> impl std::future::Future<Output = Result<Option<Project>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let row: Option<(String, String, String, String)> = conn.exec_first(
                "SELECT id, name, root_path, created_at FROM projects WHERE id = :id",
                params! { "id" => &id.0 },
            ).await.map_err(StorageError::from)?;
            Ok(row.map(|(id, name, root_path, created_at)| Project {
                id: ProjectId(id),
                name,
                root_path: PathBuf::from(root_path),
                created_at: created_at.parse::<DateTime<Utc>>().unwrap_or_else(|_| Utc::now()),
            }))
        }
    }

    fn load_project_by_path(&self, path: &Path) -> impl std::future::Future<Output = Result<Option<Project>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let path_str = path.to_string_lossy().to_string();
            let row: Option<(String, String, String, String)> = conn.exec_first(
                "SELECT id, name, root_path, created_at FROM projects WHERE root_path = :root_path",
                params! { "root_path" => &path_str },
            ).await.map_err(StorageError::from)?;
            Ok(row.map(|(id, name, root_path, created_at)| Project {
                id: ProjectId(id),
                name,
                root_path: PathBuf::from(root_path),
                created_at: created_at.parse::<DateTime<Utc>>().unwrap_or_else(|_| Utc::now()),
            }))
        }
    }

    fn list_projects(&self) -> impl std::future::Future<Output = Result<Vec<Project>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let rows: Vec<(String, String, String, String)> = conn.query(
                "SELECT id, name, root_path, created_at FROM projects ORDER BY created_at DESC",
            ).await.map_err(StorageError::from)?;
            Ok(rows
                .into_iter()
                .map(|(id, name, root_path, created_at)| Project {
                    id: ProjectId(id),
                    name,
                    root_path: PathBuf::from(root_path),
                    created_at: created_at.parse::<DateTime<Utc>>().unwrap_or_else(|_| Utc::now()),
                })
                .collect())
        }
    }

    fn update_project(&self, project: &Project) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.exec_drop(
                r"UPDATE projects SET name = :name, root_path = :root_path WHERE id = :id",
                params! {
                    "id" => &project.id.0,
                    "name" => &project.name,
                    "root_path" => project.root_path.to_string_lossy().as_ref(),
                },
            ).await.map_err(StorageError::from)?;
            Ok(())
        }
    }

    fn remove_project(&self, id: &ProjectId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.exec_drop(
                "DELETE FROM workspace_projects WHERE project_id = :id",
                params! { "id" => &id.0 },
            ).await.map_err(StorageError::from)?;
            conn.exec_drop(
                "DELETE FROM projects WHERE id = :id",
                params! { "id" => &id.0 },
            ).await.map_err(StorageError::from)?;
            Ok(())
        }
    }

    fn add_project_to_workspace(
        &self,
        workspace_id: &WorkspaceId,
        project_id: &ProjectId,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.exec_drop(
                r"INSERT IGNORE INTO workspace_projects (workspace_id, project_id)
                  VALUES (:workspace_id, :project_id)",
                params! {
                    "workspace_id" => &workspace_id.0,
                    "project_id" => &project_id.0,
                },
            ).await.map_err(StorageError::from)?;
            Ok(())
        }
    }

    fn remove_project_from_workspace(
        &self,
        workspace_id: &WorkspaceId,
        project_id: &ProjectId,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.exec_drop(
                r"DELETE FROM workspace_projects WHERE workspace_id = :workspace_id AND project_id = :project_id",
                params! {
                    "workspace_id" => &workspace_id.0,
                    "project_id" => &project_id.0,
                },
            ).await.map_err(StorageError::from)?;
            Ok(())
        }
    }

    fn list_workspace_projects(
        &self,
        workspace_id: &WorkspaceId,
    ) -> impl std::future::Future<Output = Result<Vec<Project>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let rows: Vec<(String, String, String, String)> = conn.exec(
                r"SELECT p.id, p.name, p.root_path, p.created_at
                   FROM projects p
                   INNER JOIN workspace_projects wp ON wp.project_id = p.id
                   WHERE wp.workspace_id = :workspace_id
                   ORDER BY wp.added_at DESC",
                params! { "workspace_id" => &workspace_id.0 },
            ).await.map_err(StorageError::from)?;
            Ok(rows
                .into_iter()
                .map(|(id, name, root_path, created_at)| Project {
                    id: ProjectId(id),
                    name,
                    root_path: PathBuf::from(root_path),
                    created_at: created_at.parse::<DateTime<Utc>>().unwrap_or_else(|_| Utc::now()),
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
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            conn.exec_drop(
                r"UPDATE workspaces SET default_project_id = :project_id WHERE id = :workspace_id",
                params! {
                    "workspace_id" => &workspace_id.0,
                    "project_id" => &project_id.0,
                },
            ).await.map_err(StorageError::from)?;
            Ok(())
        }
    }

    fn get_default_project(
        &self,
        workspace_id: &WorkspaceId,
    ) -> impl std::future::Future<Output = Result<Option<Project>>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let default_id: Option<String> = conn.exec_first(
                "SELECT default_project_id FROM workspaces WHERE id = :id",
                params! { "id" => &workspace_id.0 },
            ).await.map_err(StorageError::from)?.flatten();

            let Some(default_id) = default_id else {
                return Ok(None);
            };

            let row: Option<(String, String, String, String)> = conn.exec_first(
                "SELECT id, name, root_path, created_at FROM projects WHERE id = :id",
                params! { "id" => &default_id },
            ).await.map_err(StorageError::from)?;

            Ok(row.map(|(id, name, root_path, created_at)| Project {
                id: ProjectId(id),
                name,
                root_path: PathBuf::from(root_path),
                created_at: created_at.parse::<DateTime<Utc>>().unwrap_or_else(|_| Utc::now()),
            }))
        }
    }
}

// ─────────────────────────────────────────────────────────
// StorageBackend implementation
// ─────────────────────────────────────────────────────────

impl StorageBackend for MariaDbBackend {
    fn backend_type(&self) -> &'static str {
        "mariadb"
    }

    fn migrate(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;

            for stmt in INIT_SQL.split(';') {
                let trimmed = stmt.trim();
                if trimmed.is_empty() {
                    continue;
                }
                conn.query_drop(trimmed).await.map_err(StorageError::from)?;
            }

            conn.exec_drop(
                r"INSERT INTO schema_version (version) VALUES (:version)
                  ON DUPLICATE KEY UPDATE version = VALUES(version)",
                params! { "version" => SCHEMA_VERSION },
            ).await.map_err(StorageError::from)?;

            Ok(())
        }
    }

    fn health_check(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut conn = self.pool.get_conn().await.map_err(StorageError::from)?;
            let result: Option<u8> = conn.query_first("SELECT 1")
                .await.map_err(StorageError::from)?;
            if result.is_none() {
                return Err(StorageError::Connection("health check failed".to_string()).into());
            }
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
    #[ignore = "requires a running MariaDB instance"]
    async fn test_mariadb_session_crud() {
        let backend = MariaDbBackend::connect("mysql://root@localhost/clawdius_test")
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
    #[ignore = "requires a running MariaDB instance"]
    async fn test_mariadb_save_message() {
        let backend = MariaDbBackend::connect("mysql://root@localhost/clawdius_test")
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
    #[ignore = "requires a running MariaDB instance"]
    async fn test_mariadb_search_messages() {
        let backend = MariaDbBackend::connect("mysql://root@localhost/clawdius_test")
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
    #[ignore = "requires a running MariaDB instance"]
    async fn test_mariadb_update_token_usage() {
        let backend = MariaDbBackend::connect("mysql://root@localhost/clawdius_test")
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
    #[ignore = "requires a running MariaDB instance"]
    async fn test_mariadb_checkpoint_crud() {
        let backend = MariaDbBackend::connect("mysql://root@localhost/clawdius_test")
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
    #[ignore = "requires a running MariaDB instance"]
    async fn test_mariadb_graph_file_crud() {
        let backend = MariaDbBackend::connect("mysql://root@localhost/clawdius_test")
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
    #[ignore = "requires a running MariaDB instance"]
    async fn test_mariadb_storage_backend_trait() {
        let backend = MariaDbBackend::connect("mysql://root@localhost/clawdius_test")
            .await
            .unwrap();
        assert_eq!(backend.backend_type(), "mariadb");
        backend.migrate().await.unwrap();
        backend.health_check().await.unwrap();
        backend.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires a running MariaDB instance"]
    async fn test_mariadb_storage_stats() {
        let backend = MariaDbBackend::connect("mysql://root@localhost/clawdius_test")
            .await
            .unwrap();
        backend.migrate().await.unwrap();
        let stats = backend.storage_stats().await.unwrap();
        assert_eq!(stats.checkpoint_count, 0);
        assert_eq!(stats.tracked_file_count, 0);
    }

    #[tokio::test]
    #[ignore = "requires a running MariaDB instance"]
    async fn test_mariadb_track_file() {
        let backend = MariaDbBackend::connect("mysql://root@localhost/clawdius_test")
            .await
            .unwrap();
        backend.migrate().await.unwrap();
        backend.track_file(&PathBuf::from("test.rs")).await.unwrap();
        assert_eq!(backend.tracked_file_count().await.unwrap(), 1);
    }

    #[tokio::test]
    #[ignore = "requires a running MariaDB instance"]
    async fn test_mariadb_cleanup_old_checkpoints() {
        let backend = MariaDbBackend::connect("mysql://root@localhost/clawdius_test")
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
    #[ignore = "requires a running MariaDB instance"]
    async fn test_mariadb_health_check() {
        let backend = MariaDbBackend::connect("mysql://root@localhost/clawdius_test")
            .await
            .unwrap();
        backend.migrate().await.unwrap();
        backend.health_check().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires a running MariaDB instance"]
    async fn test_mariadb_graph_clear() {
        let backend = MariaDbBackend::connect("mysql://root@localhost/clawdius_test")
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
