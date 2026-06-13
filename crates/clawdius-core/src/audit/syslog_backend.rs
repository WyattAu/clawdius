use super::logger::{AuditBackend, AuditEntry};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

pub struct SyslogBackend {
    host: String,
    port: u16,
    facility: u8,
    sent_count: AtomicU64,
    failed_count: AtomicU64,
}

impl SyslogBackend {
    pub fn new(host: String, port: u16) -> Self {
        Self {
            host,
            port,
            facility: 1,
            sent_count: AtomicU64::new(0),
            failed_count: AtomicU64::new(0),
        }
    }

    fn format_rfc5424(&self, entry: &AuditEntry) -> String {
        let severity = 6;
        let pri = (self.facility * 8 + severity) as u8;
        let timestamp = chrono::DateTime::from_timestamp(entry.timestamp as i64, 0)
            .map(|t| t.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
            .unwrap_or_else(|| "-".to_string());

        format!(
            "<{}>1 {} clawdius audit {} - {}",
            pri,
            timestamp,
            entry.action,
            serde_json::to_string(entry).unwrap_or_else(|_| "{}".to_string())
        )
    }
}

#[async_trait]
impl AuditBackend for SyslogBackend {
    async fn write(&self, entry: &AuditEntry) -> Result<()> {
        let message = self.format_rfc5424(entry);

        match TcpStream::connect(format!("{}:{}", self.host, self.port)).await {
            Ok(mut stream) => {
                stream
                    .write_all(format!("{}\n", message).as_bytes())
                    .await?;
                stream.shutdown().await?;
                self.sent_count.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
            Err(e) => {
                self.failed_count.fetch_add(1, Ordering::Relaxed);
                anyhow::bail!("Syslog connection failed: {}", e);
            },
        }
    }

    async fn query(&self, _event_type: Option<&str>, _limit: usize) -> Result<Vec<AuditEntry>> {
        Ok(Vec::new())
    }

    async fn delete_before(&self, _timestamp: u64) -> Result<usize> {
        Ok(0)
    }

    fn backend_name(&self) -> &'static str {
        "syslog"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syslog_construction() {
        let backend = SyslogBackend::new("localhost".to_string(), 514);
        assert_eq!(backend.backend_name(), "syslog");
    }

    #[test]
    fn test_syslog_format_rfc5424() {
        let backend = SyslogBackend::new("localhost".to_string(), 514);
        let entry = AuditEntry {
            timestamp: 1700000000,
            event_type: "auth".to_string(),
            user_id: None,
            session_id: None,
            action: "login".to_string(),
            resource: None,
            details: serde_json::json!({}),
            ip_address: None,
            user_agent: None,
        };
        let msg = backend.format_rfc5424(&entry);
        assert!(msg.starts_with("<14>1 "));
        assert!(msg.contains("clawdius"));
    }
}
