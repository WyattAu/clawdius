#![deny(unsafe_code)]

//! PII Redaction Layer for `tracing-subscriber`
//!
//! Provides a [`tracing_subscriber::Layer`] that automatically redacts sensitive fields
//! and credential-like values from log output.

use regex::Regex;
use std::fmt;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

const DEFAULT_REPLACEMENT: &str = "[REDACTED]";
const REDACTED_USER_CONTENT: &str = "[REDACTED:USER_CONTENT]";
const REDACTED_ID: &str = "[REDACTED:ID]";
const REDACTED_IP: &str = "[REDACTED:IP]";
const REDACTED_KEY: &str = "[REDACTED:KEY]";
const REDACTED_PHONE: &str = "[REDACTED:PHONE]";

const SENSITIVE_FIELD_NAMES: &[&str] = &[
    "token",
    "api_key",
    "apikey",
    "secret",
    "password",
    "passwd",
    "credential",
    "authorization",
    "auth",
    "bearer",
    "session_token",
    "access_token",
    "refresh_token",
    "client_secret",
    "signing_secret",
    "app_secret",
    "verify_token",
    "content",
    "user_id",
    "username",
    "ip_address",
    "source_ip",
];

/// Configuration for [`PiiRedactionLayer`].
#[derive(Debug, Clone)]
pub struct PiiRedactionConfig {
    pub redact_field_names: bool,
    pub redact_value_patterns: bool,
    pub extra_sensitive_fields: Vec<String>,
    pub allowed_fields: Vec<String>,
    pub replacement: String,
}

impl Default for PiiRedactionConfig {
    fn default() -> Self {
        Self {
            redact_field_names: true,
            redact_value_patterns: true,
            extra_sensitive_fields: Vec::new(),
            allowed_fields: Vec::new(),
            replacement: DEFAULT_REPLACEMENT.to_owned(),
        }
    }
}

impl PiiRedactionConfig {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

struct CompiledPatterns {
    uuid_re: Regex,
    bearer_re: Regex,
    api_key_re: Regex,
    email_re: Regex,
    phone_re: Regex,
}

impl CompiledPatterns {
    fn new() -> Self {
        Self {
            uuid_re: Regex::new(
                r"(?i)[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
            )
            .unwrap_or_else(|_| Regex::new(r"$^").unwrap()),
            bearer_re: Regex::new(r"(?i)Bearer\s+[A-Za-z0-9\-._~+/]+=*")
                .unwrap_or_else(|_| Regex::new(r"$^").unwrap()),
            api_key_re: Regex::new(r"[A-Za-z0-9]{32,}")
                .unwrap_or_else(|_| Regex::new(r"$^").unwrap()),
            email_re: Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}")
                .unwrap_or_else(|_| Regex::new(r"$^").unwrap()),
            phone_re: Regex::new(r"\+?[1-9]\d{6,14}")
                .unwrap_or_else(|_| Regex::new(r"$^").unwrap()),
        }
    }
}

/// A [`tracing_subscriber::Layer`] that redacts PII and sensitive values from log events.
pub struct PiiRedactionLayer {
    config: PiiRedactionConfig,
    patterns: CompiledPatterns,
}

impl PiiRedactionLayer {
    #[must_use]
    pub fn new(config: PiiRedactionConfig) -> Self {
        Self {
            config,
            patterns: CompiledPatterns::new(),
        }
    }

    fn is_sensitive_field(&self, field_name: &str) -> bool {
        let lower = field_name.to_ascii_lowercase();
        if self
            .config
            .allowed_fields
            .iter()
            .any(|a| a.eq_ignore_ascii_case(field_name))
        {
            return false;
        }
        if SENSITIVE_FIELD_NAMES.iter().any(|s| *s == lower.as_str()) {
            return true;
        }
        self.config
            .extra_sensitive_fields
            .iter()
            .any(|e| e.eq_ignore_ascii_case(field_name))
    }

    fn redact_value_patterns(&self, value: &str) -> String {
        // Skip pattern matching if the value looks like a UUID
        if self.patterns.uuid_re.is_match(value) {
            return value.to_owned();
        }

        let mut result = value.to_owned();
        if self.patterns.bearer_re.is_match(&result) {
            result = self
                .patterns
                .bearer_re
                .replace(&result, "Bearer [REDACTED]")
                .to_string();
            tracing::debug!("redacted bearer token in log value");
        }
        if self.patterns.email_re.is_match(&result) {
            result = self
                .patterns
                .email_re
                .replace(&result, "user@[REDACTED]")
                .to_string();
            tracing::debug!("redacted email address in log value");
        }
        if self.patterns.phone_re.is_match(&result) {
            result = self
                .patterns
                .phone_re
                .replace(&result, REDACTED_PHONE)
                .to_string();
            tracing::debug!("redacted phone number in log value");
        }
        // API key pattern only matches if no UUID-like content is present
        if self.patterns.api_key_re.is_match(&result) && !self.patterns.uuid_re.is_match(&result) {
            result = self
                .patterns
                .api_key_re
                .replace_all(&result, REDACTED_KEY)
                .to_string();
            tracing::debug!("redacted api-key-like value in log value");
        }
        result
    }

    fn format_value(&self, field_name: &str, value: &dyn fmt::Debug) -> String {
        let value_str = format!("{value:?}");
        if self.config.redact_field_names && self.is_sensitive_field(field_name) {
            let replacement = self.replacement_for_field(field_name);
            tracing::debug!(field_name, "redacted sensitive field");
            return replacement;
        }
        if !self.config.redact_value_patterns {
            return value_str;
        }
        // Strip surrounding quotes from Debug-formatted &str values
        // so pattern matching works on the raw content.
        let inner = value_str
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(&value_str);
        let redacted = self.redact_value_patterns(inner);
        // If something was redacted, return the raw redacted string (no quotes).
        // Otherwise return the original Debug-formatted string.
        if redacted != inner {
            redacted
        } else {
            value_str
        }
    }

    /// Determine the replacement string for a sensitive field name.
    /// Category-specific replacements (USER_CONTENT, ID, IP) are always used.
    /// For other sensitive fields, the config's `replacement` is used.
    fn replacement_for_field(&self, field_name: &str) -> String {
        let lower = field_name.to_ascii_lowercase();
        if lower == "content" {
            REDACTED_USER_CONTENT.to_owned()
        } else if lower == "user_id" || lower == "username" {
            REDACTED_ID.to_owned()
        } else if lower == "ip_address" || lower == "source_ip" {
            REDACTED_IP.to_owned()
        } else {
            self.config.replacement.clone()
        }
    }
}

impl<S> Layer<S> for PiiRedactionLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = RedactingVisitor {
            layer: self,
            fields: Vec::new(),
        };
        event.record(&mut visitor);

        let message = format!(
            "{}{}",
            visitor
                .fields
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(", "),
            if visitor.fields.is_empty() {
                String::new()
            } else {
                " ".to_owned()
            }
        );

        eprintln!("{}", message);
    }
}

struct RedactingVisitor<'a> {
    layer: &'a PiiRedactionLayer,
    fields: Vec<(String, String)>,
}

impl<'a> tracing::field::Visit for RedactingVisitor<'a> {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        let redacted = self.layer.format_value(field.name(), &value);
        self.fields.push((field.name().to_owned(), redacted));
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        let redacted = self.layer.format_value(field.name(), value);
        self.fields.push((field.name().to_owned(), redacted));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.record_debug(field, &value);
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.record_debug(field, &value);
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.record_debug(field, &value);
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.record_debug(field, &value);
    }
}

/// Creates a complete subscriber layer with PII redaction applied.
///
/// Returns a layered subscriber combining [`PiiRedactionLayer`] with a
/// [`tracing_subscriber::fmt::Layer`].
#[must_use]
pub fn setup_pii_redaction(
    config: PiiRedactionConfig,
) -> tracing_subscriber::layer::Layered<
    PiiRedactionLayer,
    tracing_subscriber::fmt::Layer<tracing_subscriber::Registry>,
    tracing_subscriber::Registry,
> {
    let pii_layer = PiiRedactionLayer::new(config);
    tracing_subscriber::fmt::Layer::new().and_then(pii_layer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_layer() -> PiiRedactionLayer {
        PiiRedactionLayer::new(PiiRedactionConfig::default())
    }

    #[test]
    fn test_default_config() {
        let config = PiiRedactionConfig::default();
        assert!(config.redact_field_names);
        assert!(config.redact_value_patterns);
        assert!(config.extra_sensitive_fields.is_empty());
        assert!(config.allowed_fields.is_empty());
        assert_eq!(config.replacement, DEFAULT_REPLACEMENT);
    }

    #[test]
    fn test_redact_token_field() {
        let layer = default_layer();
        assert!(layer.is_sensitive_field("token"));
    }

    #[test]
    fn test_redact_api_key_field() {
        let layer = default_layer();
        assert!(layer.is_sensitive_field("api_key"));
        assert!(layer.is_sensitive_field("apikey"));
    }

    #[test]
    fn test_redact_secret_field() {
        let layer = default_layer();
        assert!(layer.is_sensitive_field("secret"));
        assert!(layer.is_sensitive_field("client_secret"));
        assert!(layer.is_sensitive_field("signing_secret"));
    }

    #[test]
    fn test_redact_auth_fields() {
        let layer = default_layer();
        assert!(layer.is_sensitive_field("authorization"));
        assert!(layer.is_sensitive_field("auth"));
        assert!(layer.is_sensitive_field("bearer"));
        assert!(layer.is_sensitive_field("access_token"));
        assert!(layer.is_sensitive_field("refresh_token"));
    }

    #[test]
    fn test_content_field_redaction() {
        let layer = default_layer();
        let result = layer.format_value("content", &"hello world");
        assert_eq!(result, REDACTED_USER_CONTENT);
    }

    #[test]
    fn test_user_id_field_redaction() {
        let layer = default_layer();
        let result = layer.format_value("user_id", &"12345");
        assert_eq!(result, REDACTED_ID);
    }

    #[test]
    fn test_ip_address_field_redaction() {
        let layer = default_layer();
        let result = layer.format_value("ip_address", &"192.168.1.1");
        assert_eq!(result, REDACTED_IP);
    }

    #[test]
    fn test_bearer_token_value_redaction() {
        let layer = default_layer();
        let result = layer.format_value("header", &"Bearer abc123def456");
        assert_eq!(result, "Bearer [REDACTED]");
    }

    #[test]
    fn test_api_key_value_redaction() {
        let layer = default_layer();
        let long_key = "abcdefghijklmnopqrstuvwxyz012345";
        assert_eq!(long_key.len(), 32);
        let result = layer.format_value("some_field", &long_key);
        assert_eq!(result, REDACTED_KEY);
    }

    #[test]
    fn test_uuid_not_redacted_as_api_key() {
        let layer = default_layer();
        let uuid_val = "550e8400-e29b-41d4-a716-446655440000";
        let result = layer.format_value("id_field", &uuid_val);
        assert_eq!(result, format!("{uuid_val:?}"));
    }

    #[test]
    fn test_email_value_redaction() {
        let layer = default_layer();
        let result = layer.format_value("contact", &"user@example.com");
        assert_eq!(result, "user@[REDACTED]");
    }

    #[test]
    fn test_phone_value_redaction() {
        let layer = default_layer();
        let result = layer.format_value("phone_field", &"+14155552671");
        assert_eq!(result, REDACTED_PHONE);
    }

    #[test]
    fn test_allowed_fields_not_redacted() {
        let mut config = PiiRedactionConfig::default();
        config.allowed_fields = vec!["token".to_owned()];
        let layer = PiiRedactionLayer::new(config);
        let result = layer.format_value("token", &"my_secret_token");
        assert_eq!(result, "\"my_secret_token\"");
    }

    #[test]
    fn test_extra_sensitive_fields() {
        let mut config = PiiRedactionConfig::default();
        config.extra_sensitive_fields = vec!["custom_secret".to_owned()];
        let layer = PiiRedactionLayer::new(config);
        assert!(layer.is_sensitive_field("custom_secret"));
        let result = layer.format_value("custom_secret", &"sensitive_data");
        assert_eq!(result, DEFAULT_REPLACEMENT);
    }

    #[test]
    fn test_non_matching_field_passes_through() {
        let layer = default_layer();
        let result = layer.format_value("normal_field", &"safe_value");
        assert_eq!(result, "\"safe_value\"");
    }

    #[test]
    fn test_custom_replacement_string() {
        let mut config = PiiRedactionConfig::default();
        config.replacement = "***".to_owned();
        let layer = PiiRedactionLayer::new(config);
        let result = layer.format_value("password", &"hunter2");
        assert_eq!(result, "***");
    }

    #[test]
    fn test_case_insensitive_field_matching() {
        let layer = default_layer();
        assert!(layer.is_sensitive_field("TOKEN"));
        assert!(layer.is_sensitive_field("Password"));
        assert!(layer.is_sensitive_field("API_KEY"));
    }

    #[test]
    fn test_username_field_redaction() {
        let layer = default_layer();
        let result = layer.format_value("username", &"john_doe");
        assert_eq!(result, REDACTED_ID);
    }
}
