# Clawdius API Reference

**Version:** 0.6.0  
**Last Updated:** 2026-03-01

---

## Table of Contents

1. [Core Types](#1-core-types)
2. [State Machine](#2-state-machine)
3. [Error Types](#3-error-types)
4. [Version Information](#4-version-information)
5. [Traits](#5-traits)

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

## 5. Traits

### 5.1 Result Type Alias

**Location:** `src/error.rs`

```rust
pub type Result<T> = std::result::Result<T, ClawdiusError>;
```

---

## 6. Feature Flags

### 6.1 Default Features

| Feature | Description |
|---------|-------------|
| `mimalloc` | High-performance global allocator |

### 6.2 Optional Features

| Feature | Description |
|---------|-------------|
| `hft-mode` | Enable HFT-specific optimizations |
| `broker-mode` | Enable financial trading features |

---

## 7. Re-exports

The library re-exports commonly used types:

```rust
pub use error::{ClawdiusError, Result};
pub use fsm::{Phase, StateMachine};
pub use version::VERSION;
```

---

## 8. Safety

### 8.1 Unsafe Code Policy

Per `Cargo.toml` configuration:

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
```

All unsafe code is forbidden in the Clawdius codebase.

### 8.2 Panic Policy

Per `Cargo.toml` configuration:

```toml
[workspace.lints.clippy]
panic = "forbid"
unwrap_used = "deny"
expect_used = "deny"
```

Panics and unwrap/expect are prohibited in production code.

---

## 9. Memory Layout

### 9.1 Phase Size

```rust
assert_eq!(std::mem::size_of::<Phase>(), 1); // repr(i8) for negative values
```

### 9.2 HotPathError Size

```rust
assert_eq!(std::mem::size_of::<HotPathError>(), 1); // repr(u8)
```

---

## 10. Example Usage

### Complete Example

```rust
use clawdius::{Phase, StateMachine, Result, VERSION};

fn main() -> Result<()> {
    println!("Clawdius v{}", VERSION);
    
    let mut sm = StateMachine::new()?;
    
    loop {
        match sm.tick() {
            TransitionResult::Continue => {
                // Process current phase
            }
            TransitionResult::Transition(p) => {
                println!("Transitioned to: {}", p.display_name());
            }
            TransitionResult::Complete => {
                println!("Lifecycle complete");
                break;
            }
            TransitionResult::Error(e) => {
                return Err(e);
            }
        }
    }
    
    Ok(())
}
```
