use super::SqliteBackend;
use crate::error::Result;
use crate::session::types::{
    ContentPart, Message, MessageContent, MessageRole, Session, SessionId, SessionMeta, TokenUsage,
};
use crate::storage::backend::SessionRepository;
use crate::storage::error::StorageError;
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

impl SqliteBackend {
    pub(super) fn row_to_session(
        row: &rusqlite::Row<'_>,
    ) -> std::result::Result<Session, rusqlite::Error> {
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

    pub(super) fn row_to_message(
        row: &rusqlite::Row<'_>,
    ) -> std::result::Result<Message, rusqlite::Error> {
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

impl SessionRepository for SqliteBackend {
    fn create_session(
        &self,
        session: &Session,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
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

    fn load_session(
        &self,
        id: &SessionId,
    ) -> impl std::future::Future<Output = Result<Option<Session>>> + Send {
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

    fn load_session_full(
        &self,
        id: &SessionId,
    ) -> impl std::future::Future<Output = Result<Option<Session>>> + Send {
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

                session.messages = messages
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT messages".to_string(),
                        reason: e.to_string(),
                    })?;

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

    fn delete_session(
        &self,
        id: &SessionId,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
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

    fn save_message(
        &self,
        session_id: &SessionId,
        message: &Message,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
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

    fn search_messages(
        &self,
        query: &str,
    ) -> impl std::future::Future<Output = Result<Vec<(SessionId, Message)>>> + Send {
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

    fn update_token_usage(
        &self,
        id: &SessionId,
        usage: &TokenUsage,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
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
