//! Audit Logging for Enterprise Compliance
//!
//! Provides comprehensive audit logging for regulatory compliance.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Audit event severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditSeverity {
    /// Informational event
    Info,
    /// Warning event
    Warning,
    /// Error event
    Error,
    /// Critical security event
    Critical,
}

/// Audit event category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditCategory {
    /// Authentication events
    Authentication,
    /// Authorization events
    Authorization,
    /// Data access events
    DataAccess,
    /// Data modification events
    DataModification,
    /// Configuration changes
    Configuration,
    /// System events
    System,
    /// Security events
    Security,
    /// Compliance events
    Compliance,
}

/// Audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Unique event ID
    pub id: String,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Event category
    pub category: AuditCategory,
    /// Event severity
    pub severity: AuditSeverity,
    /// Event type/action
    pub action: String,
    /// Actor (user or system)
    pub actor: Actor,
    /// Resource affected
    pub resource: Option<Resource>,
    /// Event details
    pub details: HashMap<String, serde_json::Value>,
    /// Source IP address
    pub source_ip: Option<String>,
    /// User agent
    pub user_agent: Option<String>,
    /// Session ID
    pub session_id: Option<String>,
    /// Request ID for tracing
    pub request_id: Option<String>,
    /// Outcome (success/failure)
    pub outcome: AuditOutcome,
    /// Error message (if failed)
    pub error_message: Option<String>,
    /// Related events
    pub related_events: Vec<String>,
}

/// Actor in an audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actor {
    /// Actor type
    pub actor_type: ActorType,
    /// Actor ID (user ID, service ID, etc.)
    pub id: String,
    /// Actor name
    pub name: Option<String>,
    /// Actor email
    pub email: Option<String>,
    /// Actor roles
    pub roles: Vec<String>,
}

/// Type of actor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActorType {
    /// Human user
    User,
    /// Service account
    Service,
    /// System process
    System,
    /// API key
    ApiKey,
    /// Plugin
    Plugin,
}

/// Resource in an audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    /// Resource type
    pub resource_type: String,
    /// Resource ID
    pub id: String,
    /// Resource name
    pub name: Option<String>,
    /// Resource path
    pub path: Option<String>,
}

/// Audit outcome
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditOutcome {
    /// Operation succeeded
    Success,
    /// Operation failed
    Failure,
    /// Operation denied
    Denied,
    /// Operation pending
    Pending,
}

impl AuditEvent {
    /// Create a new audit event
    pub fn new(category: AuditCategory, action: impl Into<String>, actor: Actor) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            category,
            severity: AuditSeverity::Info,
            action: action.into(),
            actor,
            resource: None,
            details: HashMap::new(),
            source_ip: None,
            user_agent: None,
            session_id: None,
            request_id: None,
            outcome: AuditOutcome::Success,
            error_message: None,
            related_events: Vec::new(),
        }
    }

    /// Set severity
    #[must_use]
    pub fn with_severity(mut self, severity: AuditSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Set resource
    #[must_use]
    pub fn with_resource(mut self, resource: Resource) -> Self {
        self.resource = Some(resource);
        self
    }

    /// Add detail
    pub fn with_detail(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.details.insert(key.into(), value);
        self
    }

    /// Set source IP
    pub fn with_source_ip(mut self, ip: impl Into<String>) -> Self {
        self.source_ip = Some(ip.into());
        self
    }

    /// Set session ID
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set outcome
    #[must_use]
    pub fn with_outcome(mut self, outcome: AuditOutcome) -> Self {
        self.outcome = outcome;
        self
    }

    /// Set error message
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error_message = Some(error.into());
        self
    }

    // Common audit events

    /// Login event
    #[must_use]
    pub fn login(actor: Actor, success: bool) -> Self {
        Self::new(AuditCategory::Authentication, "user.login", actor)
            .with_severity(if success {
                AuditSeverity::Info
            } else {
                AuditSeverity::Warning
            })
            .with_outcome(if success {
                AuditOutcome::Success
            } else {
                AuditOutcome::Failure
            })
    }

    /// Logout event
    #[must_use]
    pub fn logout(actor: Actor) -> Self {
        Self::new(AuditCategory::Authentication, "user.logout", actor)
    }

    /// File read event
    #[must_use]
    pub fn file_read(actor: Actor, path: &str) -> Self {
        Self::new(AuditCategory::DataAccess, "file.read", actor).with_resource(Resource {
            resource_type: "file".to_string(),
            id: blake3::hash(path.as_bytes()).to_hex().to_string(),
            name: None,
            path: Some(path.to_string()),
        })
    }

    /// File write event
    #[must_use]
    pub fn file_write(actor: Actor, path: &str) -> Self {
        Self::new(AuditCategory::DataModification, "file.write", actor).with_resource(Resource {
            resource_type: "file".to_string(),
            id: blake3::hash(path.as_bytes()).to_hex().to_string(),
            name: None,
            path: Some(path.to_string()),
        })
    }

    /// Command execution event
    #[must_use]
    pub fn command(actor: Actor, command: &str) -> Self {
        Self::new(AuditCategory::System, "command.execute", actor)
            .with_detail("command", serde_json::json!(command))
    }

    /// Configuration change event
    #[must_use]
    pub fn config_change(
        actor: Actor,
        key: &str,
        old_value: Option<&str>,
        new_value: Option<&str>,
    ) -> Self {
        Self::new(AuditCategory::Configuration, "config.change", actor)
            .with_detail("key", serde_json::json!(key))
            .with_detail("old_value", serde_json::json!(old_value))
            .with_detail("new_value", serde_json::json!(new_value))
    }

    /// Permission denied event
    #[must_use]
    pub fn permission_denied(actor: Actor, resource: &str, action: &str) -> Self {
        Self::new(AuditCategory::Authorization, "permission.denied", actor)
            .with_severity(AuditSeverity::Warning)
            .with_outcome(AuditOutcome::Denied)
            .with_detail("resource", serde_json::json!(resource))
            .with_detail("requested_action", serde_json::json!(action))
    }

    /// Security alert event
    #[must_use]
    pub fn security_alert(actor: Actor, alert_type: &str, message: &str) -> Self {
        Self::new(
            AuditCategory::Security,
            format!("security.{alert_type}"),
            actor,
        )
        .with_severity(AuditSeverity::Critical)
        .with_detail("message", serde_json::json!(message))
    }
}

/// Audit log storage backend
#[derive(Debug, Clone)]
pub enum AuditStorage {
    /// File-based storage
    File { path: PathBuf },
    /// `SQLite` database
    SQLite { path: PathBuf },
    /// Elasticsearch
    Elasticsearch { url: String, index: String },
    /// Custom webhook
    Webhook {
        url: String,
        headers: HashMap<String, String>,
    },
}

/// Audit logger
pub struct AuditLogger {
    storage: AuditStorage,
    buffer: Arc<RwLock<Vec<AuditEvent>>>,
    flush_interval: std::time::Duration,
    retention_days: u32,
}

impl AuditLogger {
    /// Create a new audit logger
    #[must_use]
    pub fn new(storage: AuditStorage) -> Self {
        Self {
            storage,
            buffer: Arc::new(RwLock::new(Vec::new())),
            flush_interval: std::time::Duration::from_secs(5),
            retention_days: 90,
        }
    }

    /// Set flush interval
    #[must_use]
    pub fn with_flush_interval(mut self, interval: std::time::Duration) -> Self {
        self.flush_interval = interval;
        self
    }

    /// Set retention period
    #[must_use]
    pub fn with_retention(mut self, days: u32) -> Self {
        self.retention_days = days;
        self
    }

    /// Log an audit event
    pub async fn log(&self, event: AuditEvent) -> Result<()> {
        // Add to buffer
        self.buffer.write().await.push(event);

        // Flush if buffer is large enough
        if self.buffer.read().await.len() >= 100 {
            self.flush().await?;
        }

        Ok(())
    }

    /// Flush buffer to storage
    pub async fn flush(&self) -> Result<()> {
        let mut buffer = self.buffer.write().await;
        if buffer.is_empty() {
            return Ok(());
        }

        let events: Vec<AuditEvent> = buffer.drain(..).collect();

        match &self.storage {
            AuditStorage::File { path } => {
                self.flush_to_file(path, &events).await?;
            }
            AuditStorage::SQLite { path } => {
                self.flush_to_sqlite(path, &events).await?;
            }
            AuditStorage::Elasticsearch { url, index } => {
                self.flush_to_elasticsearch(url, index, &events).await?;
            }
            AuditStorage::Webhook { url, headers } => {
                self.flush_to_webhook(url, headers, &events).await?;
            }
        }

        Ok(())
    }

    async fn flush_to_file(&self, path: &PathBuf, events: &[AuditEvent]) -> Result<()> {
        use tokio::io::AsyncWriteExt;

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;

        for event in events {
            let json = serde_json::to_string(event)?;
            file.write_all(json.as_bytes()).await?;
            file.write_all(b"\n").await?;
        }

        file.flush().await?;
        Ok(())
    }

    async fn flush_to_sqlite(&self, path: &PathBuf, events: &[AuditEvent]) -> Result<()> {
        let path = path.clone();
        let events = events.to_vec();

        // Run in blocking thread
        tokio::task::spawn_blocking(move || {
            let mut conn = rusqlite::Connection::open(&path)?;

            conn.execute(
                "CREATE TABLE IF NOT EXISTS audit_log (
                    id TEXT PRIMARY KEY,
                    timestamp TEXT NOT NULL,
                    category TEXT NOT NULL,
                    severity TEXT NOT NULL,
                    action TEXT NOT NULL,
                    actor TEXT NOT NULL,
                    resource TEXT,
                    details TEXT,
                    source_ip TEXT,
                    session_id TEXT,
                    outcome TEXT NOT NULL,
                    error_message TEXT
                )",
                [],
            )?;

            let tx = conn.transaction()?;
            for event in &events {
                tx.execute(
                    "INSERT INTO audit_log (id, timestamp, category, severity, action, actor, resource, details, source_ip, session_id, outcome, error_message)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    rusqlite::params![
                        event.id,
                        event.timestamp.to_rfc3339(),
                        serde_json::to_string(&event.category)?,
                        serde_json::to_string(&event.severity)?,
                        event.action,
                        serde_json::to_string(&event.actor)?,
                        event.resource.as_ref().map(serde_json::to_string).transpose()?,
                        serde_json::to_string(&event.details)?,
                        event.source_ip,
                        event.session_id,
                        serde_json::to_string(&event.outcome)?,
                        event.error_message,
                    ],
                )?;
            }
            tx.commit()?;

            Ok::<_, anyhow::Error>(())
        })
        .await??;

        Ok(())
    }

    async fn flush_to_elasticsearch(
        &self,
        url: &str,
        index: &str,
        events: &[AuditEvent],
    ) -> Result<()> {
        let client = reqwest::Client::new();

        for event in events {
            let bulk_url = format!("{url}/_bulk");
            let mut body = String::new();
            body.push_str(&format!("{{\"index\":{{\"_index\":\"{index}\"}}}}\n"));
            body.push_str(&serde_json::to_string(event)?);
            body.push('\n');

            client
                .post(&bulk_url)
                .header("Content-Type", "application/json")
                .body(body)
                .send()
                .await?;
        }

        Ok(())
    }

    async fn flush_to_webhook(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        events: &[AuditEvent],
    ) -> Result<()> {
        let client = reqwest::Client::new();

        let mut request = client.post(url).json(events);
        for (key, value) in headers {
            request = request.header(key, value);
        }

        request.send().await?;
        Ok(())
    }

    /// Query audit events
    pub async fn query(&self, query: AuditQuery) -> Result<Vec<AuditEvent>> {
        match &self.storage {
            AuditStorage::SQLite { path } => self.query_sqlite(path, query).await,
            _ => Err(anyhow::anyhow!("Query not supported for this storage type")),
        }
    }

    async fn query_sqlite(&self, path: &PathBuf, query: AuditQuery) -> Result<Vec<AuditEvent>> {
        let path = path.clone();

        let events = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)?;
            let mut sql = String::from("SELECT id, timestamp, category, severity, action, actor, resource, details, source_ip, session_id, outcome, error_message FROM audit_log WHERE 1=1");
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

            if let Some(start) = &query.start_time {
                sql.push_str(" AND timestamp >= ?");
                params.push(Box::new(start.to_rfc3339()));
            }

            if let Some(end) = &query.end_time {
                sql.push_str(" AND timestamp <= ?");
                params.push(Box::new(end.to_rfc3339()));
            }

            if let Some(cat) = &query.category {
                sql.push_str(" AND category = ?");
                params.push(Box::new(serde_json::to_string(cat)?));
            }

            if let Some(actor_id) = &query.actor_id {
                sql.push_str(" AND actor LIKE ?");
                params.push(Box::new(format!("%\"id\":\"{actor_id}\"")));
            }

            sql.push_str(" ORDER BY timestamp DESC LIMIT ?");
            params.push(Box::new(query.limit.unwrap_or(100) as i64));

            let mut stmt = conn.prepare(&sql)?;
            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(std::convert::AsRef::as_ref).collect();

            let events = stmt
                .query_map(param_refs.as_slice(), |row| {
                    Ok(AuditEvent {
                        id: row.get(0)?,
                        timestamp: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?).map_or_else(|_| chrono::Utc::now(), |dt| dt.with_timezone(&chrono::Utc)),
                        category: serde_json::from_str(&row.get::<_, String>(2)?).unwrap_or(AuditCategory::System),
                        severity: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or(AuditSeverity::Info),
                        action: row.get(4)?,
                        actor: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or(Actor {
                            actor_type: ActorType::System,
                            id: String::new(),
                            name: None,
                            email: None,
                            roles: Vec::new(),
                        }),
                        resource: row.get::<_, Option<String>>(6)?
                            .map(|s| serde_json::from_str(&s))
                            .transpose()
                            .ok()
                            .flatten(),
                        details: row.get::<_, Option<String>>(7)?
                            .map(|s| serde_json::from_str(&s))
                            .transpose()
                            .unwrap_or_default()
                            .unwrap_or_default(),
                        source_ip: row.get(8)?,
                        session_id: row.get(9)?,
                        request_id: None,
                        outcome: serde_json::from_str(&row.get::<_, String>(10)?).unwrap_or(AuditOutcome::Success),
                        error_message: row.get(11)?,
                        user_agent: None,
                        related_events: Vec::new(),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            Ok::<_, anyhow::Error>(events)
        })
        .await??;

        Ok(events)
    }

    /// Cleanup old audit events
    pub async fn cleanup(&self) -> Result<usize> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(i64::from(self.retention_days));

        match &self.storage {
            AuditStorage::SQLite { path } => {
                let path = path.clone();
                let deleted = tokio::task::spawn_blocking(move || {
                    let conn = rusqlite::Connection::open(&path)?;
                    let deleted = conn.execute(
                        "DELETE FROM audit_log WHERE timestamp < ?",
                        [cutoff.to_rfc3339()],
                    )?;
                    Ok::<_, anyhow::Error>(deleted)
                })
                .await??;
                Ok(deleted)
            }
            _ => Ok(0),
        }
    }
}

/// Audit query parameters
#[derive(Debug, Clone, Default)]
pub struct AuditQuery {
    /// Start time
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    /// End time
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Filter by category
    pub category: Option<AuditCategory>,
    /// Filter by actor ID
    pub actor_id: Option<String>,
    /// Maximum results
    pub limit: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_event_creation() {
        let actor = Actor {
            actor_type: ActorType::User,
            id: "user123".to_string(),
            name: Some("Test User".to_string()),
            email: Some("test@example.com".to_string()),
            roles: vec!["admin".to_string()],
        };

        let event = AuditEvent::login(actor.clone(), true);
        assert_eq!(event.category, AuditCategory::Authentication);
        assert_eq!(event.outcome, AuditOutcome::Success);
    }

    #[test]
    fn test_audit_event_file_read() {
        let actor = Actor {
            actor_type: ActorType::User,
            id: "user123".to_string(),
            name: None,
            email: None,
            roles: vec![],
        };

        let event = AuditEvent::file_read(actor, "/path/to/file.rs");
        assert_eq!(event.category, AuditCategory::DataAccess);
        assert!(event.resource.is_some());
    }
}
