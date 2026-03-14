//! Configuration management for Clawdius.
//!
//! This module provides comprehensive configuration management with support for
//! TOML files, environment variables, and secure API key storage.
//!
//! # Features
//!
//! - **TOML-based configuration**: Human-readable configuration files
//! - **Environment variable support**: Override settings via environment
//! - **Secure key storage**: System keyring integration for API keys
//! - **Sensible defaults**: Pre-configured defaults for quick start
//! - **Validation**: Configuration validation with helpful error messages
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use clawdius_core::Config;
//! use std::path::Path;
//!
//! # fn main() -> clawdius_core::Result<()> {
//! // Load configuration (tries multiple locations)
//! let config = Config::load_or_default();
//!
//! // Or load from specific path
//! let config = Config::load(Path::new("clawdius.toml"))?;
//!
//! // Access configuration
//! println!("Project: {}", config.project.name);
//! println!("Rigor level: {}", config.project.rigor_level);
//!
//! // Save configuration
//! config.save(Path::new(".clawdius/config.toml"))?;
//! # Ok(())
//! # }
//! ```
//!
//! # Configuration File Structure
//!
//! Create a `clawdius.toml` or `.clawdius/config.toml` file:
//!
//! ```toml
//! [project]
//! name = "my-project"
//! rigor_level = "high"
//! lifecycle_phase = "implementation"
//!
//! [storage]
//! database_path = ".clawdius/graph/index.db"
//! vector_path = ".clawdius/graph/vectors.lance"
//! sessions_path = ".clawdius/sessions.db"
//!
//! [llm]
//! default_provider = "anthropic"
//! max_tokens = 4096
//!
//! [llm.anthropic]
//! model = "claude-3-5-sonnet-20241022"
//! api_key_env = "ANTHROPIC_API_KEY"
//!
//! [llm.openai]
//! model = "gpt-4o"
//! api_key_env = "OPENAI_API_KEY"
//!
//! [llm.ollama]
//! model = "llama3.2"
//! base_url = "http://localhost:11434"
//!
//! [llm.retry]
//! max_retries = 3
//! initial_delay_ms = 1000
//! max_delay_ms = 30000
//! exponential_base = 2.0
//! retry_on = ["rate_limit", "timeout", "server_error", "network_error"]
//!
//! [session]
//! compact_threshold = 0.85
//! keep_recent = 4
//! min_messages = 10
//! auto_save = true
//!
//! [output]
//! show_progress = true
//!
//! [output.format]
//! format = "text"
//!
//! [shell_sandbox]
//! timeout_secs = 120
//! max_output_bytes = 1048576
//! restrict_to_cwd = true
//! blocked_commands = [
//!     "rm -rf /",
//!     "mkfs",
//!     "dd if=/dev/zero",
//! ]
//! ```
//!
//! # LLM Configuration
//!
//! Configure multiple LLM providers:
//!
//! ```rust,no_run
//! use clawdius_core::config::{Config, LlmConfig, ProviderConfig, RetryConfig};
//! use clawdius_core::llm::{LlmConfig as LlmRuntimeConfig, create_provider};
//!
//! # fn main() -> clawdius_core::Result<()> {
//! let config = Config::load_or_default();
//!
//! // Access provider-specific configuration
//! if let Some(anthropic_config) = &config.llm.anthropic {
//!     println!("Anthropic model: {:?}", anthropic_config.model);
//! }
//!
//! // Create runtime LLM config from file config
//! let llm_config = LlmRuntimeConfig::from_config(&config.llm, "anthropic")?;
//! let provider = create_provider(&llm_config)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Retry Configuration
//!
//! Configure retry behavior for LLM calls:
//!
//! ```rust
//! use clawdius_core::config::{RetryConfig, RetryCondition};
//!
//! let retry_config = RetryConfig {
//!     max_retries: 5,              // Maximum retry attempts
//!     initial_delay_ms: 1000,      // Initial delay: 1 second
//!     max_delay_ms: 60000,         // Maximum delay: 60 seconds
//!     exponential_base: 2.0,       // Double delay each retry
//!     retry_on: vec![
//!         RetryCondition::RateLimit,    // Retry on 429 errors
//!         RetryCondition::Timeout,      // Retry on timeouts
//!         RetryCondition::ServerError,  // Retry on 5xx errors
//!         RetryCondition::NetworkError, // Retry on network failures
//!     ],
//! };
//! ```
//!
//! # Session Configuration
//!
//! Configure session behavior:
//!
//! ```rust
//! use clawdius_core::config::SessionConfig;
//!
//! let session_config = SessionConfig {
//!     compact_threshold: 0.85,  // Compact at 85% of context limit
//!     keep_recent: 6,           // Keep last 6 messages when compacting
//!     min_messages: 10,         // Minimum messages before compaction
//!     auto_save: true,          // Auto-save sessions
//! };
//! ```
//!
//! # Shell Sandbox Configuration
//!
//! Configure secure command execution:
//!
//! ```rust
//! use clawdius_core::config::ShellSandboxConfig;
//!
//! let sandbox_config = ShellSandboxConfig {
//!     blocked_commands: vec![
//!         "rm -rf /".to_string(),
//!         "mkfs".to_string(),
//!     ],
//!     timeout_secs: 120,           // 2-minute timeout
//!     max_output_bytes: 1_048_576, // 1 MB max output
//!     restrict_to_cwd: true,       // Restrict to working directory
//! };
//! ```
//!
//! # Secure API Key Storage
//!
//! Store API keys securely in the system keyring (requires `keyring` feature):
//!
//! ```rust,ignore
//! use clawdius_core::config::KeyringStorage;
//!
//! let storage = KeyringStorage::global();
//!
//! // Store API key
//! storage.set_api_key("anthropic", "your-api-key")?;
//!
//! // Retrieve API key
//! let key = storage.get_api_key("anthropic")?;
//!
//! // Delete API key
//! storage.delete_api_key("anthropic")?;
//! ```
//!
//! # Configuration Precedence
//!
//! Configuration is loaded in the following order (later overrides earlier):
//!
//! 1. Default values
//! 2. `.clawdius/config.toml`
//! 3. `clawdius.toml` (in current directory)
//! 4. Environment variables (for API keys)
//! 5. System keyring (for API keys)
//!
//! # Error Handling
//!
//! Configuration operations return [`Error`] variants:
//!
//! - [`Error::Config`]: Configuration validation errors
//! - [`Error::Io`]: File I/O errors
//! - [`Error::TomlDe`]: TOML parsing errors
//! - [`Error::TomlSer`]: TOML serialization errors
//!
//! [`Error`]: crate::Error

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Project configuration
    pub project: ProjectConfig,
    /// Storage configuration
    pub storage: StorageConfig,
    /// LLM configuration
    #[serde(default)]
    pub llm: LlmConfig,
    /// Session configuration
    #[serde(default)]
    pub session: SessionConfig,
    /// Output configuration
    #[serde(default)]
    pub output: OutputConfig,
    /// Shell sandbox configuration
    #[serde(default)]
    pub shell_sandbox: ShellSandboxConfig,
    /// Telemetry configuration
    #[serde(default)]
    pub telemetry: crate::telemetry::TelemetryConfig,
}

/// Project-level configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Project name
    pub name: String,
    /// Rigor level (low, medium, high)
    #[serde(default = "default_rigor")]
    pub rigor_level: String,
    /// Current lifecycle phase
    #[serde(default = "default_phase")]
    pub lifecycle_phase: String,
}

fn default_rigor() -> String {
    "high".to_string()
}

fn default_phase() -> String {
    "context_discovery".to_string()
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Path to `SQLite` database
    #[serde(default = "default_database_path")]
    pub database_path: PathBuf,
    /// Path to vector store
    #[serde(default = "default_vector_path")]
    pub vector_path: PathBuf,
    /// Path to sessions database
    #[serde(default = "default_sessions_path")]
    pub sessions_path: PathBuf,
}

fn default_database_path() -> PathBuf {
    PathBuf::from(".clawdius/graph/index.db")
}

fn default_vector_path() -> PathBuf {
    PathBuf::from(".clawdius/graph/vectors.lance")
}

fn default_sessions_path() -> PathBuf {
    PathBuf::from(".clawdius/sessions.db")
}

/// LLM configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmConfig {
    /// Default provider
    #[serde(default)]
    pub default_provider: Option<String>,
    /// Anthropic provider settings
    #[serde(default)]
    pub anthropic: Option<ProviderConfig>,
    /// `OpenAI` provider settings
    #[serde(default)]
    pub openai: Option<ProviderConfig>,
    /// Ollama provider settings
    #[serde(default)]
    pub ollama: Option<OllamaConfig>,
    /// ZAI provider settings
    #[serde(default)]
    pub zai: Option<ProviderConfig>,
    /// Maximum tokens for responses (global default)
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    /// Retry configuration
    #[serde(default)]
    pub retry: Option<RetryConfig>,
}

/// Retry configuration for LLM calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retries
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Initial delay in milliseconds
    #[serde(default = "default_initial_delay_ms")]
    pub initial_delay_ms: u64,
    /// Maximum delay in milliseconds
    #[serde(default = "default_max_delay_ms")]
    pub max_delay_ms: u64,
    /// Exponential backoff base
    #[serde(default = "default_exponential_base")]
    pub exponential_base: f64,
    /// Conditions to retry on
    #[serde(default = "default_retry_on")]
    pub retry_on: Vec<RetryCondition>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            initial_delay_ms: default_initial_delay_ms(),
            max_delay_ms: default_max_delay_ms(),
            exponential_base: default_exponential_base(),
            retry_on: default_retry_on(),
        }
    }
}

/// Conditions under which to retry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RetryCondition {
    /// Rate limit error (429)
    RateLimit,
    /// Timeout error
    Timeout,
    /// Server error (500, 502, 503)
    ServerError,
    /// Network error
    NetworkError,
}

fn default_max_retries() -> u32 {
    3
}
fn default_initial_delay_ms() -> u64 {
    1000
}
fn default_max_delay_ms() -> u64 {
    30000
}
fn default_exponential_base() -> f64 {
    2.0
}
fn default_retry_on() -> Vec<RetryCondition> {
    vec![
        RetryCondition::RateLimit,
        RetryCondition::Timeout,
        RetryCondition::ServerError,
        RetryCondition::NetworkError,
    ]
}

/// Generic provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Model to use
    pub model: Option<String>,
    /// Environment variable name for API key
    pub api_key_env: Option<String>,
    /// Inline API key (not recommended, use `api_key_env` instead)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Base URL for custom endpoints
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

/// Ollama-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    /// Model to use
    pub model: Option<String>,
    /// Base URL for Ollama server
    #[serde(default = "default_ollama_url")]
    pub base_url: String,
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}

fn default_max_tokens() -> usize {
    4096
}

/// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Auto-compact when context reaches this percentage
    #[serde(default = "default_compact_threshold")]
    pub compact_threshold: f32,
    /// Number of recent messages to keep when compacting
    #[serde(default = "default_keep_recent")]
    pub keep_recent: usize,
    /// Minimum messages before compacting
    #[serde(default = "default_min_messages")]
    pub min_messages: usize,
    /// Auto-save sessions
    #[serde(default = "default_true")]
    pub auto_save: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            compact_threshold: default_compact_threshold(),
            keep_recent: default_keep_recent(),
            min_messages: default_min_messages(),
            auto_save: default_true(),
        }
    }
}

fn default_compact_threshold() -> f32 {
    0.85
}

fn default_keep_recent() -> usize {
    4
}

fn default_min_messages() -> usize {
    10
}

fn default_true() -> bool {
    true
}

/// Output configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutputConfig {
    /// Default output format
    #[serde(default)]
    pub format: OutputFormatConfig,
    /// Show progress indicators
    #[serde(default = "default_true")]
    pub show_progress: bool,
}

/// Output format configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormatConfig {
    #[default]
    Text,
    Json,
    StreamJson,
}

/// Shell sandbox configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellSandboxConfig {
    /// Blocked command patterns
    #[serde(default = "default_blocked_commands")]
    pub blocked_commands: Vec<String>,
    /// Default timeout in seconds
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// Maximum output size in bytes
    #[serde(default = "default_max_output_bytes")]
    pub max_output_bytes: usize,
    /// Restrict commands to working directory
    #[serde(default = "default_true")]
    pub restrict_to_cwd: bool,
}

fn default_blocked_commands() -> Vec<String> {
    vec![
        "rm -rf /".to_string(),
        "mkfs".to_string(),
        "dd if=/dev/zero".to_string(),
        "dd if=/dev/urandom".to_string(),
        ":(){ :|:& };:".to_string(),
        "chmod -R 777 /".to_string(),
        "chown -R".to_string(),
        "> /dev/sda".to_string(),
        "mv /* /dev/null".to_string(),
        "wget".to_string(),
        "curl -X POST".to_string(),
    ]
}

fn default_timeout_secs() -> u64 {
    120
}

fn default_max_output_bytes() -> usize {
    1_048_576
}

impl Default for ShellSandboxConfig {
    fn default() -> Self {
        Self {
            blocked_commands: default_blocked_commands(),
            timeout_secs: default_timeout_secs(),
            max_output_bytes: default_max_output_bytes(),
            restrict_to_cwd: true,
        }
    }
}

impl Config {
    /// Load configuration from file
    pub fn load(path: &std::path::Path) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Get the default config path
    #[must_use]
    pub fn default_path() -> PathBuf {
        PathBuf::from(".clawdius/config.toml")
    }

    /// Load configuration from default locations, or return default if not found
    #[must_use]
    pub fn load_or_default() -> Self {
        Self::load_default().unwrap_or_default()
    }

    /// Load configuration from default locations
    pub fn load_default() -> crate::Result<Self> {
        // Try local config first
        let local_path = PathBuf::from("clawdius.toml");
        if local_path.exists() {
            return Self::load(&local_path);
        }

        // Try .clawdius/config.toml
        let clawdius_path = Self::default_path();
        if clawdius_path.exists() {
            return Self::load(&clawdius_path);
        }

        // Return default config
        Ok(Self::default())
    }

    /// Save configuration to file
    pub fn save(&self, path: &std::path::Path) -> crate::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            project: ProjectConfig {
                name: "clawdius".to_string(),
                rigor_level: default_rigor(),
                lifecycle_phase: default_phase(),
            },
            storage: StorageConfig {
                database_path: default_database_path(),
                vector_path: default_vector_path(),
                sessions_path: default_sessions_path(),
            },
            llm: LlmConfig::default(),
            session: SessionConfig::default(),
            output: OutputConfig::default(),
            shell_sandbox: ShellSandboxConfig::default(),
            telemetry: crate::telemetry::TelemetryConfig::default(),
        }
    }
}

#[cfg(feature = "keyring")]
pub mod keyring_storage {
    use super::*;
    use std::sync::OnceLock;

    static KEYRING_SERVICE: &str = "clawdius";
    static KEYRING_STORAGE: OnceLock<KeyringStorage> = OnceLock::new();

    #[derive(Debug, Clone)]
    pub struct KeyringStorage {
        service: String,
    }

    impl KeyringStorage {
        pub fn new() -> Self {
            Self {
                service: KEYRING_SERVICE.to_string(),
            }
        }

        pub fn global() -> &'static KeyringStorage {
            KEYRING_STORAGE.get_or_init(|| Self::new())
        }

        pub fn get_api_key(&self, provider: &str) -> crate::Result<Option<String>> {
            let entry = keyring::Entry::new(&self.service, provider)
                .map_err(|e| crate::Error::Config(format!("Failed to access keyring: {}", e)))?;

            match entry.get_password() {
                Ok(key) => Ok(Some(key)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(crate::Error::Config(format!(
                    "Failed to retrieve API key: {}",
                    e
                ))),
            }
        }

        pub fn set_api_key(&self, provider: &str, key: &str) -> crate::Result<()> {
            let entry = keyring::Entry::new(&self.service, provider)
                .map_err(|e| crate::Error::Config(format!("Failed to access keyring: {}", e)))?;

            entry
                .set_password(key)
                .map_err(|e| crate::Error::Config(format!("Failed to store API key: {}", e)))?;

            Ok(())
        }

        pub fn delete_api_key(&self, provider: &str) -> crate::Result<()> {
            let entry = keyring::Entry::new(&self.service, provider)
                .map_err(|e| crate::Error::Config(format!("Failed to access keyring: {}", e)))?;

            match entry.delete_credential() {
                Ok(()) => Ok(()),
                Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(crate::Error::Config(format!(
                    "Failed to delete API key: {}",
                    e
                ))),
            }
        }
    }

    impl Default for KeyringStorage {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(feature = "keyring")]
pub use keyring_storage::KeyringStorage;
