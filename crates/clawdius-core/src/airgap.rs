//! Air-gapped mode for offline/self-hosted deployments.
//!
//! When air-gapped mode is enabled:
//! - All telemetry/crash reporting is disabled
//! - No external HTTP calls are made (except user-configured LLM providers)
//! - LLM provider URLs are validated against an allowlist
//! - No auto-update checks
//! - Usage data stays local
//!
//! This module provides the configuration and enforcement logic.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

/// Air-gapped mode configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirGapConfig {
    /// Whether air-gapped mode is enabled.
    pub enabled: bool,
    /// Allowed outbound hostnames (empty = allow all when not air-gapped).
    pub allowed_hosts: HashSet<String>,
    /// Whether to block all telemetry.
    pub block_telemetry: bool,
    /// Whether to block crash reporting.
    pub block_crash_reports: bool,
    /// Whether to block auto-update checks.
    pub block_auto_updates: bool,
    /// Whether to enforce local-only storage (no cloud DB).
    pub local_storage_only: bool,
    /// Custom message shown when air-gapped mode blocks an action.
    pub blocked_message: String,
}

impl Default for AirGapConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allowed_hosts: HashSet::new(),
            block_telemetry: true,
            block_crash_reports: true,
            block_auto_updates: true,
            local_storage_only: false,
            blocked_message: "Operation blocked: air-gapped mode is enabled".to_string(),
        }
    }
}

impl AirGapConfig {
    /// Create a strict air-gapped configuration.
    #[must_use]
    pub fn strict() -> Self {
        let mut allowed = HashSet::new();
        // Allow common LLM provider hosts
        allowed.insert("api.openai.com".to_string());
        allowed.insert("api.anthropic.com".to_string());
        allowed.insert("openrouter.ai".to_string());
        allowed.insert("api.zai.chat".to_string());
        allowed.insert("localhost".to_string());
        allowed.insert("127.0.0.1".to_string());

        Self {
            enabled: true,
            allowed_hosts: allowed,
            block_telemetry: true,
            block_crash_reports: true,
            block_auto_updates: true,
            local_storage_only: true,
            blocked_message: "Operation blocked: air-gapped mode is enabled. Contact your administrator.".to_string(),
        }
    }

    /// Check if an outbound HTTP request to a host is allowed.
    #[must_use]
    pub fn is_host_allowed(&self, host: &str) -> bool {
        if !self.enabled {
            return true;
        }
        // When enabled, only allowlisted hosts are permitted.
        // Empty allowlist = block everything.
        self.allowed_hosts.contains(host)
    }

    /// Check if telemetry is allowed.
    #[must_use]
    pub fn is_telemetry_allowed(&self) -> bool {
        !self.enabled || !self.block_telemetry
    }

    /// Check if crash reports are allowed.
    #[must_use]
    pub fn is_crash_reporting_allowed(&self) -> bool {
        !self.enabled || !self.block_crash_reports
    }

    /// Check if auto-updates are allowed.
    #[must_use]
    pub fn is_auto_update_allowed(&self) -> bool {
        !self.enabled || !self.block_auto_updates
    }
}

/// Air-gapped mode enforcer.
///
/// Provides runtime checks for operations that should be blocked
/// in air-gapped deployments.
pub struct AirGapEnforcer {
    config: Arc<parking_lot::RwLock<AirGapConfig>>,
}

impl AirGapEnforcer {
    /// Create a new enforcer with the given config.
    #[must_use]
    pub fn new(config: AirGapConfig) -> Self {
        Self {
            config: Arc::new(parking_lot::RwLock::new(config)),
        }
    }

    /// Create a disabled enforcer (everything allowed).
    #[must_use]
    pub fn disabled() -> Self {
        Self::new(AirGapConfig::default())
    }

    /// Check if air-gapped mode is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.read().enabled
    }

    /// Validate that an outbound HTTP request is allowed.
    ///
    /// Returns Ok(()) if allowed, Err with the blocked message otherwise.
    pub fn check_outbound_request(&self, host: &str) -> Result<(), AirGapError> {
        let config = self.config.read();
        if config.is_host_allowed(host) {
            Ok(())
        } else {
            Err(AirGapError::BlockedOutbound {
                host: host.to_string(),
                message: config.blocked_message.clone(),
            })
        }
    }

    /// Validate that telemetry can be sent.
    pub fn check_telemetry(&self) -> Result<(), AirGapError> {
        let config = self.config.read();
        if config.is_telemetry_allowed() {
            Ok(())
        } else {
            Err(AirGapError::BlockedTelemetry {
                message: config.blocked_message.clone(),
            })
        }
    }

    /// Validate that a crash report can be sent.
    pub fn check_crash_report(&self) -> Result<(), AirGapError> {
        let config = self.config.read();
        if config.is_crash_reporting_allowed() {
            Ok(())
        } else {
            Err(AirGapError::BlockedCrashReport {
                message: config.blocked_message.clone(),
            })
        }
    }

    /// Get the current config.
    #[must_use]
    pub fn config(&self) -> AirGapConfig {
        self.config.read().clone()
    }

    /// Update the configuration.
    pub fn update_config(&self, new_config: AirGapConfig) {
        *self.config.write() = new_config;
    }

    /// Add an allowed host.
    pub fn add_allowed_host(&self, host: impl Into<String>) {
        self.config.write().allowed_hosts.insert(host.into());
    }

    /// Remove an allowed host.
    pub fn remove_allowed_host(&self, host: &str) {
        self.config.write().allowed_hosts.remove(host);
    }

    /// Enable air-gapped mode.
    pub fn enable(&self) {
        self.config.write().enabled = true;
    }

    /// Disable air-gapped mode.
    pub fn disable(&self) {
        self.config.write().enabled = false;
    }
}

impl Default for AirGapEnforcer {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Air-gapped mode errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum AirGapError {
    #[error("outbound request blocked to {host}: {message}")]
    BlockedOutbound { host: String, message: String },
    #[error("telemetry blocked: {message}")]
    BlockedTelemetry { message: String },
    #[error("crash report blocked: {message}")]
    BlockedCrashReport { message: String },
}

// ─────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_not_air_gapped() {
        let config = AirGapConfig::default();
        assert!(!config.enabled);
        assert!(config.is_host_allowed("any-host.com"));
        // When not enabled, telemetry is allowed regardless of block_telemetry flag
        assert!(config.is_telemetry_allowed());
    }

    #[test]
    fn test_strict_config() {
        let config = AirGapConfig::strict();
        assert!(config.enabled);
        assert!(config.is_host_allowed("api.openai.com"));
        assert!(config.is_host_allowed("api.anthropic.com"));
        assert!(!config.is_host_allowed("telemetry.example.com"));
        assert!(!config.is_telemetry_allowed());
        assert!(!config.is_crash_reporting_allowed());
        assert!(config.local_storage_only);
    }

    #[test]
    fn test_enforcer_disabled() {
        let enforcer = AirGapEnforcer::disabled();
        assert!(!enforcer.is_enabled());
        assert!(enforcer.check_outbound_request("anything.com").is_ok());
        // When not air-gapped, telemetry is allowed
        assert!(enforcer.check_telemetry().is_ok());
    }

    #[test]
    fn test_enforcer_strict() {
        let enforcer = AirGapEnforcer::new(AirGapConfig::strict());
        assert!(enforcer.is_enabled());
        assert!(enforcer.check_outbound_request("api.openai.com").is_ok());
        assert!(enforcer.check_outbound_request("evil.com").is_err());
        assert!(enforcer.check_telemetry().is_err());
        assert!(enforcer.check_crash_report().is_err());
    }

    #[test]
    fn test_enforcer_enable_disable() {
        let enforcer = AirGapEnforcer::disabled();
        assert!(!enforcer.is_enabled());
        enforcer.enable();
        assert!(enforcer.is_enabled());
        enforcer.disable();
        assert!(!enforcer.is_enabled());
    }

    #[test]
    fn test_add_remove_host() {
        let enforcer = AirGapEnforcer::new(AirGapConfig::strict());
        assert!(enforcer.check_outbound_request("custom-llm.local").is_err());

        enforcer.add_allowed_host("custom-llm.local");
        assert!(enforcer.check_outbound_request("custom-llm.local").is_ok());

        enforcer.remove_allowed_host("custom-llm.local");
        assert!(enforcer.check_outbound_request("custom-llm.local").is_err());
    }

    #[test]
    fn test_update_config() {
        let enforcer = AirGapEnforcer::disabled();
        let mut new_config = AirGapConfig::default();
        new_config.enabled = true;
        new_config.blocked_message = "Custom blocked msg".to_string();
        new_config.block_telemetry = true;
        enforcer.update_config(new_config);

        assert!(enforcer.is_enabled());
        let config = enforcer.config();
        assert_eq!(config.blocked_message, "Custom blocked msg");
    }

    #[test]
    fn test_blocked_error_messages() {
        let enforcer = AirGapEnforcer::new(AirGapConfig::strict());
        let err = enforcer.check_outbound_request("evil.com").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("evil.com"));
        assert!(msg.contains("air-gapped"));
    }

    #[test]
    fn test_config_serialization() {
        let config = AirGapConfig::strict();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AirGapConfig = serde_json::from_str(&json).unwrap();
        assert!(deserialized.enabled);
        assert_eq!(deserialized.blocked_message, config.blocked_message);
    }

    #[test]
    fn test_empty_allowlist_when_enabled_blocks_all() {
        // When enabled with empty allowlist, everything is blocked
        let enforcer = AirGapEnforcer::new(AirGapConfig {
            enabled: true,
            allowed_hosts: HashSet::new(),
            block_telemetry: true,
            block_crash_reports: true,
            block_auto_updates: true,
            local_storage_only: true,
            blocked_message: "blocked".to_string(),
        });
        assert!(enforcer.check_outbound_request("anything.com").is_err());
        assert!(enforcer.check_outbound_request("api.openai.com").is_err());
    }

    #[test]
    fn test_localhost_allowed_in_strict() {
        let config = AirGapConfig::strict();
        assert!(config.is_host_allowed("localhost"));
        assert!(config.is_host_allowed("127.0.0.1"));
    }
}
