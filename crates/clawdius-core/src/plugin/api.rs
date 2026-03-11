//! Plugin API - Core traits and types for plugins
//!
//! This module defines the interface that plugins must implement.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::hooks::HookContext;

/// Unique identifier for a plugin
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginId(String);

impl PluginId {
    pub fn new(name: &str, version: &str) -> Self {
        Self(format!("{}@{}", name, version))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PluginId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Plugin capabilities flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginCapabilities {
    /// Can read files
    pub can_read_files: bool,
    /// Can write files
    pub can_write_files: bool,
    /// Can execute commands
    pub can_execute: bool,
    /// Can make network requests
    pub can_network: bool,
    /// Can access the LLM
    pub can_access_llm: bool,
    /// Can access session history
    pub can_access_history: bool,
    /// Can modify other plugins
    pub can_modify_plugins: bool,
}

impl Default for PluginCapabilities {
    fn default() -> Self {
        Self {
            can_read_files: true,
            can_write_files: false,
            can_execute: false,
            can_network: false,
            can_access_llm: false,
            can_access_history: false,
            can_modify_plugins: false,
        }
    }
}

impl PluginCapabilities {
    /// Unrestricted capabilities (use with caution)
    pub fn unrestricted() -> Self {
        Self {
            can_read_files: true,
            can_write_files: true,
            can_execute: true,
            can_network: true,
            can_access_llm: true,
            can_access_history: true,
            can_modify_plugins: true,
        }
    }

    /// Read-only capabilities
    pub fn read_only() -> Self {
        Self {
            can_read_files: true,
            can_write_files: false,
            can_execute: false,
            can_network: false,
            can_access_llm: false,
            can_access_history: false,
            can_modify_plugins: false,
        }
    }
}

/// Plugin state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginState {
    /// Plugin is loaded but not initialized
    Loaded,
    /// Plugin is initializing
    Initializing,
    /// Plugin is active and running
    Active,
    /// Plugin is paused
    Paused,
    /// Plugin encountered an error
    Error,
    /// Plugin is being unloaded
    Unloading,
}

/// Plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Unique plugin identifier
    pub id: PluginId,
    /// Human-readable name
    pub name: String,
    /// Plugin version (semver)
    pub version: String,
    /// Plugin description
    pub description: String,
    /// Author information
    pub author: PluginAuthor,
    /// Plugin homepage URL
    pub homepage: Option<String>,
    /// Repository URL
    pub repository: Option<String>,
    /// License identifier (SPDX)
    pub license: String,
    /// Plugin keywords/tags
    pub keywords: Vec<String>,
    /// Minimum Clawdius version required
    pub min_clawdius_version: String,
    /// Plugin capabilities
    pub capabilities: PluginCapabilities,
    /// Hooks this plugin subscribes to
    pub subscribed_hooks: Vec<String>,
    /// Custom configuration schema (JSON Schema)
    pub config_schema: Option<serde_json::Value>,
    /// Dependencies on other plugins
    pub dependencies: Vec<PluginDependency>,
}

/// Plugin author information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAuthor {
    pub name: String,
    pub email: Option<String>,
    pub url: Option<String>,
}

/// Plugin dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependency {
    /// Dependency plugin name
    pub name: String,
    /// Version requirement (semver range)
    pub version: String,
    /// Whether this is an optional dependency
    pub optional: bool,
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Plugin ID
    pub plugin_id: PluginId,
    /// Whether the plugin is enabled
    pub enabled: bool,
    /// Priority (higher = runs first)
    pub priority: i32,
    /// Custom configuration values
    pub settings: HashMap<String, serde_json::Value>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            plugin_id: PluginId::new("unknown", "0.0.0"),
            enabled: true,
            priority: 0,
            settings: HashMap::new(),
        }
    }
}

/// Result of a hook execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResult {
    /// Whether the hook execution was successful
    pub success: bool,
    /// Result data (if any)
    pub data: Option<serde_json::Value>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Whether to stop propagation to other plugins
    pub stop_propagation: bool,
}

impl HookResult {
    pub fn success() -> Self {
        Self {
            success: true,
            data: None,
            error: None,
            stop_propagation: false,
        }
    }

    pub fn success_with_data(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            stop_propagation: false,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
            stop_propagation: false,
        }
    }

    pub fn stop() -> Self {
        Self {
            success: true,
            data: None,
            error: None,
            stop_propagation: true,
        }
    }
}

/// Core plugin trait that all plugins must implement
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Get plugin metadata
    fn metadata(&self) -> &PluginMetadata;

    /// Get current plugin state
    fn state(&self) -> PluginState;

    /// Initialize the plugin
    async fn initialize(&mut self, config: PluginConfig) -> Result<()>;

    /// Shutdown the plugin
    async fn shutdown(&mut self) -> Result<()>;

    /// Handle a hook event
    async fn on_hook(&self, hook_name: &str, context: &HookContext) -> Result<HookResult>;

    /// Get plugin configuration
    fn config(&self) -> &PluginConfig;

    /// Update plugin configuration
    async fn update_config(&mut self, config: PluginConfig) -> Result<()>;

    /// Health check
    async fn health_check(&self) -> Result<bool> {
        Ok(self.state() == PluginState::Active)
    }
}

/// Plugin statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginStats {
    /// Number of times hooks were called
    pub hook_calls: u64,
    /// Number of successful hook executions
    pub successful_hooks: u64,
    /// Number of failed hook executions
    pub failed_hooks: u64,
    /// Total execution time in milliseconds
    pub total_execution_time_ms: u64,
    /// Memory usage in bytes
    pub memory_usage_bytes: u64,
    /// Last activity timestamp
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
}
