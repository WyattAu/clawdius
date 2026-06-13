use super::logger::{AuditBackend, AuditEntry};
use super::now_timestamp;
use crate::config::AuditConfig;
use anyhow::Result;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;

pub struct AuditManager {
    backend: Arc<dyn AuditBackend>,
    buffer: Mutex<Vec<AuditEntry>>,
    flush_interval: Duration,
    retention_days: u32,
}

impl AuditManager {
    pub fn from_config(config: &AuditConfig) -> Result<Self> {
        let backend: Arc<dyn AuditBackend> = match config.backend.as_str() {
            "memory" => Arc::new(super::logger::MemoryBackend::new(10000)),
            "file" => Arc::new(super::file_backend::FileBackend::new(&config.path)),
            "sqlite" => Arc::new(super::sqlite_backend::SqliteBackend::new(
                &config.sqlite_path,
            )?),
            "webhook" => Arc::new(super::webhook_backend::WebhookBackend::new(
                config.path.clone(),
            )),
            "syslog" => {
                let parts: Vec<&str> = config.path.splitn(2, ':').collect();
                let host = parts.first().unwrap_or(&"localhost").to_string();
                let port = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(514);
                Arc::new(super::syslog_backend::SyslogBackend::new(host, port))
            },
            "elasticsearch" => Arc::new(super::elasticsearch_backend::ElasticsearchBackend::new(
                config.path.clone(),
                "clawdius-audit".to_string(),
            )),
            other => anyhow::bail!("Unknown audit backend: {}", other),
        };

        Ok(Self {
            backend,
            buffer: Mutex::new(Vec::new()),
            flush_interval: Duration::from_secs(config.flush_interval_secs),
            retention_days: config.retention_days,
        })
    }

    pub async fn log(&self, entry: AuditEntry) -> Result<()> {
        self.backend.write(&entry).await
    }

    pub fn buffer(&self, entry: AuditEntry) {
        self.buffer.lock().push(entry);
    }

    pub async fn flush(&self) -> Result<()> {
        let entries: Vec<AuditEntry> = {
            let mut buf = self.buffer.lock();
            std::mem::take(&mut *buf)
        };

        if entries.is_empty() {
            return Ok(());
        }

        self.backend.write_batch(&entries).await?;
        self.backend.flush().await?;
        Ok(())
    }

    pub async fn run_retention(&self) -> Result<usize> {
        let cutoff = now_timestamp() - (self.retention_days as u64 * 86400);
        self.backend.delete_before(cutoff).await
    }

    pub async fn query(&self, event_type: Option<&str>, limit: usize) -> Result<Vec<AuditEntry>> {
        self.backend.query(event_type, limit).await
    }

    pub fn backend(&self) -> &Arc<dyn AuditBackend> {
        &self.backend
    }

    pub fn flush_interval(&self) -> Duration {
        self.flush_interval
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AuditConfig;

    #[tokio::test]
    async fn test_manager_memory_backend() {
        let config = AuditConfig {
            backend: "memory".to_string(),
            path: "audit".to_string(),
            sqlite_path: "audit.db".to_string(),
            flush_interval_secs: 5,
            retention_days: 90,
        };

        let manager = AuditManager::from_config(&config).unwrap();
        assert_eq!(manager.backend().backend_name(), "memory");

        let entry = crate::audit::events::login_event("user1");
        manager.log(entry).await.unwrap();

        let results = manager.query(Some("auth"), 10).await.unwrap();
        assert_eq!(results.len(), 1);
    }
}
