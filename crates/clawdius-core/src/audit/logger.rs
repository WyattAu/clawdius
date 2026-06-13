use anyhow::Result;
use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: u64,
    pub event_type: String,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub action: String,
    pub resource: Option<String>,
    pub details: serde_json::Value,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[async_trait]
pub trait AuditBackend: Send + Sync {
    async fn write(&self, entry: &AuditEntry) -> Result<()>;

    async fn write_batch(&self, entries: &[AuditEntry]) -> Result<()> {
        for entry in entries {
            self.write(entry).await?;
        }
        Ok(())
    }

    async fn query(&self, event_type: Option<&str>, limit: usize) -> Result<Vec<AuditEntry>>;

    async fn delete_before(&self, timestamp: u64) -> Result<usize>;

    async fn flush(&self) -> Result<()> {
        Ok(())
    }

    fn backend_name(&self) -> &'static str;
}

pub struct MemoryBackend {
    entries: RwLock<Vec<AuditEntry>>,
    max_entries: usize,
}

impl MemoryBackend {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: RwLock::new(Vec::new()),
            max_entries,
        }
    }
}

#[async_trait]
impl AuditBackend for MemoryBackend {
    async fn write(&self, entry: &AuditEntry) -> Result<()> {
        let mut entries = self.entries.write();
        if entries.len() >= self.max_entries {
            entries.remove(0);
        }
        entries.push(entry.clone());
        Ok(())
    }

    async fn query(&self, event_type: Option<&str>, limit: usize) -> Result<Vec<AuditEntry>> {
        let entries = self.entries.read();
        let filtered: Vec<AuditEntry> = entries
            .iter()
            .filter(|e| event_type.map_or(true, |t| e.event_type == t))
            .rev()
            .take(limit)
            .cloned()
            .collect();
        Ok(filtered.into_iter().rev().collect())
    }

    async fn delete_before(&self, timestamp: u64) -> Result<usize> {
        let mut entries = self.entries.write();
        let before = entries.len();
        entries.retain(|e| e.timestamp >= timestamp);
        Ok(before - entries.len())
    }

    fn backend_name(&self) -> &'static str {
        "memory"
    }
}

#[must_use]
pub fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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
    async fn test_memory_write_and_query() {
        let backend = MemoryBackend::new(100);
        backend.write(&sample_entry("auth", 100)).await.unwrap();
        backend.write(&sample_entry("auth", 200)).await.unwrap();
        backend.write(&sample_entry("llm", 300)).await.unwrap();

        let all = backend.query(None, 10).await.unwrap();
        assert_eq!(all.len(), 3);

        let auth = backend.query(Some("auth"), 10).await.unwrap();
        assert_eq!(auth.len(), 2);
    }

    #[tokio::test]
    async fn test_memory_delete_before() {
        let backend = MemoryBackend::new(100);
        backend.write(&sample_entry("auth", 100)).await.unwrap();
        backend.write(&sample_entry("auth", 200)).await.unwrap();
        backend.write(&sample_entry("auth", 300)).await.unwrap();

        let deleted = backend.delete_before(200).await.unwrap();
        assert_eq!(deleted, 1);

        let remaining = backend.query(None, 10).await.unwrap();
        assert_eq!(remaining.len(), 2);
    }

    #[tokio::test]
    async fn test_memory_max_entries_eviction() {
        let backend = MemoryBackend::new(2);
        backend.write(&sample_entry("a", 1)).await.unwrap();
        backend.write(&sample_entry("b", 2)).await.unwrap();
        backend.write(&sample_entry("c", 3)).await.unwrap();

        let all = backend.query(None, 10).await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].event_type, "b");
        assert_eq!(all[1].event_type, "c");
    }
}
