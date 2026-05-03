use super::PostgresBackend;
use crate::error::Result;
use crate::session::types::{
    ContentPart, Message, MessageContent, MessageRole, Session, SessionId,
    SessionMeta, TokenUsage,
};
use crate::storage::backend::SessionRepository;
use crate::storage::error::StorageError;
use chrono::{DateTime, Utc};
use tokio_postgres::types::ToSql;
use uuid::Uuid;

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
