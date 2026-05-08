//! PostgreSQL storage backend implementation.
//!
//! Implements all three domain traits (`SessionRepository`, `TimelineRepository`,
//! `GraphRepository`) and the unified `StorageBackend` trait using `tokio-postgres`
//! with `deadpool_postgres` connection pooling.
//!
//! Enable with the `postgres` feature flag.

mod graph;
mod migrations;
mod sessions;
mod timeline;
mod workspaces;

use super::backend::StorageBackend;
use super::error::StorageError;
use crate::error::Result;
use crate::graph_rag::ast::{Reference, Relationship, Symbol, SymbolKind};
use crate::session::types::{
    ContentPart, Message, MessageContent, MessageRole, Session, SessionId, SessionMeta, TokenUsage,
};
use crate::timeline::{CheckpointId, CheckpointInfo, FileVersion};
use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;
use migrations::{INIT_SQL, SCHEMA_VERSION};
use std::path::PathBuf;
use tokio_postgres::types::ToSql;
use uuid::Uuid;

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

    pub(super) async fn get_client(
        &self,
    ) -> std::result::Result<deadpool_postgres::Client, StorageError> {
        self.pool.get().await.map_err(|e| {
            StorageError::Connection(format!("failed to get connection from pool: {e}"))
        })
    }

    pub(super) fn row_to_session(
        row: &tokio_postgres::Row,
    ) -> std::result::Result<Session, StorageError> {
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
            id: SessionId::from_uuid(Uuid::parse_str(&id_str).map_err(|e| {
                StorageError::RowConversion {
                    reason: format!("invalid session UUID: {e}"),
                }
            })?),
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

    pub(super) fn row_to_message(
        row: &tokio_postgres::Row,
    ) -> std::result::Result<Message, StorageError> {
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

    pub(super) fn row_to_checkpoint_info(row: &tokio_postgres::Row) -> CheckpointInfo {
        CheckpointInfo {
            id: CheckpointId::from_string(row.get::<_, String>(0)),
            name: row.get(1),
            description: row.get(2),
            timestamp: row.get(3),
            files_count: row.get::<_, i32>(4) as usize,
            total_size: row.get::<_, i64>(5) as usize,
        }
    }

    pub(super) fn row_to_file_version(row: &tokio_postgres::Row) -> FileVersion {
        FileVersion {
            path: PathBuf::from(row.get::<_, String>(0)),
            version: row.get::<_, i32>(1) as u64,
            timestamp: row.get(5),
            checksum: row.get(3),
            size: row.get::<_, i64>(4) as usize,
            checkpoint_id: CheckpointId::from_string(row.get(2)),
        }
    }

    pub(super) fn row_to_symbol(
        row: &tokio_postgres::Row,
    ) -> std::result::Result<Symbol, StorageError> {
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

    pub(super) fn row_to_reference(row: &tokio_postgres::Row) -> Reference {
        Reference {
            id: Some(row.get(0)),
            symbol_id: row.get(1),
            file_id: row.get(2),
            line: row.get(3),
            col: row.get(4),
            context: row.get(5),
        }
    }

    pub(super) fn row_to_relationship(row: &tokio_postgres::Row) -> Relationship {
        let rel_type_str: String = row.get(3);
        let rel_type = Self::parse_relationship_type(&rel_type_str);
        Relationship {
            id: Some(row.get(0)),
            from_symbol: row.get(1),
            to_symbol: row.get(2),
            relationship_type: rel_type,
        }
    }

    pub(super) fn parse_symbol_kind(s: &str) -> SymbolKind {
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

    pub(super) fn parse_relationship_type(s: &str) -> crate::graph_rag::ast::RelationshipType {
        match s {
            "Calls" => crate::graph_rag::ast::RelationshipType::Calls,
            "Implements" => crate::graph_rag::ast::RelationshipType::Implements,
            "Contains" => crate::graph_rag::ast::RelationshipType::Contains,
            "Imports" => crate::graph_rag::ast::RelationshipType::Imports,
            "References" => crate::graph_rag::ast::RelationshipType::References,
            "Extends" => crate::graph_rag::ast::RelationshipType::Extends,
            "DependsOn" => crate::graph_rag::ast::RelationshipType::DependsOn,
            other => crate::graph_rag::ast::RelationshipType::Other(other.to_string()),
        }
    }
}

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
        async move { Ok(()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_rag::ast::FileInfo;
    use crate::storage::backend::{GraphRepository, SessionRepository, TimelineRepository};
    use std::path::PathBuf;

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
