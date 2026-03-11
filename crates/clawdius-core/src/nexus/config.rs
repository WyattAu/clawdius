//! Configuration management for Nexus FSM
//!
//! Supports loading from TOML files, environment variable overrides, and validation.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::{NexusError, Result};

static GLOBAL_CONFIG: OnceLock<NexusConfig> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NexusConfig {
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub recovery: RecoveryConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub events: EventsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    pub path: PathBuf,
    #[serde(default = "default_pool_size")]
    pub pool_size: usize,
    #[serde(default = "default_busy_timeout_ms")]
    pub busy_timeout_ms: u64,
    #[serde(default)]
    pub enable_wal: bool,
    #[serde(default)]
    pub enable_foreign_keys: bool,
}

fn default_db_path() -> PathBuf {
    PathBuf::from(".clawdius/nexus.db")
}

fn default_pool_size() -> usize {
    4
}

fn default_busy_timeout_ms() -> u64 {
    5000
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: default_db_path(),
            pool_size: default_pool_size(),
            busy_timeout_ms: default_busy_timeout_ms(),
            enable_wal: true,
            enable_foreign_keys: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    #[serde(default = "default_cache_size")]
    pub max_entries: usize,
    #[serde(default = "default_cache_ttl_secs")]
    pub ttl_secs: u64,
    #[serde(default)]
    pub enabled: bool,
}

fn default_cache_size() -> usize {
    1000
}

fn default_cache_ttl_secs() -> u64 {
    3600
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: default_cache_size(),
            ttl_secs: default_cache_ttl_secs(),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_metrics_prefix")]
    pub prefix: String,
    #[serde(default = "default_metrics_retention_hours")]
    pub retention_hours: u64,
}

fn default_metrics_prefix() -> String {
    "clawdius_nexus".to_string()
}

fn default_metrics_retention_hours() -> u64 {
    168
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            prefix: default_metrics_prefix(),
            retention_hours: default_metrics_retention_hours(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryConfig {
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,
    #[serde(default = "default_initial_delay_ms")]
    pub initial_delay_ms: u64,
    #[serde(default = "default_max_delay_ms")]
    pub max_delay_ms: u64,
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,
    #[serde(default = "default_circuit_breaker_threshold")]
    pub circuit_breaker_threshold: usize,
    #[serde(default = "default_circuit_breaker_reset_ms")]
    pub circuit_breaker_reset_ms: u64,
}

fn default_max_retries() -> usize {
    3
}

fn default_initial_delay_ms() -> u64 {
    100
}

fn default_max_delay_ms() -> u64 {
    5000
}

fn default_backoff_multiplier() -> f64 {
    2.0
}

fn default_circuit_breaker_threshold() -> usize {
    5
}

fn default_circuit_breaker_reset_ms() -> u64 {
    30000
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            initial_delay_ms: default_initial_delay_ms(),
            max_delay_ms: default_max_delay_ms(),
            backoff_multiplier: default_backoff_multiplier(),
            circuit_breaker_threshold: default_circuit_breaker_threshold(),
            circuit_breaker_reset_ms: default_circuit_breaker_reset_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default)]
    pub structured: bool,
    #[serde(default)]
    pub correlation_ids: bool,
    #[serde(default)]
    pub module_levels: std::collections::HashMap<String, String>,
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            structured: true,
            correlation_ids: true,
            module_levels: std::collections::HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventsConfig {
    #[serde(default = "default_max_history")]
    pub max_history: usize,
    #[serde(default)]
    pub persist_audit: bool,
    #[serde(default = "default_audit_table")]
    pub audit_table: String,
}

fn default_max_history() -> usize {
    1000
}

fn default_audit_table() -> String {
    "audit_log".to_string()
}

impl Default for EventsConfig {
    fn default() -> Self {
        Self {
            max_history: default_max_history(),
            persist_audit: false,
            audit_table: default_audit_table(),
        }
    }
}

impl NexusConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(NexusError::IoError)?;

        let mut config: NexusConfig = toml::from_str(&content)
            .map_err(|e| NexusError::ConfigError(format!("Failed to parse config: {e}")))?;

        config.apply_env_overrides();
        config.validate()?;

        Ok(config)
    }

    #[must_use]
    pub fn load_or_default(path: &Path) -> Self {
        Self::load(path).unwrap_or_default()
    }

    pub fn global() -> &'static NexusConfig {
        GLOBAL_CONFIG.get_or_init(NexusConfig::default)
    }

    pub fn set_global(config: NexusConfig) {
        let _ = GLOBAL_CONFIG.set(config);
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(val) = std::env::var("CLAWDIUS_DB_PATH") {
            self.database.path = PathBuf::from(val);
        }
        if let Ok(val) = std::env::var("CLAWDIUS_DB_POOL_SIZE") {
            if let Ok(size) = val.parse() {
                self.database.pool_size = size;
            }
        }
        if let Ok(val) = std::env::var("CLAWDIUS_CACHE_SIZE") {
            if let Ok(size) = val.parse() {
                self.cache.max_entries = size;
            }
        }
        if let Ok(val) = std::env::var("CLAWDIUS_CACHE_ENABLED") {
            self.cache.enabled = val.eq_ignore_ascii_case("true");
        }
        if let Ok(val) = std::env::var("CLAWDIUS_METRICS_ENABLED") {
            self.metrics.enabled = val.eq_ignore_ascii_case("true");
        }
        if let Ok(val) = std::env::var("CLAWDIUS_LOG_LEVEL") {
            self.logging.level = val;
        }
        if let Ok(val) = std::env::var("CLAWDIUS_MAX_RETRIES") {
            if let Ok(retries) = val.parse() {
                self.recovery.max_retries = retries;
            }
        }
    }

    fn validate(&self) -> Result<()> {
        if self.database.pool_size == 0 {
            return Err(NexusError::LockError(
                "database.pool_size must be at least 1".to_string(),
            ));
        }
        if self.cache.max_entries == 0 {
            return Err(NexusError::LockError(
                "cache.max_entries must be at least 1".to_string(),
            ));
        }
        if self.recovery.max_retries > 10 {
            return Err(NexusError::LockError(
                "recovery.max_retries cannot exceed 10".to_string(),
            ));
        }
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.logging.level.to_lowercase().as_str()) {
            return Err(NexusError::LockError(format!(
                "Invalid log level: {}. Valid levels: {:?}",
                self.logging.level, valid_levels
            )));
        }
        Ok(())
    }

    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self)
            .map_err(|e| NexusError::ConfigError(format!("Failed to serialize config: {e}")))
    }
}

#[derive(Debug, Clone)]
pub struct ConfigBuilder {
    config: NexusConfig,
}

impl ConfigBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: NexusConfig::default(),
        }
    }

    pub fn database_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.database.path = path.into();
        self
    }

    #[must_use]
    pub fn pool_size(mut self, size: usize) -> Self {
        self.config.database.pool_size = size;
        self
    }

    #[must_use]
    pub fn cache_size(mut self, size: usize) -> Self {
        self.config.cache.max_entries = size;
        self
    }

    #[must_use]
    pub fn cache_enabled(mut self, enabled: bool) -> Self {
        self.config.cache.enabled = enabled;
        self
    }

    #[must_use]
    pub fn metrics_enabled(mut self, enabled: bool) -> Self {
        self.config.metrics.enabled = enabled;
        self
    }

    pub fn log_level(mut self, level: impl Into<String>) -> Self {
        self.config.logging.level = level.into();
        self
    }

    #[must_use]
    pub fn max_retries(mut self, retries: usize) -> Self {
        self.config.recovery.max_retries = retries;
        self
    }

    pub fn build(self) -> Result<NexusConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = NexusConfig::default();
        assert!(config.database.pool_size >= 1);
        assert!(config.cache.max_entries >= 1);
        assert!(config.metrics.enabled);
    }

    #[test]
    fn test_config_builder() {
        let config = ConfigBuilder::new()
            .pool_size(8)
            .cache_size(500)
            .log_level("debug")
            .build()
            .unwrap();

        assert_eq!(config.database.pool_size, 8);
        assert_eq!(config.cache.max_entries, 500);
        assert_eq!(config.logging.level, "debug");
    }

    #[test]
    fn test_config_validation() {
        let result = ConfigBuilder::new().pool_size(0).build();
        assert!(result.is_err());
    }

    #[test]
    fn test_config_serialization() {
        let config = NexusConfig::default();
        let toml_str = config.to_toml().unwrap();
        let parsed: NexusConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config.database.pool_size, parsed.database.pool_size);
    }

    #[test]
    fn test_invalid_log_level() {
        let mut config = NexusConfig::default();
        config.logging.level = "invalid".to_string();
        assert!(config.validate().is_err());
    }
}
