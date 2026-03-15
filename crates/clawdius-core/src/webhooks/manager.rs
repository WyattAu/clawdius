//! Webhook manager for handling webhook registrations and deliveries

use crate::error::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{
    DeliveryStatus, WebhookConfig, WebhookDelivery, WebhookEvent, WebhookId, WebhookPayload,
};

/// Context for a webhook delivery
struct DeliveryContext {
    /// Webhook ID
    webhook_id: WebhookId,
    /// Target URL
    url: String,
    /// Custom headers
    headers: HashMap<String, String>,
    /// Timeout in seconds
    timeout_secs: u64,
    /// Maximum retry attempts
    max_retries: u32,
    /// Payload to deliver
    payload: WebhookPayload,
    /// Event type
    event: WebhookEvent,
}

/// Webhook manager for handling registrations and deliveries
pub struct WebhookManager {
    webhooks: Arc<RwLock<HashMap<WebhookId, WebhookConfig>>>,
    deliveries: Arc<RwLock<Vec<WebhookDelivery>>>,
    client: reqwest::Client,
}

impl WebhookManager {
    /// Create a new webhook manager
    #[must_use]
    pub fn new() -> Self {
        Self {
            webhooks: Arc::new(RwLock::new(HashMap::new())),
            deliveries: Arc::new(RwLock::new(Vec::new())),
            client: reqwest::Client::new(),
        }
    }

    /// Register a new webhook
    ///
    /// # Errors
    ///
    /// Returns an error if the webhook configuration is invalid.
    pub async fn register(&self, config: WebhookConfig) -> Result<WebhookId> {
        let id = config.id.clone();
        self.webhooks.write().await.insert(id.clone(), config);
        Ok(id)
    }

    /// Unregister a webhook
    ///
    /// # Errors
    ///
    /// Returns an error if the storage operation fails.
    pub async fn unregister(&self, id: &WebhookId) -> Result<bool> {
        Ok(self.webhooks.write().await.remove(id).is_some())
    }

    /// Get a webhook by ID
    pub async fn get(&self, id: &WebhookId) -> Option<WebhookConfig> {
        self.webhooks.read().await.get(id).cloned()
    }

    /// List all webhooks
    pub async fn list(&self) -> Vec<WebhookConfig> {
        self.webhooks.read().await.values().cloned().collect()
    }

    /// Update a webhook
    ///
    /// # Errors
    ///
    /// Returns an error if the storage operation fails.
    pub async fn update(&self, id: &WebhookId, config: WebhookConfig) -> Result<bool> {
        let mut webhooks = self.webhooks.write().await;
        if webhooks.contains_key(id) {
            webhooks.insert(id.clone(), config);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Enable/disable a webhook
    ///
    /// # Errors
    ///
    /// Returns an error if the storage operation fails.
    pub async fn set_active(&self, id: &WebhookId, active: bool) -> Result<bool> {
        let mut webhooks = self.webhooks.write().await;
        if let Some(config) = webhooks.get_mut(id) {
            config.active = active;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Trigger an event - sends to all matching webhooks
    pub async fn trigger(&self, event: WebhookEvent, data: serde_json::Value) {
        let webhooks = self.webhooks.read().await;
        let matching_webhooks: Vec<_> = webhooks
            .values()
            .filter(|w| w.subscribes_to(event))
            .cloned()
            .collect();
        drop(webhooks);

        for webhook in matching_webhooks {
            let manager = self.clone();
            let webhook_id = webhook.id.clone();
            let secret = webhook.secret.clone();
            let url = webhook.url.clone();
            let headers = webhook.headers.clone();
            let timeout_secs = webhook.timeout_secs;
            let max_retries = webhook.max_retries;

            let mut payload = WebhookPayload::new(event, data.clone());
            if let Some(ref secret) = secret {
                payload.sign(secret);
            }

            tokio::spawn(async move {
                manager
                    .deliver(DeliveryContext {
                        webhook_id,
                        url,
                        headers,
                        timeout_secs,
                        max_retries,
                        payload,
                        event,
                    })
                    .await;
            });
        }
    }

    /// Deliver a webhook payload
    async fn deliver(&self, ctx: DeliveryContext) {
        let mut delivery = WebhookDelivery::new(ctx.webhook_id.clone(), ctx.event);

        for attempt in 0..=ctx.max_retries {
            delivery.increment_attempt();

            let start = std::time::Instant::now();

            let mut request = self
                .client
                .post(&ctx.url)
                .json(&ctx.payload)
                .timeout(std::time::Duration::from_secs(ctx.timeout_secs));

            for (key, value) in &ctx.headers {
                request = request.header(key, value);
            }

            if let Some(ref sig) = ctx.payload.signature {
                request = request.header("X-Clawdius-Signature", sig);
            }

            request = request
                .header("X-Clawdius-Event", ctx.payload.event.clone())
                .header("X-Clawdius-Delivery", &delivery.delivery_id);

            match request.send().await {
                Ok(response) => {
                    let status = response.status().as_u16();
                    let is_success = response.status().is_success();
                    let body = response.text().await.unwrap_or_default();
                    // Safe cast: webhook durations won't exceed u64::MAX
                    let duration_ms = start.elapsed().as_millis().try_into().unwrap_or(u64::MAX);

                    if is_success {
                        delivery.success(status, body, duration_ms);
                        self.record_delivery(delivery).await;
                        return;
                    }

                    delivery.fail(format!("HTTP {status}: {body}"), duration_ms);
                }
                Err(e) => {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    if e.is_timeout() {
                        delivery.timeout();
                    } else {
                        delivery.fail(e.to_string(), duration_ms);
                    }
                }
            }

            if attempt < ctx.max_retries {
                // Exponential backoff
                let delay_ms = 100 * 2_u64.pow(attempt);
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            }
        }

        self.record_delivery(delivery).await;
    }

    /// Record a delivery
    async fn record_delivery(&self, delivery: WebhookDelivery) {
        let mut deliveries = self.deliveries.write().await;
        deliveries.push(delivery);

        // Keep only last 1000 deliveries
        if deliveries.len() > 1000 {
            deliveries.remove(0);
        }
    }

    /// Get delivery history
    pub async fn get_deliveries(&self, webhook_id: Option<&WebhookId>) -> Vec<WebhookDelivery> {
        let deliveries = self.deliveries.read().await;
        match webhook_id {
            Some(id) => deliveries
                .iter()
                .filter(|d| &d.webhook_id == id)
                .cloned()
                .collect(),
            None => deliveries.clone(),
        }
    }

    /// Get recent deliveries
    pub async fn get_recent_deliveries(&self, limit: usize) -> Vec<WebhookDelivery> {
        let deliveries = self.deliveries.read().await;
        deliveries.iter().rev().take(limit).cloned().collect()
    }

    /// Get delivery statistics
    pub async fn get_stats(&self) -> WebhookStats {
        let deliveries = self.deliveries.read().await;
        let webhooks = self.webhooks.read().await;

        let total_deliveries = deliveries.len();
        let successful = deliveries
            .iter()
            .filter(|d| d.status == DeliveryStatus::Success)
            .count();
        let failed = deliveries
            .iter()
            .filter(|d| d.status == DeliveryStatus::Failed)
            .count();
        let pending = deliveries
            .iter()
            .filter(|d| d.status == DeliveryStatus::Pending)
            .count();
        let timeouts = deliveries
            .iter()
            .filter(|d| d.status == DeliveryStatus::Timeout)
            .count();

        WebhookStats {
            total_webhooks: webhooks.len(),
            active_webhooks: webhooks.values().filter(|w| w.active).count(),
            total_deliveries,
            successful_deliveries: successful,
            failed_deliveries: failed,
            pending_deliveries: pending,
            timeout_deliveries: timeouts,
        }
    }

    /// Clear delivery history
    pub async fn clear_deliveries(&self) {
        self.deliveries.write().await.clear();
    }
}

impl Clone for WebhookManager {
    fn clone(&self) -> Self {
        Self {
            webhooks: self.webhooks.clone(),
            deliveries: self.deliveries.clone(),
            client: self.client.clone(),
        }
    }
}

impl Default for WebhookManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Webhook statistics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WebhookStats {
    /// Total registered webhooks
    pub total_webhooks: usize,
    /// Active webhooks
    pub active_webhooks: usize,
    /// Total deliveries
    pub total_deliveries: usize,
    /// Successful deliveries
    pub successful_deliveries: usize,
    /// Failed deliveries
    pub failed_deliveries: usize,
    /// Pending deliveries
    pub pending_deliveries: usize,
    /// Timeout deliveries
    pub timeout_deliveries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_webhook_manager_register() {
        let manager = WebhookManager::new();
        let config = WebhookConfig::new("test", "https://example.com/hook");

        let id = manager.register(config).await.unwrap();
        let retrieved = manager.get(&id).await;

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test");
    }

    #[tokio::test]
    async fn test_webhook_manager_unregister() {
        let manager = WebhookManager::new();
        let config = WebhookConfig::new("test", "https://example.com/hook");

        let id = manager.register(config).await.unwrap();
        assert!(manager.unregister(&id).await.unwrap());
        assert!(manager.get(&id).await.is_none());
    }

    #[tokio::test]
    async fn test_webhook_manager_list() {
        let manager = WebhookManager::new();

        manager
            .register(WebhookConfig::new("test1", "https://example.com/hook1"))
            .await
            .unwrap();
        manager
            .register(WebhookConfig::new("test2", "https://example.com/hook2"))
            .await
            .unwrap();

        let list = manager.list().await;
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_webhook_stats() {
        let manager = WebhookManager::new();

        manager
            .register(WebhookConfig::new("test", "https://example.com/hook"))
            .await
            .unwrap();

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_webhooks, 1);
        assert_eq!(stats.active_webhooks, 1);
    }
}
