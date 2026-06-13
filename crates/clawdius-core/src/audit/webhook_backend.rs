use super::logger::{AuditBackend, AuditEntry};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub struct WebhookBackend {
    url: String,
    client: reqwest::Client,
    max_retries: u32,
    sent_count: AtomicU64,
    failed_count: AtomicU64,
}

impl WebhookBackend {
    pub fn new(url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            url,
            client,
            max_retries: 3,
            sent_count: AtomicU64::new(0),
            failed_count: AtomicU64::new(0),
        }
    }
}

#[async_trait]
impl AuditBackend for WebhookBackend {
    async fn write(&self, entry: &AuditEntry) -> Result<()> {
        let payload = serde_json::to_string(entry)?;

        for attempt in 0..=self.max_retries {
            match self
                .client
                .post(&self.url)
                .header("Content-Type", "application/json")
                .body(payload.clone())
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    self.sent_count.fetch_add(1, Ordering::Relaxed);
                    return Ok(());
                },
                Ok(resp) => {
                    tracing::warn!(
                        "Webhook audit backend returned status {} (attempt {}/{})",
                        resp.status(),
                        attempt + 1,
                        self.max_retries + 1
                    );
                },
                Err(e) => {
                    tracing::warn!(
                        "Webhook audit backend request failed: {} (attempt {}/{})",
                        e,
                        attempt + 1,
                        self.max_retries + 1
                    );
                },
            }

            if attempt < self.max_retries {
                tokio::time::sleep(Duration::from_millis(100 * 2u64.pow(attempt))).await;
            }
        }

        self.failed_count.fetch_add(1, Ordering::Relaxed);
        anyhow::bail!(
            "Webhook audit backend failed after {} retries",
            self.max_retries
        );
    }

    async fn query(&self, _event_type: Option<&str>, _limit: usize) -> Result<Vec<AuditEntry>> {
        Ok(Vec::new())
    }

    async fn delete_before(&self, _timestamp: u64) -> Result<usize> {
        Ok(0)
    }

    fn backend_name(&self) -> &'static str {
        "webhook"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_construction() {
        let backend = WebhookBackend::new("http://localhost:9999/webhook".to_string());
        assert_eq!(backend.backend_name(), "webhook");
    }
}
