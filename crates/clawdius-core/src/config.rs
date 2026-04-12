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
    /// Messaging/webhook server configuration
    #[serde(default)]
    pub messaging: MessagingConfig,
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

// ---------------------------------------------------------------------------
// Messaging / webhook gateway configuration
// ---------------------------------------------------------------------------

/// Top-level `[messaging]` section in `clawdius.toml`.
///
/// All fields default to sensible values so an empty `[messaging]` table is
/// valid and enables the webhook server with mock channels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagingConfig {
    /// Server bind address (default: `"0.0.0.0"`)
    #[serde(default = "default_msg_host")]
    pub host: String,
    /// Server bind port (default: `8080`)
    #[serde(default = "default_msg_port")]
    pub port: u16,
    /// CORS allowed origins. `["*"]` means permissive.
    #[serde(default)]
    pub cors_origins: Vec<String>,
    /// Per-platform rate limit (requests / minute).
    #[serde(default = "default_msg_rate_limit")]
    pub rate_limit_per_minute: u32,
    /// Maximum webhook request body size in bytes.
    #[serde(default = "default_msg_max_body")]
    pub max_request_size_bytes: usize,
    /// API keys accepted by all platforms.
    #[serde(default)]
    pub global_api_keys: Vec<String>,
    /// Per-platform API keys: `{ platform = ["key1", "key2"] }`.
    #[serde(default)]
    pub api_keys: std::collections::HashMap<String, Vec<String>>,
    /// Per-platform webhook credentials.
    #[serde(default)]
    pub platforms: std::collections::HashMap<String, WebhookPlatformConfig>,
    /// Audit logging configuration
    #[serde(default)]
    pub audit: AuditConfig,
    /// PII redaction configuration
    #[serde(default)]
    pub pii_redaction: PiiRedactionConfig,
    /// Retry queue configuration
    #[serde(default)]
    pub retry: RetryQueueConfig,
    /// State store backend selection
    #[serde(default)]
    pub state_store: StateStoreConfig,
    /// Multi-tenant configuration
    #[serde(default)]
    pub tenants: TenantSectionConfig,
    /// IP allowlist for webhook requests (CIDR notation).
    /// If empty, all source IPs are accepted.
    #[serde(default)]
    pub ip_allowlist: Vec<String>,
    /// API key rotation: seconds before a newly added key expires.
    /// Default: 0 (no expiry).
    #[serde(default)]
    pub key_default_expiry_secs: u64,
    /// API key rotation: grace period in seconds after expiry.
    /// Default: 0 (no grace period).
    #[serde(default)]
    pub key_grace_period_secs: u64,
    /// HMAC secret for JWT token signing. Empty string (default) disables
    /// JWT auth; API keys are still accepted as a fallback.
    ///
    /// **WARNING:** For production use, set this via the environment variable
    /// `CLAWDIUS_JWT_SECRET` rather than in the config file, to avoid
    /// leaking the secret in version control.
    #[serde(default)]
    pub jwt_secret: String,
}

fn default_msg_host() -> String {
    "0.0.0.0".to_string()
}
fn default_msg_port() -> u16 {
    8080
}
fn default_msg_rate_limit() -> u32 {
    60
}
fn default_msg_max_body() -> usize {
    1_000_000
}

/// Platform-specific webhook credentials.
///
/// Only the fields relevant to a given platform are read; extras are ignored
/// by serde (using `#[serde(default)]`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebhookPlatformConfig {
    // Telegram
    #[serde(default)]
    pub secret_token: Option<String>,
    /// Telegram bot token for sending messages (from @BotFather)
    #[serde(default)]
    pub bot_token: Option<String>,
    // Discord
    #[serde(default)]
    pub public_key_pem: Option<String>,
    /// Discord bot token for sending messages
    #[serde(default)]
    pub discord_bot_token: Option<String>,
    // Matrix
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub homeserver_base_url: Option<String>,
    // Slack
    #[serde(default)]
    pub signing_secret: Option<String>,
    /// Slack bot token for sending messages (xoxb-...)
    #[serde(default)]
    pub slack_bot_token: Option<String>,
    // Rocket.Chat
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    /// Rocket.Chat server URL (e.g., "https://chat.example.com")
    #[serde(default)]
    pub server_url: Option<String>,
    // Signal
    #[serde(default)]
    pub verification_token: Option<String>,
    /// signal-cli-rest-api endpoint URL
    #[serde(default)]
    pub signal_api_url: Option<String>,
    /// Signal phone number
    #[serde(default)]
    pub signal_number: Option<String>,
    // WhatsApp
    #[serde(default)]
    pub verify_token: Option<String>,
    #[serde(default)]
    pub app_secret: Option<String>,
    /// WhatsApp phone number ID
    #[serde(default)]
    pub phone_number_id: Option<String>,
    /// WhatsApp access token
    #[serde(default)]
    pub whatsapp_access_token: Option<String>,
}

impl Default for MessagingConfig {
    fn default() -> Self {
        Self {
            host: default_msg_host(),
            port: default_msg_port(),
            cors_origins: Vec::new(),
            rate_limit_per_minute: default_msg_rate_limit(),
            max_request_size_bytes: default_msg_max_body(),
            global_api_keys: Vec::new(),
            api_keys: std::collections::HashMap::new(),
            platforms: std::collections::HashMap::new(),
            audit: AuditConfig::default(),
            pii_redaction: PiiRedactionConfig::default(),
            retry: RetryQueueConfig::default(),
            state_store: StateStoreConfig::default(),
            tenants: TenantSectionConfig::default(),
            ip_allowlist: Vec::new(),
            key_default_expiry_secs: 0,
            key_grace_period_secs: 0,
            jwt_secret: String::new(),
        }
    }
}

impl MessagingConfig {
    /// Whether any platform credentials or API keys are configured.
    pub fn is_configured(&self) -> bool {
        !self.global_api_keys.is_empty() || !self.api_keys.is_empty() || !self.platforms.is_empty()
    }

    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.port == 0 {
            errors.push("messaging.port must not be 0".to_string());
        }

        if self.max_request_size_bytes == 0 {
            errors.push("messaging.max_request_size_bytes must not be 0".to_string());
        } else if self.max_request_size_bytes > 100 * 1024 * 1024 {
            errors.push("messaging.max_request_size_bytes exceeds 100 MB".to_string());
        }

        if self.rate_limit_per_minute == 0 {
            errors.push("messaging.rate_limit_per_minute must be > 0".to_string());
        }

        if self.audit.retention_days == 0 {
            errors.push("messaging.audit.retention_days must be > 0".to_string());
        }

        if self.audit.flush_interval_secs == 0 {
            errors.push("messaging.audit.flush_interval_secs must be > 0".to_string());
        } else if self.audit.flush_interval_secs > 3600 {
            errors.push("messaging.audit.flush_interval_secs exceeds 1 hour".to_string());
        }

        if self.retry.max_retries == 0 {
            errors.push("messaging.retry.max_retries must be > 0".to_string());
        }
        if self.retry.initial_delay_ms == 0 {
            errors.push("messaging.retry.initial_delay_ms must be > 0".to_string());
        }
        if self.retry.max_delay_ms < self.retry.initial_delay_ms {
            errors.push("messaging.retry.max_delay_ms must be >= initial_delay_ms".to_string());
        }
        if self.retry.exponential_base < 1.0 {
            errors.push("messaging.retry.exponential_base must be >= 1.0".to_string());
        }
        if self.retry.jitter_factor < 0.0 || self.retry.jitter_factor > 1.0 {
            errors.push("messaging.retry.jitter_factor must be between 0.0 and 1.0".to_string());
        }

        if self.state_store.backend == "sqlite" && self.state_store.sqlite_path.trim().is_empty() {
            errors.push(
                "messaging.state_store.sqlite_path must not be empty when backend is \"sqlite\""
                    .to_string(),
            );
        }

        if !self.state_store.encryption_key.is_empty() {
            let hex = self.state_store.encryption_key.trim();
            if hex.len() != 64 {
                errors.push(
                    "messaging.state_store.encryption_key must be 64 hex characters (32 bytes) if provided"
                        .to_string(),
                );
            } else if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                errors.push(
                    "messaging.state_store.encryption_key must contain only hex characters"
                        .to_string(),
                );
            }
        }

        if self.tenants.enabled && self.tenants.db_path.trim().is_empty() {
            errors.push(
                "messaging.tenants.db_path must not be empty when tenants are enabled".to_string(),
            );
        }

        if self.pii_redaction.replacement.trim().is_empty() {
            errors.push("messaging.pii_redaction.replacement must not be empty".to_string());
        }

        errors
    }
}

/// Audit logging configuration.
///
/// Controls where audit events are persisted and how long they are retained.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    /// Storage backend: `"file"`, `"sqlite"`, `"elasticsearch"`, `"webhook"`, or `"memory"`.
    #[serde(default = "default_audit_backend")]
    pub backend: String,
    /// File-system directory for file-based audit logs (default: `"audit"`).
    #[serde(default = "default_audit_path")]
    pub path: String,
    /// SQLite database path for sqlite-based audit logs (default: `"audit.db"`).
    #[serde(default = "default_audit_sqlite_path")]
    pub sqlite_path: String,
    /// How often to flush buffered audit events, in seconds (default: `5`).
    #[serde(default = "default_audit_flush_secs")]
    pub flush_interval_secs: u64,
    /// Number of days to retain audit records (default: `90`).
    #[serde(default = "default_audit_retention")]
    pub retention_days: u32,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            backend: default_audit_backend(),
            path: default_audit_path(),
            sqlite_path: default_audit_sqlite_path(),
            flush_interval_secs: default_audit_flush_secs(),
            retention_days: default_audit_retention(),
        }
    }
}

fn default_audit_backend() -> String {
    "file".to_string()
}
fn default_audit_path() -> String {
    "audit".to_string()
}
fn default_audit_sqlite_path() -> String {
    "audit.db".to_string()
}
fn default_audit_flush_secs() -> u64 {
    5
}
fn default_audit_retention() -> u32 {
    90
}

/// PII redaction configuration for log output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiiRedactionConfig {
    /// Whether to redact known sensitive field names (default: `true`).
    #[serde(default = "default_true")]
    pub redact_field_names: bool,
    /// Whether to redact credential-like value patterns (default: `true`).
    #[serde(default = "default_true")]
    pub redact_value_patterns: bool,
    /// Additional field names to treat as sensitive.
    #[serde(default)]
    pub extra_sensitive_fields: Vec<String>,
    /// Field names that are explicitly allowed (never redacted).
    #[serde(default)]
    pub allowed_fields: Vec<String>,
    /// Replacement string used in place of redacted values.
    #[serde(default = "default_redaction_replacement")]
    pub replacement: String,
}

impl Default for PiiRedactionConfig {
    fn default() -> Self {
        Self {
            redact_field_names: true,
            redact_value_patterns: true,
            extra_sensitive_fields: Vec::new(),
            allowed_fields: Vec::new(),
            replacement: default_redaction_replacement(),
        }
    }
}

fn default_redaction_replacement() -> String {
    "[REDACTED]".to_string()
}

/// Retry queue configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryQueueConfig {
    /// Maximum number of retry attempts (default: 5)
    #[serde(default = "default_retry_queue_max")]
    pub max_retries: u32,
    /// Initial delay between retries in milliseconds (default: 1000)
    #[serde(default = "default_retry_queue_initial_delay")]
    pub initial_delay_ms: u64,
    /// Maximum delay between retries in milliseconds (default: 300000 = 5 min)
    #[serde(default = "default_retry_queue_max_delay")]
    pub max_delay_ms: u64,
    /// Exponential backoff base (default: 2.0)
    #[serde(default = "default_retry_queue_base")]
    pub exponential_base: f64,
    /// Jitter factor as fraction (default: 0.1 = ±10%)
    #[serde(default = "default_retry_queue_jitter")]
    pub jitter_factor: f64,
    /// Maximum number of tasks in the queue (default: 10000)
    #[serde(default = "default_retry_queue_max_size")]
    pub max_queue_size: usize,
    /// Whether to enable dead letter queue for exhausted tasks (default: true)
    #[serde(default = "default_true")]
    pub dead_letter_enabled: bool,
}

impl Default for RetryQueueConfig {
    fn default() -> Self {
        Self {
            max_retries: default_retry_queue_max(),
            initial_delay_ms: default_retry_queue_initial_delay(),
            max_delay_ms: default_retry_queue_max_delay(),
            exponential_base: default_retry_queue_base(),
            jitter_factor: default_retry_queue_jitter(),
            max_queue_size: default_retry_queue_max_size(),
            dead_letter_enabled: default_true(),
        }
    }
}

fn default_retry_queue_max() -> u32 {
    5
}
fn default_retry_queue_initial_delay() -> u64 {
    1000
}
fn default_retry_queue_max_delay() -> u64 {
    300_000
}
fn default_retry_queue_base() -> f64 {
    2.0
}
fn default_retry_queue_jitter() -> f64 {
    0.1
}
fn default_retry_queue_max_size() -> usize {
    10_000
}

/// State store backend configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateStoreConfig {
    /// Backend type: "memory" or "sqlite" (default: "sqlite")
    #[serde(default = "default_state_backend")]
    pub backend: String,
    /// SQLite database path (default: "messaging_state.db")
    #[serde(default = "default_state_path")]
    pub sqlite_path: String,
    /// Optional 32-byte hex-encoded AES-256 key for encryption at rest.
    /// When set, all values stored in the state store are encrypted with
    /// AES-256-GCM. If empty or missing, data is stored in plaintext.
    /// Generate with: `openssl rand -hex 32`
    #[serde(default)]
    pub encryption_key: String,
}

impl Default for StateStoreConfig {
    fn default() -> Self {
        Self {
            backend: default_state_backend(),
            sqlite_path: default_state_path(),
            encryption_key: String::new(),
        }
    }
}

fn default_state_backend() -> String {
    "sqlite".to_string()
}
fn default_state_path() -> String {
    "messaging_state.db".to_string()
}

/// Multi-tenant configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantSectionConfig {
    /// Whether multi-tenancy is enabled (default: false)
    #[serde(default)]
    pub enabled: bool,
    /// Default maximum sessions per user (default: 100)
    #[serde(default = "default_tenant_max_sessions")]
    pub default_max_sessions_per_user: u32,
    /// SQLite database path for tenant data (default: "tenants.db")
    #[serde(default = "default_tenant_path")]
    pub db_path: String,
}

impl Default for TenantSectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_max_sessions_per_user: default_tenant_max_sessions(),
            db_path: default_tenant_path(),
        }
    }
}

fn default_tenant_max_sessions() -> u32 {
    100
}
fn default_tenant_path() -> String {
    "tenants.db".to_string()
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

    pub fn validate(&self) -> Vec<String> {
        let mut errors = self.messaging.validate();

        if self.project.name.trim().is_empty() {
            errors.push("project.name must not be empty".to_string());
        }

        errors
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
            messaging: MessagingConfig::default(),
            telemetry: crate::telemetry::TelemetryConfig::default(),
        }
    }
}

#[cfg(feature = "keyring")]
pub mod keyring_storage {

    use std::sync::OnceLock;

    static KEYRING_SERVICE: &str = "clawdius";
    static KEYRING_STORAGE: OnceLock<KeyringStorage> = OnceLock::new();

    #[derive(Debug, Clone)]
    pub struct KeyringStorage {
        service: String,
    }

    impl KeyringStorage {
        #[must_use]
        pub fn new() -> Self {
            Self {
                service: KEYRING_SERVICE.to_string(),
            }
        }

        pub fn global() -> &'static KeyringStorage {
            KEYRING_STORAGE.get_or_init(Self::new)
        }

        pub fn get_api_key(&self, provider: &str) -> crate::Result<Option<String>> {
            let entry = keyring::Entry::new(&self.service, provider)
                .map_err(|e| crate::Error::Config(format!("Failed to access keyring: {e}")))?;

            match entry.get_password() {
                Ok(key) => Ok(Some(key)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(crate::Error::Config(format!(
                    "Failed to retrieve API key: {e}"
                ))),
            }
        }

        pub fn set_api_key(&self, provider: &str, key: &str) -> crate::Result<()> {
            let entry = keyring::Entry::new(&self.service, provider)
                .map_err(|e| crate::Error::Config(format!("Failed to access keyring: {e}")))?;

            entry
                .set_password(key)
                .map_err(|e| crate::Error::Config(format!("Failed to store API key: {e}")))?;

            Ok(())
        }

        pub fn delete_api_key(&self, provider: &str) -> crate::Result<()> {
            let entry = keyring::Entry::new(&self.service, provider)
                .map_err(|e| crate::Error::Config(format!("Failed to access keyring: {e}")))?;

            match entry.delete_credential() {
                Ok(()) => Ok(()),
                Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(crate::Error::Config(format!(
                    "Failed to delete API key: {e}"
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

#[cfg(test)]
mod tests {
    use super::*;

    fn default_msg_config() -> MessagingConfig {
        MessagingConfig::default()
    }

    #[test]
    fn default_config_is_valid() {
        let config = default_msg_config();
        let errors = config.validate();
        assert!(errors.is_empty(), "Default config has errors: {:?}", errors);
    }

    #[test]
    fn port_zero_is_invalid() {
        let mut config = default_msg_config();
        config.port = 0;
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.contains("port")));
    }

    #[test]
    fn max_request_size_zero_is_invalid() {
        let mut config = default_msg_config();
        config.max_request_size_bytes = 0;
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.contains("max_request_size_bytes")));
    }

    #[test]
    fn rate_limit_zero_is_invalid() {
        let mut config = default_msg_config();
        config.rate_limit_per_minute = 0;
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.contains("rate_limit")));
    }

    #[test]
    fn audit_retention_zero_is_invalid() {
        let mut config = default_msg_config();
        config.audit.retention_days = 0;
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.contains("retention")));
    }

    #[test]
    fn retry_max_delay_less_than_initial_is_invalid() {
        let mut config = default_msg_config();
        config.retry.initial_delay_ms = 5000;
        config.retry.max_delay_ms = 1000;
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.contains("max_delay_ms")));
    }

    #[test]
    fn state_store_empty_path_with_sqlite_is_invalid() {
        let mut config = default_msg_config();
        config.state_store.backend = "sqlite".to_string();
        config.state_store.sqlite_path = "".to_string();
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.contains("sqlite_path")));
    }

    #[test]
    fn encryption_key_wrong_length_is_invalid() {
        let mut config = default_msg_config();
        config.state_store.encryption_key = "deadbeef".to_string();
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.contains("encryption_key")));
    }

    #[test]
    fn encryption_key_invalid_hex_is_invalid() {
        let mut config = default_msg_config();
        config.state_store.encryption_key = "g".repeat(64);
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.contains("encryption_key")));
    }

    #[test]
    fn encryption_key_valid_64_hex_passes() {
        let mut config = default_msg_config();
        config.state_store.encryption_key = "a".repeat(64);
        let errors = config.validate();
        assert!(!errors.iter().any(|e| e.contains("encryption_key")));
    }

    #[test]
    fn tenant_enabled_empty_db_path_is_invalid() {
        let mut config = default_msg_config();
        config.tenants.enabled = true;
        config.tenants.db_path = "".to_string();
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.contains("tenants")));
    }

    #[test]
    fn root_config_validate_includes_messaging() {
        let mut config = Config::default();
        config.messaging.port = 0;
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.contains("port")));
    }

    #[test]
    fn empty_project_name_is_invalid() {
        let mut config = Config::default();
        config.project.name = "  ".to_string();
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.contains("project.name")));
    }

    #[test]
    fn valid_custom_config_passes() {
        let config = MessagingConfig {
            port: 8080,
            max_request_size_bytes: 1_000_000,
            rate_limit_per_minute: 60,
            audit: AuditConfig {
                retention_days: 30,
                flush_interval_secs: 5,
                ..AuditConfig::default()
            },
            state_store: StateStoreConfig {
                backend: "memory".to_string(),
                ..StateStoreConfig::default()
            },
            ..default_msg_config()
        };
        let errors = config.validate();
        assert!(errors.is_empty(), "Custom config has errors: {:?}", errors);
    }
}
