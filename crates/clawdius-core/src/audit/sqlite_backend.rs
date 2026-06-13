use super::logger::{AuditBackend, AuditEntry};
use anyhow::{Context, Result};
use async_trait::async_trait;
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Arc;

pub struct SqliteBackend {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteBackend {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let conn = Connection::open(&path)
            .with_context(|| format!("Failed to open audit SQLite database: {}", path.display()))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS audit_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                user_id TEXT,
                session_id TEXT,
                action TEXT NOT NULL,
                resource TEXT,
                details TEXT NOT NULL,
                ip_address TEXT,
                user_agent TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_entries(timestamp);
            CREATE INDEX IF NOT EXISTS idx_audit_event_type ON audit_entries(event_type);",
        )
        .with_context(|| "Failed to create audit tables")?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

#[async_trait]
impl AuditBackend for SqliteBackend {
    async fn write(&self, entry: &AuditEntry) -> Result<()> {
        let conn = self.conn.clone();
        let entry = entry.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock();
            conn.execute(
                "INSERT INTO audit_entries (timestamp, event_type, user_id, session_id, action, resource, details, ip_address, user_agent)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    entry.timestamp as i64,
                    entry.event_type,
                    entry.user_id,
                    entry.session_id,
                    entry.action,
                    entry.resource,
                    serde_json::to_string(&entry.details)?,
                    entry.ip_address,
                    entry.user_agent,
                ],
            )?;
            Ok(())
        })
        .await?
    }

    async fn query(&self, event_type: Option<&str>, limit: usize) -> Result<Vec<AuditEntry>> {
        let conn = self.conn.clone();
        let event_type = event_type.map(String::from);
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock();
            let entries = if let Some(et) = &event_type {
                let mut stmt = conn.prepare(
                    "SELECT timestamp, event_type, user_id, session_id, action, resource, details, ip_address, user_agent
                     FROM audit_entries WHERE event_type = ?1 ORDER BY timestamp DESC LIMIT ?2",
                )?;
                let rows: Vec<AuditEntry> = stmt
                    .query_map(params![et, limit as i64], |row| {
                        Ok(AuditEntry {
                            timestamp: row.get::<_, i64>(0)? as u64,
                            event_type: row.get(1)?,
                            user_id: row.get(2)?,
                            session_id: row.get(3)?,
                            action: row.get(4)?,
                            resource: row.get(5)?,
                            details: serde_json::from_str(&row.get::<_, String>(6)?)
                                .unwrap_or(serde_json::json!({})),
                            ip_address: row.get(7)?,
                            user_agent: row.get(8)?,
                        })
                    })?
                    .filter_map(|e| e.ok())
                    .collect();
                rows
            } else {
                let mut stmt = conn.prepare(
                    "SELECT timestamp, event_type, user_id, session_id, action, resource, details, ip_address, user_agent
                     FROM audit_entries ORDER BY timestamp DESC LIMIT ?1",
                )?;
                let rows: Vec<AuditEntry> = stmt
                    .query_map(params![limit as i64], |row| {
                        Ok(AuditEntry {
                            timestamp: row.get::<_, i64>(0)? as u64,
                            event_type: row.get(1)?,
                            user_id: row.get(2)?,
                            session_id: row.get(3)?,
                            action: row.get(4)?,
                            resource: row.get(5)?,
                            details: serde_json::from_str(&row.get::<_, String>(6)?)
                                .unwrap_or(serde_json::json!({})),
                            ip_address: row.get(7)?,
                            user_agent: row.get(8)?,
                        })
                    })?
                    .filter_map(|e| e.ok())
                    .collect();
                rows
            };
            Ok(entries)
        })
        .await?
    }

    async fn delete_before(&self, timestamp: u64) -> Result<usize> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock();
            let count = conn.execute(
                "DELETE FROM audit_entries WHERE timestamp < ?1",
                params![timestamp as i64],
            )?;
            Ok(count)
        })
        .await?
    }

    fn backend_name(&self) -> &'static str {
        "sqlite"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry(event_type: &str, ts: u64) -> AuditEntry {
        AuditEntry {
            timestamp: ts,
            event_type: event_type.to_string(),
            user_id: None,
            session_id: None,
            action: "test".to_string(),
            resource: None,
            details: serde_json::json!({}),
            ip_address: None,
            user_agent: None,
        }
    }

    #[tokio::test]
    async fn test_sqlite_write_and_query() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let backend = SqliteBackend::new(&db_path).unwrap();

        backend.write(&sample_entry("auth", 100)).await.unwrap();
        backend.write(&sample_entry("auth", 200)).await.unwrap();
        backend.write(&sample_entry("llm", 300)).await.unwrap();

        let all = backend.query(None, 10).await.unwrap();
        assert_eq!(all.len(), 3);

        let auth = backend.query(Some("auth"), 10).await.unwrap();
        assert_eq!(auth.len(), 2);
    }

    #[tokio::test]
    async fn test_sqlite_delete_before() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let backend = SqliteBackend::new(&db_path).unwrap();

        backend.write(&sample_entry("auth", 100)).await.unwrap();
        backend.write(&sample_entry("auth", 200)).await.unwrap();
        backend.write(&sample_entry("auth", 300)).await.unwrap();

        let deleted = backend.delete_before(200).await.unwrap();
        assert_eq!(deleted, 1);

        let remaining = backend.query(None, 10).await.unwrap();
        assert_eq!(remaining.len(), 2);
    }
}
