use super::{now_timestamp, AuditEntry};

pub struct AuditEventBuilder {
    event_type: String,
    user_id: Option<String>,
    session_id: Option<String>,
    action: String,
    resource: Option<String>,
    details: serde_json::Value,
}

impl AuditEventBuilder {
    pub fn new(event_type: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            user_id: None,
            session_id: None,
            action: action.into(),
            resource: None,
            details: serde_json::json!({}),
        }
    }

    pub fn user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    pub fn session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    pub fn resource(mut self, resource: impl Into<String>) -> Self {
        self.resource = Some(resource.into());
        self
    }

    #[must_use]
    pub fn details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }

    #[must_use]
    pub fn build(self) -> AuditEntry {
        AuditEntry {
            timestamp: now_timestamp(),
            event_type: self.event_type,
            user_id: self.user_id,
            session_id: self.session_id,
            action: self.action,
            resource: self.resource,
            details: self.details,
            ip_address: None,
            user_agent: None,
        }
    }
}

#[must_use]
pub fn login_event(user_id: &str) -> AuditEntry {
    AuditEventBuilder::new("auth", "login")
        .user(user_id)
        .build()
}

#[must_use]
pub fn chat_event(session_id: &str, model: &str) -> AuditEntry {
    AuditEventBuilder::new("llm", "chat")
        .session(session_id)
        .details(serde_json::json!({ "model": model }))
        .build()
}

#[must_use]
pub fn tool_event(tool: &str, action: &str) -> AuditEntry {
    AuditEventBuilder::new("tool", action)
        .resource(tool)
        .build()
}
