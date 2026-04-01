#![deny(unsafe_code)]

//! Structured Audit Logging for the Messaging Pipeline
//!
//! Bridges the messaging system to the enterprise audit infrastructure,
//! converting messaging-specific events into enterprise `AuditEvent` records.

use crate::enterprise::audit::{
    Actor, ActorType, AuditCategory, AuditEvent, AuditLogger, AuditOutcome, AuditSeverity,
    AuditStorage, Resource,
};
use crate::messaging::types::Platform;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagingAuditEvent {
    MessageReceived {
        platform: Platform,
        user_id: String,
        chat_id: String,
        message_length: usize,
    },
    CommandExecuted {
        platform: Platform,
        user_id: String,
        command: String,
        category: String,
        success: bool,
        latency_ms: u64,
    },
    RateLimitHit {
        platform: Platform,
        user_id: String,
        limit: u32,
        remaining: u32,
    },
    PermissionDenied {
        platform: Platform,
        user_id: String,
        command: String,
        required_permission: String,
    },
    SessionBound {
        platform: Platform,
        user_id: String,
        session_id: String,
        is_new: bool,
    },
    SessionClosed {
        platform: Platform,
        user_id: String,
        session_id: String,
        reason: String,
    },
    LlmCallInitiated {
        platform: Platform,
        user_id: String,
        session_id: String,
        model: String,
        prompt_length: usize,
    },
    LlmCallCompleted {
        platform: Platform,
        user_id: String,
        session_id: String,
        model: String,
        latency_ms: u64,
        tokens_used: u32,
        success: bool,
    },
    ResponseSent {
        platform: Platform,
        user_id: String,
        chat_id: String,
        message_ids: Vec<String>,
        was_streaming: bool,
        total_chunks: usize,
    },
    WebhookAuthFailed {
        platform: Platform,
        reason: String,
    },
    WebhookSignatureFailed {
        platform: Platform,
        reason: String,
    },
    WebhookParseFailed {
        platform: Platform,
        reason: String,
    },
    ChannelRegistered {
        platform: Platform,
    },
    ChannelError {
        platform: Platform,
        operation: String,
        error: String,
    },
}

impl MessagingAuditEvent {
    fn action_name(&self) -> &'static str {
        match self {
            Self::MessageReceived { .. } => "messaging.message_received",
            Self::CommandExecuted { .. } => "messaging.command_executed",
            Self::RateLimitHit { .. } => "messaging.rate_limit_hit",
            Self::PermissionDenied { .. } => "messaging.permission_denied",
            Self::SessionBound { .. } => "messaging.session_bound",
            Self::SessionClosed { .. } => "messaging.session_closed",
            Self::LlmCallInitiated { .. } => "messaging.llm_call_initiated",
            Self::LlmCallCompleted { .. } => "messaging.llm_call_completed",
            Self::ResponseSent { .. } => "messaging.response_sent",
            Self::WebhookAuthFailed { .. } => "messaging.webhook_auth_failed",
            Self::WebhookSignatureFailed { .. } => "messaging.webhook_signature_failed",
            Self::WebhookParseFailed { .. } => "messaging.webhook_parse_failed",
            Self::ChannelRegistered { .. } => "messaging.channel_registered",
            Self::ChannelError { .. } => "messaging.channel_error",
        }
    }

    fn category(&self) -> AuditCategory {
        match self {
            Self::WebhookAuthFailed { .. } | Self::WebhookSignatureFailed { .. } => {
                AuditCategory::Authentication
            },
            Self::PermissionDenied { .. } => AuditCategory::Authorization,
            Self::LlmCallInitiated { .. } | Self::LlmCallCompleted { .. } => {
                AuditCategory::DataAccess
            },
            Self::RateLimitHit { .. } => AuditCategory::Security,
            Self::WebhookParseFailed { .. }
            | Self::ChannelError { .. }
            | Self::ChannelRegistered { .. } => AuditCategory::Security,
            Self::MessageReceived { .. }
            | Self::CommandExecuted { .. }
            | Self::SessionBound { .. }
            | Self::SessionClosed { .. }
            | Self::ResponseSent { .. } => AuditCategory::System,
        }
    }

    fn severity(&self) -> AuditSeverity {
        match self {
            Self::WebhookAuthFailed { .. }
            | Self::WebhookSignatureFailed { .. }
            | Self::RateLimitHit { .. }
            | Self::PermissionDenied { .. } => AuditSeverity::Warning,
            Self::ChannelError { .. } => AuditSeverity::Error,
            _ => AuditSeverity::Info,
        }
    }

    fn outcome(&self) -> AuditOutcome {
        match self {
            Self::WebhookAuthFailed { .. }
            | Self::WebhookSignatureFailed { .. }
            | Self::PermissionDenied { .. } => AuditOutcome::Denied,
            Self::ChannelError { .. } => AuditOutcome::Failure,
            Self::CommandExecuted { success, .. } | Self::LlmCallCompleted { success, .. } => {
                if *success {
                    AuditOutcome::Success
                } else {
                    AuditOutcome::Failure
                }
            },
            _ => AuditOutcome::Success,
        }
    }

    fn actor_id(&self) -> Option<&str> {
        match self {
            Self::MessageReceived { user_id, .. }
            | Self::CommandExecuted { user_id, .. }
            | Self::RateLimitHit { user_id, .. }
            | Self::PermissionDenied { user_id, .. }
            | Self::SessionBound { user_id, .. }
            | Self::SessionClosed { user_id, .. }
            | Self::LlmCallInitiated { user_id, .. }
            | Self::LlmCallCompleted { user_id, .. }
            | Self::ResponseSent { user_id, .. } => Some(user_id),
            _ => None,
        }
    }

    fn platform(&self) -> Option<Platform> {
        match self {
            Self::MessageReceived { platform, .. }
            | Self::CommandExecuted { platform, .. }
            | Self::RateLimitHit { platform, .. }
            | Self::PermissionDenied { platform, .. }
            | Self::SessionBound { platform, .. }
            | Self::SessionClosed { platform, .. }
            | Self::LlmCallInitiated { platform, .. }
            | Self::LlmCallCompleted { platform, .. }
            | Self::ResponseSent { platform, .. }
            | Self::WebhookAuthFailed { platform, .. }
            | Self::WebhookSignatureFailed { platform, .. }
            | Self::WebhookParseFailed { platform, .. }
            | Self::ChannelRegistered { platform }
            | Self::ChannelError { platform, .. } => Some(*platform),
        }
    }

    fn session_id(&self) -> Option<&str> {
        match self {
            Self::SessionBound { session_id, .. }
            | Self::SessionClosed { session_id, .. }
            | Self::LlmCallInitiated { session_id, .. }
            | Self::LlmCallCompleted { session_id, .. } => Some(session_id),
            _ => None,
        }
    }

    fn details(&self) -> HashMap<String, serde_json::Value> {
        let mut d = HashMap::new();
        if let Some(platform) = self.platform() {
            d.insert("platform".to_string(), serde_json::json!(platform.as_str()));
        }
        if let Some(user_id) = self.actor_id() {
            d.insert("user_id".to_string(), serde_json::json!(user_id));
        }
        if let Some(sid) = self.session_id() {
            d.insert("session_id".to_string(), serde_json::json!(sid));
        }
        match self {
            Self::MessageReceived {
                chat_id,
                message_length,
                ..
            } => {
                d.insert("chat_id".to_string(), serde_json::json!(chat_id));
                d.insert(
                    "message_length".to_string(),
                    serde_json::json!(message_length),
                );
            },
            Self::CommandExecuted {
                command,
                category,
                success,
                latency_ms,
                ..
            } => {
                d.insert("command".to_string(), serde_json::json!(command));
                d.insert("category".to_string(), serde_json::json!(category));
                d.insert("success".to_string(), serde_json::json!(success));
                d.insert("latency_ms".to_string(), serde_json::json!(latency_ms));
            },
            Self::RateLimitHit {
                limit, remaining, ..
            } => {
                d.insert("limit".to_string(), serde_json::json!(limit));
                d.insert("remaining".to_string(), serde_json::json!(remaining));
            },
            Self::PermissionDenied {
                command,
                required_permission,
                ..
            } => {
                d.insert("command".to_string(), serde_json::json!(command));
                d.insert(
                    "required_permission".to_string(),
                    serde_json::json!(required_permission),
                );
            },
            Self::SessionBound { is_new, .. } => {
                d.insert("is_new".to_string(), serde_json::json!(is_new));
            },
            Self::SessionClosed { reason, .. } => {
                d.insert("reason".to_string(), serde_json::json!(reason));
            },
            Self::LlmCallInitiated {
                model,
                prompt_length,
                ..
            } => {
                d.insert("model".to_string(), serde_json::json!(model));
                d.insert(
                    "prompt_length".to_string(),
                    serde_json::json!(prompt_length),
                );
            },
            Self::LlmCallCompleted {
                model,
                latency_ms,
                tokens_used,
                success,
                ..
            } => {
                d.insert("model".to_string(), serde_json::json!(model));
                d.insert("latency_ms".to_string(), serde_json::json!(latency_ms));
                d.insert("tokens_used".to_string(), serde_json::json!(tokens_used));
                d.insert("success".to_string(), serde_json::json!(success));
            },
            Self::ResponseSent {
                chat_id,
                message_ids,
                was_streaming,
                total_chunks,
                ..
            } => {
                d.insert("chat_id".to_string(), serde_json::json!(chat_id));
                d.insert("message_ids".to_string(), serde_json::json!(message_ids));
                d.insert(
                    "was_streaming".to_string(),
                    serde_json::json!(was_streaming),
                );
                d.insert("total_chunks".to_string(), serde_json::json!(total_chunks));
            },
            Self::WebhookAuthFailed { reason, .. }
            | Self::WebhookSignatureFailed { reason, .. }
            | Self::WebhookParseFailed { reason, .. } => {
                d.insert("reason".to_string(), serde_json::json!(reason));
            },
            Self::ChannelRegistered { .. } => {},
            Self::ChannelError {
                operation, error, ..
            } => {
                d.insert("operation".to_string(), serde_json::json!(operation));
                d.insert("error".to_string(), serde_json::json!(error));
            },
        }
        d
    }

    fn to_audit_event(&self) -> AuditEvent {
        let actor = match self.actor_id() {
            Some(uid) => Actor {
                actor_type: ActorType::User,
                id: uid.to_string(),
                name: None,
                email: None,
                roles: Vec::new(),
            },
            None => Actor {
                actor_type: ActorType::System,
                id: "messaging-gateway".to_string(),
                name: None,
                email: None,
                roles: Vec::new(),
            },
        };

        let resource = match self {
            Self::MessageReceived { chat_id, .. } | Self::ResponseSent { chat_id, .. } => {
                Some(Resource {
                    resource_type: "chat".to_string(),
                    id: chat_id.clone(),
                    name: None,
                    path: None,
                })
            },
            Self::ChannelRegistered { platform } | Self::ChannelError { platform, .. } => {
                Some(Resource {
                    resource_type: "channel".to_string(),
                    id: platform.as_str().to_string(),
                    name: None,
                    path: None,
                })
            },
            _ => None,
        };

        let mut event = AuditEvent::new(self.category(), self.action_name(), actor)
            .with_severity(self.severity())
            .with_outcome(self.outcome());

        if let Some(res) = resource {
            event = event.with_resource(res);
        }
        if let Some(sid) = self.session_id() {
            event = event.with_session(sid);
        }
        for (k, v) in self.details() {
            event = event.with_detail(k, v);
        }
        event
    }
}

pub struct MessagingAuditLogger {
    inner: Arc<RwLock<AuditLogger>>,
}

impl MessagingAuditLogger {
    #[must_use]
    pub fn new(storage: AuditStorage) -> Self {
        Self {
            inner: Arc::new(RwLock::new(AuditLogger::new(storage))),
        }
    }

    pub async fn log_event(&self, event: MessagingAuditEvent) -> anyhow::Result<()> {
        let audit_event = event.to_audit_event();

        match event.severity() {
            AuditSeverity::Critical => {
                error!(event = %event.action_name(), "audit: critical messaging event")
            },
            AuditSeverity::Error => {
                error!(event = %event.action_name(), "audit: error messaging event")
            },
            AuditSeverity::Warning => {
                warn!(event = %event.action_name(), "audit: warning messaging event")
            },
            AuditSeverity::Info => {
                debug!(event = %event.action_name(), "audit: info messaging event")
            },
        }

        let guard = self.inner.write().await;
        guard.log(audit_event).await
    }
}

impl Clone for MessagingAuditLogger {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_event() -> MessagingAuditEvent {
        MessagingAuditEvent::MessageReceived {
            platform: Platform::Telegram,
            user_id: "user1".to_string(),
            chat_id: "chat1".to_string(),
            message_length: 42,
        }
    }

    fn make_event_audit(event: &MessagingAuditEvent) -> AuditEvent {
        event.to_audit_event()
    }

    #[test]
    fn message_received_maps_to_system() {
        let e = test_event();
        let audit = make_event_audit(&e);
        assert_eq!(audit.category, AuditCategory::System);
        assert_eq!(audit.outcome, AuditOutcome::Success);
    }

    #[test]
    fn command_executed_maps_to_system() {
        let e = MessagingAuditEvent::CommandExecuted {
            platform: Platform::Discord,
            user_id: "u2".to_string(),
            command: "/clawd status".to_string(),
            category: "status".to_string(),
            success: true,
            latency_ms: 10,
        };
        let audit = make_event_audit(&e);
        assert_eq!(audit.category, AuditCategory::System);
        assert_eq!(audit.outcome, AuditOutcome::Success);
        assert_eq!(audit.details.get("latency_ms").unwrap(), 10);
    }

    #[test]
    fn command_executed_failure_maps_correctly() {
        let e = MessagingAuditEvent::CommandExecuted {
            platform: Platform::Slack,
            user_id: "u3".to_string(),
            command: "/clawd admin".to_string(),
            category: "admin".to_string(),
            success: false,
            latency_ms: 5,
        };
        let audit = make_event_audit(&e);
        assert_eq!(audit.category, AuditCategory::System);
        assert_eq!(audit.outcome, AuditOutcome::Failure);
    }

    #[test]
    fn rate_limit_maps_to_security_warning() {
        let e = MessagingAuditEvent::RateLimitHit {
            platform: Platform::Telegram,
            user_id: "fast_user".to_string(),
            limit: 20,
            remaining: 0,
        };
        let audit = make_event_audit(&e);
        assert_eq!(audit.category, AuditCategory::Security);
        assert_eq!(audit.severity, AuditSeverity::Warning);
    }

    #[test]
    fn permission_denied_maps_to_authorization() {
        let e = MessagingAuditEvent::PermissionDenied {
            platform: Platform::Discord,
            user_id: "u4".to_string(),
            command: "/clawd admin".to_string(),
            required_permission: "can_admin".to_string(),
        };
        let audit = make_event_audit(&e);
        assert_eq!(audit.category, AuditCategory::Authorization);
        assert_eq!(audit.severity, AuditSeverity::Warning);
        assert_eq!(audit.outcome, AuditOutcome::Denied);
    }

    #[test]
    fn webhook_auth_failed_has_denied_outcome() {
        let e = MessagingAuditEvent::WebhookAuthFailed {
            platform: Platform::Webhook,
            reason: "invalid token".to_string(),
        };
        let audit = make_event_audit(&e);
        assert_eq!(audit.category, AuditCategory::Authentication);
        assert_eq!(audit.outcome, AuditOutcome::Denied);
    }

    #[test]
    fn webhook_signature_failed_has_denied_outcome() {
        let e = MessagingAuditEvent::WebhookSignatureFailed {
            platform: Platform::Webhook,
            reason: "signature mismatch".to_string(),
        };
        let audit = make_event_audit(&e);
        assert_eq!(audit.category, AuditCategory::Authentication);
        assert_eq!(audit.outcome, AuditOutcome::Denied);
    }

    #[test]
    fn webhook_parse_failed_maps_to_security() {
        let e = MessagingAuditEvent::WebhookParseFailed {
            platform: Platform::Webhook,
            reason: "malformed json".to_string(),
        };
        let audit = make_event_audit(&e);
        assert_eq!(audit.category, AuditCategory::Security);
    }

    #[test]
    fn llm_call_initiated_maps_to_data_access() {
        let e = MessagingAuditEvent::LlmCallInitiated {
            platform: Platform::Telegram,
            user_id: "u5".to_string(),
            session_id: "sess1".to_string(),
            model: "gpt-4".to_string(),
            prompt_length: 500,
        };
        let audit = make_event_audit(&e);
        assert_eq!(audit.category, AuditCategory::DataAccess);
        assert_eq!(audit.details.get("model").unwrap(), "gpt-4");
        assert_eq!(audit.session_id.as_deref(), Some("sess1"));
    }

    #[test]
    fn llm_call_completed_maps_to_data_access() {
        let e = MessagingAuditEvent::LlmCallCompleted {
            platform: Platform::Telegram,
            user_id: "u5".to_string(),
            session_id: "sess1".to_string(),
            model: "gpt-4".to_string(),
            latency_ms: 2000,
            tokens_used: 150,
            success: true,
        };
        let audit = make_event_audit(&e);
        assert_eq!(audit.category, AuditCategory::DataAccess);
        assert_eq!(audit.outcome, AuditOutcome::Success);
        assert_eq!(audit.details.get("tokens_used").unwrap(), 150);
    }

    #[test]
    fn response_sent_details_populated() {
        let e = MessagingAuditEvent::ResponseSent {
            platform: Platform::Discord,
            user_id: "u6".to_string(),
            chat_id: "ch2".to_string(),
            message_ids: vec!["msg1".to_string(), "msg2".to_string()],
            was_streaming: true,
            total_chunks: 5,
        };
        let audit = make_event_audit(&e);
        assert_eq!(audit.category, AuditCategory::System);
        let ids = audit.details.get("message_ids").unwrap();
        assert_eq!(ids.as_array().map(|a| a.len()), Some(2));
        assert_eq!(audit.details.get("total_chunks").unwrap(), 5);
    }

    #[test]
    fn channel_error_maps_to_failure() {
        let e = MessagingAuditEvent::ChannelError {
            platform: Platform::Matrix,
            operation: "send_message".to_string(),
            error: "connection refused".to_string(),
        };
        let audit = make_event_audit(&e);
        assert_eq!(audit.category, AuditCategory::Security);
        assert_eq!(audit.outcome, AuditOutcome::Failure);
        assert_eq!(audit.severity, AuditSeverity::Error);
    }

    #[test]
    fn channel_registered_maps_to_security() {
        let e = MessagingAuditEvent::ChannelRegistered {
            platform: Platform::Slack,
        };
        let audit = make_event_audit(&e);
        assert_eq!(audit.category, AuditCategory::Security);
        assert_eq!(audit.outcome, AuditOutcome::Success);
    }

    #[test]
    fn actor_is_user_when_user_id_present() {
        let e = test_event();
        let audit = make_event_audit(&e);
        assert!(matches!(audit.actor.actor_type, ActorType::User));
        assert_eq!(audit.actor.id, "user1");
    }

    #[test]
    fn actor_is_system_when_no_user_id() {
        let e = MessagingAuditEvent::ChannelRegistered {
            platform: Platform::Telegram,
        };
        let audit = make_event_audit(&e);
        assert!(matches!(audit.actor.actor_type, ActorType::System));
    }

    #[test]
    fn logger_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MessagingAuditLogger>();
    }

    #[tokio::test]
    async fn log_event_roundtrip() {
        let logger = MessagingAuditLogger::new(AuditStorage::File {
            path: std::env::temp_dir().join("audit_test_roundtrip.log"),
        });
        let result = logger
            .log_event(MessagingAuditEvent::MessageReceived {
                platform: Platform::Telegram,
                user_id: "test".to_string(),
                chat_id: "c1".to_string(),
                message_length: 10,
            })
            .await;
        assert!(result.is_ok());
    }
}
