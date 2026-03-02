//! Error types for Clawdius
//!
//! Implements the "Error Bifurcation" principle from the Rust SOP:
//! - Flat, C-like enums for hot-path errors (zero allocation)
//! - Thiserror for control plane errors

use std::fmt;
use thiserror::Error;

/// Main result type for Clawdius operations
pub type Result<T> = std::result::Result<T, ClawdiusError>;

/// Top-level error type for Clawdius
#[derive(Error, Debug)]
pub enum ClawdiusError {
    /// State machine errors
    #[error("State machine error: {0}")]
    StateMachine(#[from] StateMachineError),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Database errors
    #[error("Database error: {0}")]
    Database(String),

    /// SQLite errors
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// LLM errors
    #[error("LLM error: {0}")]
    Llm(String),

    /// Sandbox errors
    #[error("Sandbox error: {0}")]
    Sandbox(#[from] SandboxError),

    /// Host kernel errors
    #[error("Host kernel error: {0}")]
    Host(#[from] HostError),

    /// SOP violation errors
    #[error("SOP violation: {0}")]
    SopViolation(String),

    /// Brain errors
    #[error("Brain error: {0}")]
    Brain(#[from] BrainError),

    /// Broker errors
    #[error("Broker error: {0}")]
    Broker(#[from] BrokerError),

    /// Phase transition errors
    #[error("Invalid phase transition: {from:?} -> {to:?}")]
    InvalidTransition {
        /// Source phase
        from: super::fsm::Phase,
        /// Target phase
        to: super::fsm::Phase,
    },
}

/// Host kernel specific errors
#[derive(Error, Debug)]
pub enum HostError {
    /// Initialization failed
    #[error("Initialization failed: {reason}")]
    InitializationFailed {
        /// Reason for failure
        reason: String,
    },

    /// Kernel already running
    #[error("Kernel already running")]
    AlreadyRunning,

    /// Kernel not running
    #[error("Kernel not running")]
    NotRunning,

    /// Component failure
    #[error("Component {component} failed: {reason}")]
    ComponentFailure {
        /// Component ID
        component: String,
        /// Failure reason
        reason: String,
    },

    /// Resource exhausted
    #[error("Resource exhausted: {resource}")]
    ResourceExhausted {
        /// Resource type
        resource: String,
    },
}

/// State machine specific errors
#[derive(Error, Debug)]
pub enum StateMachineError {
    /// Invalid state transition attempted
    #[error("Invalid transition from {from} to {to}")]
    InvalidTransition {
        /// Source phase
        from: String,
        /// Target phase
        to: String,
    },

    /// Quality gate failed
    #[error("Quality gate failed: {gate}")]
    QualityGateFailed {
        /// Name of the failed gate
        gate: String,
    },

    /// Required artifact missing
    #[error("Required artifact missing: {artifact}")]
    MissingArtifact {
        /// Path to the missing artifact
        artifact: String,
    },

    /// Phase prerequisites not met
    #[error("Phase prerequisites not met: {details}")]
    PrerequisitesNotMet {
        /// Details about missing prerequisites
        details: String,
    },
}

/// Sandbox execution errors
#[derive(Error, Debug)]
pub enum SandboxError {
    /// Failed to create sandbox
    #[error("Failed to create sandbox: {reason}")]
    CreationFailed {
        /// Reason for failure
        reason: String,
    },

    /// Sandbox execution failed
    #[error("Sandbox execution failed: {exit_code}")]
    ExecutionFailed {
        /// Exit code from sandboxed process
        exit_code: i32,
    },

    /// Capability violation detected
    #[error("Capability violation: {capability}")]
    CapabilityViolation {
        /// The violated capability
        capability: String,
    },

    /// Timeout exceeded
    #[error("Sandbox execution timeout after {seconds}s")]
    Timeout {
        /// Timeout duration in seconds
        seconds: u64,
    },

    /// Settings validation error
    #[error("Settings validation error: {0}")]
    SettingsValidation(String),
}

/// Brain WASM runtime errors
#[derive(Error, Debug)]
pub enum BrainError {
    /// WASM module compilation failed
    #[error("WASM compilation failed: {reason}")]
    WasmCompileFailed {
        /// Reason for failure
        reason: String,
    },

    /// WASM runtime trap
    #[error("WASM trap: {message}")]
    WasmTrap {
        /// Trap message
        message: String,
    },

    /// RPC version mismatch
    #[error("RPC version mismatch: expected {expected}, got {actual}")]
    RpcVersionMismatch {
        /// Expected version
        expected: u32,
        /// Actual version
        actual: u32,
    },

    /// LLM call failed
    #[error("LLM call failed: {reason}")]
    LlmCallFailed {
        /// Reason for failure
        reason: String,
    },

    /// Capability insufficient
    #[error("Capability insufficient: required {required}")]
    CapabilityInsufficient {
        /// Required capability
        required: String,
    },

    /// Memory limit exceeded
    #[error("Memory limit exceeded: {bytes} bytes")]
    MemoryLimitExceeded {
        /// Bytes attempted
        bytes: usize,
    },

    /// SOP violation
    #[error("SOP violation: {violation}")]
    SopViolation {
        /// Violation description
        violation: String,
    },

    /// Prompt too long
    #[error("Prompt too long: {tokens} tokens")]
    PromptTooLong {
        /// Token count
        tokens: usize,
    },

    /// Instance not initialized
    #[error("Brain instance not initialized")]
    NotInitialized,

    /// Instance already initialized
    #[error("Brain instance already initialized")]
    AlreadyInitialized,

    /// RPC serialization error
    #[error("RPC serialization error: {reason}")]
    SerializationError {
        /// Reason for failure
        reason: String,
    },
}

/// Hot-path error codes (zero allocation, C-like enum)
/// Used for performance-critical code paths per Rust SOP
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotPathError {
    /// No error
    None = 0,
    /// Buffer overflow
    BufferOverflow = 1,
    /// Invalid input
    InvalidInput = 2,
    /// Timeout
    Timeout = 3,
    /// Resource exhausted
    ResourceExhausted = 4,
    /// Parse error
    ParseError = 5,
}

impl fmt::Display for HotPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "no error"),
            Self::BufferOverflow => write!(f, "buffer overflow"),
            Self::InvalidInput => write!(f, "invalid input"),
            Self::Timeout => write!(f, "timeout"),
            Self::ResourceExhausted => write!(f, "resource exhausted"),
            Self::ParseError => write!(f, "parse error"),
        }
    }
}

impl std::error::Error for HotPathError {}

/// Broker HFT errors
#[derive(Error, Debug)]
pub enum BrokerError {
    /// Ring buffer error
    #[error("Ring buffer error: {0}")]
    RingBuffer(String),

    /// Risk check failed
    #[error("Risk check failed: {reason}")]
    RiskCheckFailed {
        /// Reason for failure
        reason: String,
    },

    /// Signal dispatch failed
    #[error("Signal dispatch failed: {reason}")]
    DispatchFailed {
        /// Reason for failure
        reason: String,
    },

    /// Market data error
    #[error("Market data error: {reason}")]
    MarketDataError {
        /// Reason for failure
        reason: String,
    },

    /// Configuration error
    #[error("Broker configuration error: {0}")]
    Config(String),

    /// Not running
    #[error("Broker not running")]
    NotRunning,

    /// Already running
    #[error("Broker already running")]
    AlreadyRunning,

    /// Latency bound exceeded
    #[error("Latency bound exceeded: operation took {actual_us}us, max {max_us}us")]
    LatencyExceeded {
        /// Actual duration in microseconds
        actual_us: u64,
        /// Maximum allowed in microseconds
        max_us: u64,
    },
}
