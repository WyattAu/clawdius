//! Configuration management for Clawdius
//!
//! Loads configuration from `clawdius.toml` and `.clawdius/settings.toml`.
//! Uses serde for TOML parsing with validation at startup.

use crate::error::{ClawdiusError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Project configuration
    pub project: ProjectConfig,
    /// Storage configuration
    #[serde(default)]
    pub storage: StorageConfig,
    /// LLM configuration
    #[serde(default)]
    pub llm: LlmConfig,
    /// Runtime configuration
    #[serde(default)]
    pub runtime: RuntimeConfig,
}

/// Project-level configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Project name
    pub name: String,
    /// Rigor level (low, medium, high)
    #[serde(default = "default_rigor_level")]
    pub rigor_level: RigorLevel,
    /// Current lifecycle phase
    #[serde(default)]
    pub lifecycle_phase: String,
}

/// Rigor level for the project
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RigorLevel {
    /// Low rigor - minimal checks
    Low,
    /// Medium rigor - standard checks
    Medium,
    /// High rigor - full SOP enforcement
    #[default]
    High,
}

fn default_rigor_level() -> RigorLevel {
    RigorLevel::High
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Path to the graph database
    #[serde(default = "default_database_path")]
    pub database_path: PathBuf,
    /// Path to vector storage
    #[serde(default = "default_vector_path")]
    pub vector_path: PathBuf,
}

fn default_database_path() -> PathBuf {
    PathBuf::from(".clawdius/graph/index.db")
}

fn default_vector_path() -> PathBuf {
    PathBuf::from(".clawdius/graph/vectors.lance")
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            database_path: default_database_path(),
            vector_path: default_vector_path(),
        }
    }
}

/// LLM provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Default LLM provider
    #[serde(default = "default_provider")]
    pub default_provider: String,
    /// Model to use
    #[serde(default = "default_model")]
    pub model: String,
    /// Enable MCP (Model Context Protocol)
    #[serde(default = "default_mcp_enabled")]
    pub mcp_enabled: bool,
}

fn default_provider() -> String {
    String::from("anthropic")
}

fn default_model() -> String {
    String::from("claude-3-5-sonnet-latest")
}

fn default_mcp_enabled() -> bool {
    true
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            default_provider: default_provider(),
            model: default_model(),
            mcp_enabled: default_mcp_enabled(),
        }
    }
}

/// Runtime configuration for the Host Kernel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Number of worker threads (0 = auto)
    #[serde(default)]
    pub worker_threads: usize,
    /// Enable debug mode
    #[serde(default)]
    pub debug: bool,
    /// Shutdown timeout in seconds
    #[serde(default = "default_shutdown_timeout")]
    pub shutdown_timeout_secs: u64,
    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_shutdown_timeout() -> u64 {
    30
}

fn default_log_level() -> String {
    String::from("info")
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            worker_threads: 0,
            debug: false,
            shutdown_timeout_secs: default_shutdown_timeout(),
            log_level: default_log_level(),
        }
    }
}

impl Config {
    /// Load configuration from the given directory
    ///
    /// Looks for `clawdius.toml` in the specified directory.
    /// Falls back to defaults if not found.
    ///
    /// # Errors
    /// Returns an error if the configuration file exists but cannot be parsed.
    pub fn load(project_root: &Path) -> Result<Self> {
        let config_path = project_root.join("clawdius.toml");

        if config_path.exists() {
            Self::load_from_file(&config_path)
        } else {
            tracing::warn!(
                path = %config_path.display(),
                "Configuration file not found, using defaults"
            );
            Ok(Self::default())
        }
    }

    /// Load configuration from a specific file
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or parsed.
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            ClawdiusError::Config(format!("Failed to read {}: {}", path.display(), e))
        })?;

        let config: Self = toml::from_str(&content).map_err(|e| {
            ClawdiusError::Config(format!("Failed to parse {}: {}", path.display(), e))
        })?;

        tracing::info!(path = %path.display(), "Configuration loaded");
        Ok(config)
    }

    /// Validate the configuration
    ///
    /// # Errors
    /// Returns an error if validation fails.
    pub fn validate(&self) -> Result<()> {
        if self.project.name.is_empty() {
            return Err(ClawdiusError::Config("Project name cannot be empty".into()));
        }

        if self.runtime.shutdown_timeout_secs == 0 {
            return Err(ClawdiusError::Config(
                "Shutdown timeout must be greater than 0".into(),
            ));
        }

        Ok(())
    }

    /// Get the project root directory
    #[must_use]
    pub fn project_root(&self) -> &Path {
        Path::new(".")
    }

    /// Get the database path, resolved relative to project root
    #[must_use]
    pub fn resolved_database_path(&self) -> PathBuf {
        self.storage.database_path.clone()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            project: ProjectConfig {
                name: String::from("clawdius"),
                rigor_level: RigorLevel::High,
                lifecycle_phase: String::from("context_discovery"),
            },
            storage: StorageConfig::default(),
            llm: LlmConfig::default(),
            runtime: RuntimeConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.project.name, "clawdius");
        assert_eq!(config.project.rigor_level, RigorLevel::High);
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        assert!(config.validate().is_ok());

        config.project.name = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_load_from_toml() {
        let toml = r#"
[project]
name = "test-project"
rigor_level = "low"

[storage]
database_path = "test.db"
"#;
        let config: Config = toml::from_str(toml).expect("Failed to parse");
        assert_eq!(config.project.name, "test-project");
        assert_eq!(config.project.rigor_level, RigorLevel::Low);
    }
}
