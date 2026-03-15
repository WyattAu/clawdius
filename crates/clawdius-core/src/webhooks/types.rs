//! Webhook types and configurations

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a webhook
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WebhookId(pub String);

impl WebhookId {
    /// Create a new webhook ID
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate a random webhook ID
    #[must_use]
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl std::fmt::Display for WebhookId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Webhook event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEvent {
    /// Session created
    SessionCreated,
    /// Session updated
    SessionUpdated,
    /// Session deleted
    SessionDeleted,
    /// Message sent
    MessageSent,
    /// Message received
    MessageReceived,
    /// Tool executed
    ToolExecuted,
    /// File changed
    FileChanged,
    /// Checkpoint created
    CheckpointCreated,
    /// Checkpoint restored
    CheckpointRestored,
    /// Workflow started
    WorkflowStarted,
    /// Workflow completed
    WorkflowCompleted,
    /// Workflow failed
    WorkflowFailed,
    /// Task started
    TaskStarted,
    /// Task completed
    TaskCompleted,
    /// Task failed
    TaskFailed,
    /// Code generated
    CodeGenerated,
    /// Tests generated
    TestsGenerated,
    /// Error occurred
    ErrorOccurred,
    /// All events
    All,
}

impl WebhookEvent {
    /// Get all event types
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::SessionCreated,
            Self::SessionUpdated,
            Self::SessionDeleted,
            Self::MessageSent,
            Self::MessageReceived,
            Self::ToolExecuted,
            Self::FileChanged,
            Self::CheckpointCreated,
            Self::CheckpointRestored,
            Self::WorkflowStarted,
            Self::WorkflowCompleted,
            Self::WorkflowFailed,
            Self::TaskStarted,
            Self::TaskCompleted,
            Self::TaskFailed,
            Self::CodeGenerated,
            Self::TestsGenerated,
            Self::ErrorOccurred,
        ]
    }

    /// Get event name as string
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SessionCreated => "session.created",
            Self::SessionUpdated => "session.updated",
            Self::SessionDeleted => "session.deleted",
            Self::MessageSent => "message.sent",
            Self::MessageReceived => "message.received",
            Self::ToolExecuted => "tool.executed",
            Self::FileChanged => "file.changed",
            Self::CheckpointCreated => "checkpoint.created",
            Self::CheckpointRestored => "checkpoint.restored",
            Self::WorkflowStarted => "workflow.started",
            Self::WorkflowCompleted => "workflow.completed",
            Self::WorkflowFailed => "workflow.failed",
            Self::TaskStarted => "task.started",
            Self::TaskCompleted => "task.completed",
            Self::TaskFailed => "task.failed",
            Self::CodeGenerated => "code.generated",
            Self::TestsGenerated => "tests.generated",
            Self::ErrorOccurred => "error.occurred",
            Self::All => "*",
        }
    }
}

impl std::fmt::Display for WebhookEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Unique identifier
    pub id: WebhookId,
    /// Webhook name
    pub name: String,
    /// Target URL
    pub url: String,
    /// Secret for signature verification
    pub secret: Option<String>,
    /// Events to subscribe to
    pub events: Vec<WebhookEvent>,
    /// Whether webhook is active
    pub active: bool,
    /// Custom headers
    pub headers: HashMap<String, String>,
    /// Timeout in seconds
    pub timeout_secs: u64,
    /// Maximum retries
    pub max_retries: u32,
    /// Created timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl WebhookConfig {
    /// Create a new webhook configuration
    #[must_use]
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            id: WebhookId::generate(),
            name: name.into(),
            url: url.into(),
            secret: None,
            events: vec![WebhookEvent::All],
            active: true,
            headers: HashMap::new(),
            timeout_secs: 30,
            max_retries: 3,
            created_at: chrono::Utc::now(),
        }
    }

    /// Set the secret
    #[must_use]
    pub fn with_secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
    }

    /// Set the events
    #[must_use]
    pub fn with_events(mut self, events: Vec<WebhookEvent>) -> Self {
        self.events = events;
        self
    }

    /// Add a custom header
    #[must_use]
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Set timeout
    #[must_use]
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set max retries
    #[must_use]
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Check if webhook subscribes to an event
    #[must_use]
    pub fn subscribes_to(&self, event: WebhookEvent) -> bool {
        self.active && (self.events.contains(&WebhookEvent::All) || self.events.contains(&event))
    }
}

/// Webhook payload sent to endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    /// Unique delivery ID
    pub delivery_id: String,
    /// Event type
    pub event: String,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Event data
    pub data: serde_json::Value,
    /// Signature (HMAC-SHA256)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

impl WebhookPayload {
    /// Create a new webhook payload
    #[must_use]
    pub fn new(event: WebhookEvent, data: serde_json::Value) -> Self {
        Self {
            delivery_id: uuid::Uuid::new_v4().to_string(),
            event: event.as_str().to_string(),
            timestamp: chrono::Utc::now(),
            data,
            signature: None,
        }
    }

    /// Sign the payload with a secret
    pub fn sign(&mut self, secret: &str) {
        use sha3::{Digest, Sha3_256};
        let payload = serde_json::to_string(&self.data).unwrap_or_default();
        let mut hasher = Sha3_256::new();
        hasher.update(secret.as_bytes());
        hasher.update(payload.as_bytes());
        let result = hasher.finalize();
        self.signature = Some(hex_encode(result.as_slice()));
    }

    /// Verify the payload signature
    #[must_use]
    pub fn verify(&self, secret: &str) -> bool {
        use sha3::{Digest, Sha3_256};

        let Some(ref sig) = self.signature else {
            return false;
        };

        let payload = serde_json::to_string(&self.data).unwrap_or_default();
        let mut hasher = Sha3_256::new();
        hasher.update(secret.as_bytes());
        hasher.update(payload.as_bytes());
        let result = hasher.finalize();
        let expected = hex_encode(result.as_slice());

        sig == &expected
    }
}

/// Simple hex encoding without external crate
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Delivery status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    /// Delivery pending
    Pending,
    /// Delivery successful
    Success,
    /// Delivery failed
    Failed,
    /// Delivery timed out
    Timeout,
}

/// Webhook delivery record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDelivery {
    /// Delivery ID
    pub delivery_id: String,
    /// Webhook ID
    pub webhook_id: WebhookId,
    /// Event type
    pub event: WebhookEvent,
    /// Status
    pub status: DeliveryStatus,
    /// HTTP status code (if available)
    pub http_status: Option<u16>,
    /// Response body (if available)
    pub response: Option<String>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Attempt number
    pub attempt: u32,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

impl WebhookDelivery {
    /// Create a new delivery record
    #[must_use]
    pub fn new(webhook_id: WebhookId, event: WebhookEvent) -> Self {
        Self {
            delivery_id: uuid::Uuid::new_v4().to_string(),
            webhook_id,
            event,
            status: DeliveryStatus::Pending,
            http_status: None,
            response: None,
            error: None,
            attempt: 0,
            timestamp: chrono::Utc::now(),
            duration_ms: 0,
        }
    }

    /// Mark as successful
    pub fn success(&mut self, http_status: u16, response: String, duration_ms: u64) {
        self.status = DeliveryStatus::Success;
        self.http_status = Some(http_status);
        self.response = Some(response);
        self.duration_ms = duration_ms;
    }

    /// Mark as failed
    pub fn fail(&mut self, error: String, duration_ms: u64) {
        self.status = DeliveryStatus::Failed;
        self.error = Some(error);
        self.duration_ms = duration_ms;
    }

    /// Mark as timed out
    pub fn timeout(&mut self) {
        self.status = DeliveryStatus::Timeout;
        self.error = Some("Request timed out".to_string());
    }

    /// Increment attempt counter
    pub fn increment_attempt(&mut self) {
        self.attempt += 1;
        self.timestamp = chrono::Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_config_subscribes_to() {
        let config = WebhookConfig::new("test", "https://example.com/hook").with_events(vec![
            WebhookEvent::SessionCreated,
            WebhookEvent::MessageSent,
        ]);

        assert!(config.subscribes_to(WebhookEvent::SessionCreated));
        assert!(config.subscribes_to(WebhookEvent::MessageSent));
        assert!(!config.subscribes_to(WebhookEvent::ToolExecuted));
    }

    #[test]
    fn test_webhook_config_all_events() {
        let config = WebhookConfig::new("test", "https://example.com/hook")
            .with_events(vec![WebhookEvent::All]);

        assert!(config.subscribes_to(WebhookEvent::SessionCreated));
        assert!(config.subscribes_to(WebhookEvent::ToolExecuted));
    }

    #[test]
    fn test_webhook_payload_sign_verify() {
        let mut payload = WebhookPayload::new(
            WebhookEvent::SessionCreated,
            serde_json::json!({"id": "123"}),
        );
        let secret = "my-secret";

        payload.sign(secret);
        assert!(payload.signature.is_some());
        assert!(payload.verify(secret));
        assert!(!payload.verify("wrong-secret"));
    }

    #[test]
    fn test_webhook_delivery() {
        let mut delivery =
            WebhookDelivery::new(WebhookId::new("test"), WebhookEvent::SessionCreated);

        assert_eq!(delivery.status, DeliveryStatus::Pending);

        delivery.success(200, "OK".to_string(), 150);
        assert_eq!(delivery.status, DeliveryStatus::Success);
        assert_eq!(delivery.http_status, Some(200));
        assert_eq!(delivery.duration_ms, 150);
    }
}
