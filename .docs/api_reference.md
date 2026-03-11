# Clawdius API Reference

**Version:** 0.7.0  
**Last Updated:** 2026-03-06

---

## Table of Contents

1. [Core Types](#1-core-types)
2. [State Machine](#2-state-machine)
3. [Error Types](#3-error-types)
4. [Version Information](#4-version-information)
5. [LLM Integration](#5-llm-integration)
6. [Tool Execution](#6-tool-execution)
7. [Configuration](#7-configuration)
8. [Keyring Storage](#8-keyring-storage)
9. [Traits](#9-traits)

---

## 1. Core Types

### 1.1 Phase

The `Phase` enum represents all 24 phases in the Nexus R&D Lifecycle.

**Location:** `src/fsm.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    ContextDiscovery = -1,
    EnvironmentMaterialization = 0,
    RequirementsEngineering = 1,
    EpistemologicalDiscovery = 2,
    KnowledgeIntegration = 3,
    SupplyChainHardening = 4,
    ArchitecturalSpecification = 5,
    ConcurrencyAnalysis = 6,
    SecurityEngineering = 7,
    ResourceManagement = 8,
    PerformanceEngineering = 9,
    CrossPlatformCompatibility = 10,
    AdversarialLoop = 11,
    RegressionBaseline = 12,
    CiCdEngineering = 13,
    DocumentationVerification = 14,
    NarrativeDocumentation = 15,
    KnowledgeBaseUpdate = 16,
    ExecutionGraphGeneration = 17,
    SupplyChainMonitoring = 18,
    DeploymentOperations = 19,
    ProjectClosure = 20,
    ContinuousMonitoring = 21,
    KnowledgeTransfer = 22,
}
```

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `display_name` | `fn display_name(&self) -> &'static str` | Human-readable phase name |
| `next` | `fn next(&self) -> Option<Self>` | Get next phase in sequence |
| `is_terminal` | `fn is_terminal(&self) -> bool` | Check if phase is terminal |

#### Example

```rust
use clawdius::fsm::Phase;

let phase = Phase::ContextDiscovery;
assert_eq!(phase.display_name(), "Context Discovery");
assert_eq!(phase.next(), Some(Phase::EnvironmentMaterialization));
assert!(!phase.is_terminal());
```

---

### 1.2 TransitionResult

Result of a state machine tick operation.

**Location:** `src/fsm.rs`

```rust
#[derive(Debug)]
pub enum TransitionResult {
    Continue,
    Transition(Phase),
    Complete,
    Error(ClawdiusError),
}
```

#### Variants

| Variant | Description |
|---------|-------------|
| `Continue` | Remain in current phase |
| `Transition(Phase)` | Transition to new phase |
| `Complete` | All phases complete |
| `Error(ClawdiusError)` | Error occurred |

---

### 1.3 QualityGateStatus

Status of a quality gate evaluation.

**Location:** `src/fsm.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityGateStatus {
    Passed,
    Failed,
    Pending,
}
```

---

### 1.4 QualityGate

Quality gate definition for phase transitions.

**Location:** `src/fsm.rs`

```rust
#[derive(Debug)]
pub struct QualityGate {
    pub id: String,
    pub description: String,
    pub status: QualityGateStatus,
}
```

---

## 2. State Machine

### 2.1 StateMachine

The main state machine for the Nexus lifecycle.

**Location:** `src/fsm.rs`

```rust
#[derive(Debug)]
pub struct StateMachine {
    phase: Phase,
    quality_gates: Vec<QualityGate>,
    error_level: u8,
    ticks_in_phase: u64,
}
```

#### Constructors

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new() -> Result<Self>` | Create at Context Discovery |
| `at_phase` | `fn at_phase(phase: Phase) -> Result<Self>` | Create at specific phase |

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `current_phase` | `fn current_phase(&self) -> Phase` | Get current phase |
| `error_level` | `fn error_level(&self) -> u8` | Get error level (0-10) |
| `tick` | `fn tick(&mut self) -> TransitionResult` | Process one state transition |
| `pass_gate` | `fn pass_gate(&mut self, gate_id: &str) -> Result<()>` | Mark gate as passed |
| `fail_gate` | `fn fail_gate(&mut self, gate_id: &str, reason: &str)` | Mark gate as failed |

#### Example

```rust
use clawdius::fsm::{StateMachine, Phase, TransitionResult};

let mut sm = StateMachine::new()?;

assert_eq!(sm.current_phase(), Phase::ContextDiscovery);

match sm.tick() {
    TransitionResult::Continue => println!("Still in current phase"),
    TransitionResult::Transition(p) => println!("Moved to {:?}", p),
    TransitionResult::Complete => println!("All phases done"),
    TransitionResult::Error(e) => eprintln!("Error: {}", e),
}
```

---

## 3. Error Types

### 3.1 ClawdiusError

Top-level error type for Clawdius operations.

**Location:** `src/error.rs`

```rust
#[derive(Error, Debug)]
pub enum ClawdiusError {
    #[error("State machine error: {0}")]
    StateMachine(#[from] StateMachineError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[error("LLM error: {0}")]
    Llm(String),

    #[error("Sandbox error: {0}")]
    Sandbox(#[from] SandboxError),

    #[error("SOP violation: {0}")]
    SopViolation(String),

    #[error("Invalid phase transition: {from:?} -> {to:?}")]
    InvalidTransition {
        from: Phase,
        to: Phase,
    },
}
```

---

### 3.2 StateMachineError

State machine specific errors.

**Location:** `src/error.rs`

```rust
#[derive(Error, Debug)]
pub enum StateMachineError {
    #[error("Invalid transition from {from} to {to}")]
    InvalidTransition { from: String, to: String },

    #[error("Quality gate failed: {gate}")]
    QualityGateFailed { gate: String },

    #[error("Required artifact missing: {artifact}")]
    MissingArtifact { artifact: String },

    #[error("Phase prerequisites not met: {details}")]
    PrerequisitesNotMet { details: String },
}
```

---

### 3.3 SandboxError

Sandbox execution errors.

**Location:** `src/error.rs`

```rust
#[derive(Error, Debug)]
pub enum SandboxError {
    #[error("Failed to create sandbox: {reason}")]
    CreationFailed { reason: String },

    #[error("Sandbox execution failed: {exit_code}")]
    ExecutionFailed { exit_code: i32 },

    #[error("Capability violation: {capability}")]
    CapabilityViolation { capability: String },

    #[error("Sandbox execution timeout after {seconds}s")]
    Timeout { seconds: u64 },
}
```

---

### 3.4 HotPathError

Zero-allocation error codes for hot paths.

**Location:** `src/error.rs`

```rust
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotPathError {
    None = 0,
    BufferOverflow = 1,
    InvalidInput = 2,
    Timeout = 3,
    ResourceExhausted = 4,
    ParseError = 5,
}
```

#### Design Note

Per Rust SOP Part 1.2, hot-path errors use `#[repr(u8)]` C-like enums to ensure zero heap allocation and fit entirely in CPU registers.

---

## 4. Version Information

### 4.1 VERSION Constant

**Location:** `src/version.rs`

```rust
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
```

---

### 4.2 VersionInfo

Parsed version components.

**Location:** `src/version.rs`

```rust
pub struct VersionInfo {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}
```

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `parse` | `fn parse(version: &str) -> Self` | Parse version string |
| `current` | `fn current() -> Self` | Get current version |

#### Example

```rust
use clawdius::version::VersionInfo;

let v = VersionInfo::current();
println!("Version: {}.{}.{}", v.major, v.minor, v.patch);
```

---

## 5. LLM Integration

### 5.1 LlmConfig

Configuration for LLM provider connections.

**Location:** `src/llm.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub max_tokens: usize,
}
```

#### Constructors

| Method | Signature | Description |
|--------|-----------|-------------|
| `from_env` | `fn from_env(provider: &str) -> Result<Self>` | Create from environment variables |
| `from_config` | `fn from_config(config: &LlmConfig, provider: &str) -> Result<Self>` | Create from config file |

#### Example

```rust
use clawdius_core::llm::LlmConfig;

// From environment
let config = LlmConfig::from_env("anthropic")?;

// From config file
let file_config = clawdius_core::Config::load_default()?.llm;
let config = LlmConfig::from_config(&file_config, "openai")?;
```

### 5.2 Provider Factory

Create LLM providers dynamically.

**Location:** `src/llm.rs`

```rust
pub fn create_provider(config: &LlmConfig) -> Result<LlmProvider>

pub fn create_provider_with_retry(
    config: &LlmConfig,
    retry_config: Option<RetryConfig>,
) -> Result<LlmClientWithRetry>
```

#### Supported Providers

| Provider | Identifier | Default Model |
|----------|------------|---------------|
| Anthropic | `anthropic` | claude-3-5-sonnet-20241022 |
| OpenAI | `openai` | gpt-4o |
| Ollama | `ollama` | llama3.2 |
| ZAI | `zai` | zai-default |

#### Example

```rust
use clawdius_core::llm::{create_provider, create_provider_with_retry, LlmConfig};

let config = LlmConfig::from_env("anthropic")?;

// Basic provider
let provider = create_provider(&config)?;
let response = provider.chat(messages).await?;

// With automatic retry
let client = create_provider_with_retry(&config, None)?;
let response = client.chat(messages).await?;
```

### 5.3 ChatMessage

Represents a message in a conversation.

**Location:** `src/llm/messages.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatRole {
    System,
    User,
    Assistant,
}
```

### 5.4 Retry Logic

Execute operations with automatic retry.

**Location:** `src/llm.rs`

```rust
pub async fn with_retry<T, F, Fut>(
    config: &RetryConfig,
    f: F,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
```

#### RetryConfig

**Location:** `src/config.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_retries: u32,           // Default: 3
    pub initial_delay_ms: u64,      // Default: 1000
    pub max_delay_ms: u64,          // Default: 30000
    pub exponential_base: f64,      // Default: 2.0
    pub retry_on: Vec<RetryCondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RetryCondition {
    RateLimit,      // HTTP 429
    Timeout,        // Request timeout
    ServerError,    // HTTP 5xx
    NetworkError,   // Connection failures
}
```

#### Example

```rust
use clawdius_core::llm::{with_retry, RetryConfig, RetryCondition};

let config = RetryConfig {
    max_retries: 5,
    initial_delay_ms: 2000,
    max_delay_ms: 60000,
    exponential_base: 2.0,
    retry_on: vec![
        RetryCondition::RateLimit,
        RetryCondition::ServerError,
    ],
};

let result = with_retry(&config, || async {
    // Operation that may fail transiently
    provider.chat(messages.clone()).await
}).await?;
```

---

## 6. Tool Execution

### 6.1 Tool Definition

**Location:** `src/tools.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub metadata: Option<serde_json::Value>,
}
```

### 6.2 Shell Tool

Execute shell commands with sandboxing.

**Location:** `src/tools/shell.rs`

```rust
pub struct ShellTool {
    config: ShellSandboxConfig,
    project_dir: PathBuf,
}

impl ShellTool {
    pub fn new(config: ShellSandboxConfig, project_dir: PathBuf) -> Self;
    pub fn execute(&self, params: ShellParams) -> Result<ShellResult>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellParams {
    pub command: String,
    pub timeout: u64,        // Default: 120000ms
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}
```

#### Example

```rust
use clawdius_core::tools::shell::{ShellTool, ShellParams};
use clawdius_core::config::ShellSandboxConfig;

let config = ShellSandboxConfig::default();
let tool = ShellTool::new(config, std::env::current_dir()?);

let params = ShellParams {
    command: "cargo test".to_string(),
    timeout: 60000,
    cwd: None,
};

let result = tool.execute(params)?;
println!("Exit code: {}", result.exit_code);
println!("Output: {}", result.stdout);
```

### 6.3 File Tool

File system operations.

**Location:** `src/tools/file.rs`

Operations include:
- Read file contents
- Write file contents
- List directory contents
- Search files by pattern

### 6.4 Git Tool

Git repository operations.

**Location:** `src/tools/git.rs`

Operations include:
- Get current status
- Stage/unstage files
- Create commits
- View diff
- Get file history

---

## 7. Configuration

### 7.1 Config

Main configuration structure.

**Location:** `src/config.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub project: ProjectConfig,
    pub storage: StorageConfig,
    pub llm: LlmConfig,
    pub session: SessionConfig,
    pub output: OutputConfig,
    pub shell_sandbox: ShellSandboxConfig,
}
```

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `load` | `fn load(path: &Path) -> Result<Self>` | Load from file |
| `load_default` | `fn load_default() -> Result<Self>` | Load from default locations |
| `load_or_default` | `fn load_or_default() -> Self` | Load or return default |
| `save` | `fn save(&self, path: &Path) -> Result<()>` | Save to file |
| `default_path` | `fn default_path() -> PathBuf` | Get default config path |

#### Example

```rust
use clawdius_core::Config;

// Load from default location
let config = Config::load_default()?;

// Load from specific path
let config = Config::load(std::path::Path::new("custom/config.toml"))?;

// Access LLM settings
println!("Default provider: {:?}", config.llm.default_provider);
println!("Max tokens: {}", config.llm.max_tokens);
```

### 7.2 ShellSandboxConfig

Configure shell command sandboxing.

**Location:** `src/config.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellSandboxConfig {
    pub blocked_commands: Vec<String>,
    pub timeout_secs: u64,
    pub max_output_bytes: usize,
    pub restrict_to_cwd: bool,
}
```

---

## 8. Keyring Storage

Secure API key storage using system keyring.

**Location:** `src/config.rs` (feature-gated: `keyring`)

```rust
#[cfg(feature = "keyring")]
pub struct KeyringStorage {
    service: String,
}

#[cfg(feature = "keyring")]
impl KeyringStorage {
    pub fn new() -> Self;
    pub fn global() -> &'static KeyringStorage;
    pub fn get_api_key(&self, provider: &str) -> Result<Option<String>>;
    pub fn set_api_key(&self, provider: &str, key: &str) -> Result<()>;
    pub fn delete_api_key(&self, provider: &str) -> Result<()>;
}
```

#### Example

```rust
#[cfg(feature = "keyring")]
{
    use clawdius_core::config::KeyringStorage;

    let storage = KeyringStorage::global();

    // Store a key
    storage.set_api_key("anthropic", "sk-ant-...")?;

    // Retrieve a key
    if let Some(key) = storage.get_api_key("anthropic")? {
        println!("Key found: {}***", &key[..8]);
    }

    // Delete a key
    storage.delete_api_key("anthropic")?;
}
```

---

## 9. Timeline API

### 9.1 TimelineManager

Manage file timeline checkpoints and rollback.

**Location:** `src/timeline.rs`

```rust
pub struct TimelineManager {
    db_path: PathBuf,
    project_root: PathBuf,
}

impl TimelineManager {
    pub fn new(project_root: PathBuf) -> Result<Self>;
    pub fn create_checkpoint(&self, name: &str, description: Option<&str>, tag: Option<&str>) -> Result<Checkpoint>;
    pub fn list_checkpoints(&self, limit: Option<usize>) -> Result<Vec<Checkpoint>>;
    pub fn get_checkpoint(&self, id: &str) -> Result<Option<Checkpoint>>;
    pub fn rollback(&self, checkpoint_id: &str, dry_run: bool) -> Result<RollbackResult>;
    pub fn diff_checkpoints(&self, from_id: &str, to_id: &str) -> Result<CheckpointDiff>;
    pub fn get_file_history(&self, file_path: &str, limit: Option<usize>) -> Result<Vec<FileHistoryEntry>>;
}
```

#### Checkpoint

**Location:** `src/timeline.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub tag: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub files_changed: usize,
    pub checksum: String,
}
```

#### Example

```rust
use clawdius_core::timeline::TimelineManager;
use std::path::PathBuf;

let manager = TimelineManager::new(PathBuf::from("./project"))?;

// Create checkpoint
let checkpoint = manager.create_checkpoint(
    "before-refactor",
    Some("Pre-refactor checkpoint"),
    None
)?;

println!("Created checkpoint: {}", checkpoint.id);

// List checkpoints
let checkpoints = manager.list_checkpoints(Some(10))?;
for cp in checkpoints {
    println!("{}: {} ({})", cp.id, cp.name, cp.timestamp);
}

// Rollback
let result = manager.rollback(&checkpoint.id, false)?;
println!("Rolled back {} files", result.files_restored);
```

### 9.2 CheckpointDiff

**Location:** `src/timeline.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointDiff {
    pub from_checkpoint: Checkpoint,
    pub to_checkpoint: Checkpoint,
    pub files_added: Vec<String>,
    pub files_modified: Vec<FileChange>,
    pub files_deleted: Vec<String>,
    pub stats: DiffStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub additions: usize,
    pub deletions: usize,
    pub diff: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffStats {
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
}
```

#### Example

```rust
let diff = manager.diff_checkpoints("abc123", "def456")?;

println!("Files changed: {}", diff.stats.files_changed);
println!("Insertions: {}", diff.stats.insertions);
println!("Deletions: {}", diff.stats.deletions);

for change in diff.files_modified {
    println!("Modified: {}", change.path);
    println!("  +{} -{}", change.additions, change.deletions);
}
```

### 9.3 FileHistoryEntry

**Location:** `src/timeline.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHistoryEntry {
    pub checkpoint_id: String,
    pub checkpoint_name: String,
    pub timestamp: DateTime<Utc>,
    pub operation: FileOperation,
    pub diff: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileOperation {
    Created,
    Modified,
    Deleted,
}
```

#### Example

```rust
let history = manager.get_file_history("src/main.rs", Some(20))?;

for entry in history {
    println!("{}: {:?} in {}", entry.timestamp, entry.operation, entry.checkpoint_name);
}
```

---

## 10. JSON Output API

### 10.1 OutputFormat

**Location:** `src/output.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
    StreamJson,
}
```

### 10.2 JsonResponse

Standard JSON response structure.

**Location:** `src/output.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonResponse<T> {
    pub status: ResponseStatus,
    pub data: T,
    pub error: Option<ErrorInfo>,
    pub metadata: ResponseMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseStatus {
    Success,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMetadata {
    pub version: String,
    pub timestamp: DateTime<Utc>,
}
```

#### Example

```rust
use clawdius_core::output::{JsonResponse, ResponseStatus};

let response = JsonResponse {
    status: ResponseStatus::Success,
    data: checkpoint,
    error: None,
    metadata: ResponseMetadata {
        version: "0.7.0".to_string(),
        timestamp: Utc::now(),
    },
};

let json = serde_json::to_string_pretty(&response)?;
println!("{}", json);
```

---

## 11. Completions API

### 11.1 CompletionHandler

Enhanced code completion with caching.

**Location:** `src/completions.rs`

```rust
pub struct CompletionHandler {
    cache: LruCache<CacheKey, CompletionResult>,
    config: CompletionConfig,
}

impl CompletionHandler {
    pub fn new(config: CompletionConfig) -> Self;
    pub fn complete(&mut self, request: CompletionRequest) -> Result<CompletionResult>;
    pub fn clear_cache(&mut self);
}

#[derive(Debug, Clone)]
pub struct CompletionConfig {
    pub cache_size: usize,
    pub timeout_ms: u64,
    pub fallback_enabled: bool,
    pub languages: HashMap<String, LanguageConfig>,
}

#[derive(Debug, Clone)]
pub struct LanguageConfig {
    pub enabled: bool,
    pub max_tokens: usize,
}
```

### 11.2 CompletionRequest

**Location:** `src/completions.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub code: String,
    pub language: String,
    pub cursor_position: usize,
    pub file_path: Option<String>,
    pub context: Option<String>,
}
```

### 11.3 CompletionResult

**Location:** `src/completions.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResult {
    pub completions: Vec<Completion>,
    pub cache_hit: bool,
    pub generation_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Completion {
    pub text: String,
    pub display_text: String,
    pub kind: CompletionKind,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompletionKind {
    Function,
    Variable,
    Type,
    Keyword,
    Snippet,
}
```

#### Example

```rust
use clawdius_core::completions::{CompletionHandler, CompletionConfig, CompletionRequest};

let config = CompletionConfig {
    cache_size: 100,
    timeout_ms: 5000,
    fallback_enabled: true,
    languages: Default::default(),
};

let mut handler = CompletionHandler::new(config);

let request = CompletionRequest {
    code: "fn main() {\n    let x = ".to_string(),
    language: "rust".to_string(),
    cursor_position: 26,
    file_path: Some("src/main.rs".to_string()),
    context: None,
};

let result = handler.complete(request)?;

println!("Found {} completions", result.completions.len());
println!("Cache hit: {}", result.cache_hit);
```

---

## 12. Traits

### 5.1 Result Type Alias

**Location:** `src/error.rs`

```rust
pub type Result<T> = std::result::Result<T, ClawdiusError>;
```

---

## 13. Feature Flags

### 13.1 Default Features

| Feature | Description |
|---------|-------------|
| `mimalloc` | High-performance global allocator |

### 13.2 Optional Features

| Feature | Description |
|---------|-------------|
| `keyring` | Secure API key storage in system keyring |
| `hft-mode` | Enable HFT-specific optimizations |
| `broker-mode` | Enable financial trading features |
| `timeline` | Enable file timeline system (enabled by default) |
| `completions` | Enable enhanced code completions (enabled by default) |

---

## 14. Re-exports

The library re-exports commonly used types:

```rust
pub use error::{Error, Result};
pub use config::Config;
pub use session::{Session, SessionStore, SessionManager};
pub use context::{Context, ContextItem, Mention, MentionResolver};
pub use output::OutputFormat;
pub use diff::{FileDiff, DiffPreview, DiffStats, DiffRenderer, DiffTheme};
pub use llm::{LlmConfig, ChatMessage, ChatRole, RetryConfig, RetryCondition};
pub use timeline::{TimelineManager, Checkpoint, CheckpointDiff, FileHistoryEntry};
pub use completions::{CompletionHandler, CompletionConfig, CompletionRequest, CompletionResult};

#[cfg(feature = "keyring")]
pub use config::KeyringStorage;
```

---

## 15. Safety

### 12.1 Unsafe Code Policy

Per `Cargo.toml` configuration:

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
```

All unsafe code is forbidden in the Clawdius codebase.

### 12.2 Panic Policy

Per `Cargo.toml` configuration:

```toml
[workspace.lints.clippy]
panic = "forbid"
unwrap_used = "allow"
expect_used = "allow"
```

---

## 16. Example Usage

### Complete Example

```rust
use clawdius_core::{
    Config,
    llm::{create_provider_with_retry, LlmConfig, ChatMessage, ChatRole},
    tools::shell::{ShellTool, ShellParams},
    config::ShellSandboxConfig,
};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> clawdius_core::Result<()> {
    // Load configuration
    let config = Config::load_default()?;
    
    // Create LLM client with retry
    let llm_config = LlmConfig::from_config(&config.llm, "anthropic")?;
    let client = create_provider_with_retry(&llm_config, config.llm.retry)?;
    
    // Send a message
    let messages = vec![
        ChatMessage {
            role: ChatRole::User,
            content: "Hello, Clawdius!".to_string(),
        }
    ];
    
    let response = client.chat(messages).await?;
    println!("Response: {}", response);
    
    // Execute a shell command
    let shell_config = ShellSandboxConfig::default();
    let shell = ShellTool::new(shell_config, PathBuf::from("."));
    
    let result = shell.execute(ShellParams {
        command: "cargo --version".to_string(),
        timeout: 5000,
        cwd: None,
    })?;
    
    println!("Cargo version: {}", result.stdout);
    
    Ok(())
}
```
