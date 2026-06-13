use super::logger::{AuditBackend, AuditEntry};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct ElasticsearchBackend {
    url: String,
    index: String,
    client: reqwest::Client,
    sent_count: AtomicU64,
    failed_count: AtomicU64,
}

impl ElasticsearchBackend {
    pub fn new(url: String, index: String) -> Self {
        let client = reqwest::Client::builder().build().unwrap_or_default();
        Self {
            url,
            index,
            client,
            sent_count: AtomicU64::new(0),
            failed_count: AtomicU64::new(0),
        }
    }
}

#[async_trait]
impl AuditBackend for ElasticsearchBackend {
    async fn write(&self, entry: &AuditEntry) -> Result<()> {
        let bulk_url = format!("{}/{}/_doc", self.url.trim_end_matches('/'), self.index);

        let resp = self
            .client
            .post(&bulk_url)
            .header("Content-Type", "application/json")
            .json(entry)
            .send()
            .await
            .with_context(|| "Elasticsearch request failed")?;

        if resp.status().is_success() {
            self.sent_count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        } else {
            self.failed_count.fetch_add(1, Ordering::Relaxed);
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Elasticsearch returned {}: {}", status, body)
        }
    }

    async fn write_batch(&self, entries: &[AuditEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let bulk_url = format!("{}/{}/_bulk", self.url.trim_end_matches('/'), self.index);

        let mut body = String::new();
        for entry in entries {
            body.push_str("{\"index\":{}}\n");
            body.push_str(&serde_json::to_string(entry)?);
            body.push('\n');
        }

        let resp = self
            .client
            .post(&bulk_url)
            .header("Content-Type", "application/x-ndjson")
            .body(body)
            .send()
            .await
            .with_context(|| "Elasticsearch bulk request failed")?;

        if resp.status().is_success() {
            self.sent_count
                .fetch_add(entries.len() as u64, Ordering::Relaxed);
            Ok(())
        } else {
            self.failed_count
                .fetch_add(entries.len() as u64, Ordering::Relaxed);
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Elasticsearch bulk returned {}: {}", status, body)
        }
    }

    async fn query(&self, event_type: Option<&str>, limit: usize) -> Result<Vec<AuditEntry>> {
        let search_url = format!("{}/{}/_search", self.url.trim_end_matches('/'), self.index);

        let query = match event_type {
            Some(et) => serde_json::json!({
                "query": {"term": {"event_type": et}},
                "size": limit,
                "sort": [{"timestamp": {"order": "desc"}}]
            }),
            None => serde_json::json!({
                "query": {"match_all": {}},
                "size": limit,
                "sort": [{"timestamp": {"order": "desc"}}]
            }),
        };

        let resp = self
            .client
            .post(&search_url)
            .header("Content-Type", "application/json")
            .json(&query)
            .send()
            .await
            .with_context(|| "Elasticsearch search request failed")?;

        if !resp.status().is_success() {
            anyhow::bail!("Elasticsearch search returned {}", resp.status());
        }

        let body: serde_json::Value = resp.json().await?;
        let hits = body["hits"]["hits"].as_array().cloned().unwrap_or_default();

        let entries = hits
            .into_iter()
            .filter_map(|h| serde_json::from_value::<AuditEntry>(h["_source"].clone()).ok())
            .collect();

        Ok(entries)
    }

    async fn delete_before(&self, timestamp: u64) -> Result<usize> {
        let delete_url = format!(
            "{}/{}/_delete_by_query",
            self.url.trim_end_matches('/'),
            self.index
        );

        let query = serde_json::json!({
            "query": {"range": {"timestamp": {"lt": timestamp}}}
        });

        let resp = self
            .client
            .post(&delete_url)
            .header("Content-Type", "application/json")
            .json(&query)
            .send()
            .await
            .with_context(|| "Elasticsearch delete request failed")?;

        if resp.status().is_success() {
            let body: serde_json::Value = resp.json().await?;
            let deleted = body["deleted"].as_u64().unwrap_or(0) as usize;
            Ok(deleted)
        } else {
            anyhow::bail!("Elasticsearch delete returned {}", resp.status());
        }
    }

    fn backend_name(&self) -> &'static str {
        "elasticsearch"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elasticsearch_construction() {
        let backend = ElasticsearchBackend::new(
            "http://localhost:9200".to_string(),
            "clawdius-audit".to_string(),
        );
        assert_eq!(backend.backend_name(), "elasticsearch");
    }
}
