# Anti-Patterns

**Document ID:** AP-CLAWDIUS-008-5  
**Version:** 1.0.0  
**Phase:** 7.5 (Knowledge Base Update)  
**Date:** 2026-03-01  
**Status:** APPROVED

---

## Overview

This document catalogs patterns and practices that have been identified as problematic during the Clawdius R&D cycle. These anti-patterns should be avoided in future development.

---

## 1. Concurrency Anti-Patterns

### 1.1 Using `std::sync::Mutex` on Hot Paths

**Anti-Pattern:**
```rust
// ❌ WRONG
use std::sync::Mutex;

struct SharedState {
    data: Mutex<Vec<u8>>,
}

fn hot_path(state: &SharedState) {
    let mut data = state.data.lock().unwrap(); // Blocks, non-deterministic
    data.push(42);
}
```

**Problems:**
- OS-level blocking
- Non-deterministic latency
- Priority inversion risk
- Cache line bouncing

**Correct Approach:**
```rust
// ✅ CORRECT
use std::sync::atomic::{AtomicU64, Ordering};
use crossbeam_utils::CachePadded;

struct SharedState {
    counter: CachePadded<AtomicU64>,
}

fn hot_path(state: &SharedState) {
    state.counter.fetch_add(1, Ordering::Relaxed); // Lock-free
}
```

---

### 1.2 `SeqCst` Memory Ordering Everywhere

**Anti-Pattern:**
```rust
// ❌ WRONG
use std::sync::atomic::Ordering::SeqCst;

fn producer(buffer: &RingBuffer, value: T) {
    buffer.tail.store(tail, SeqCst); // Overly strong, slow
}

fn consumer(buffer: &RingBuffer) -> Option<T> {
    buffer.tail.load(SeqCst) // Overly strong, slow
}
```

**Problems:**
- Emits `MFENCE` on x86 (expensive)
- Unnecessary synchronization
- Latency impact on hot paths

**Correct Approach:**
```rust
// ✅ CORRECT
use std::sync::atomic::Ordering::{Acquire, Release, Relaxed};

fn producer(buffer: &RingBuffer, value: T) {
    buffer.tail.store(tail, Release); // Sufficient for producer
}

fn consumer(buffer: &RingBuffer) -> Option<T> {
    buffer.tail.load(Acquire) // Sufficient for consumer
}
```

---

### 1.3 Unbounded Channels

**Anti-Pattern:**
```rust
// ❌ WRONG
use std::sync::mpsc::channel;

let (tx, rx) = channel(); // Unbounded!
```

**Problems:**
- Memory exhaustion under load
- No backpressure signal
- OOM crashes

**Correct Approach:**
```rust
// ✅ CORRECT
use std::sync::mpsc::sync_channel;

let (tx, rx) = sync_channel(1024); // Bounded

// Or with tower for async
use tower::limit::ConcurrencyLimit;
```

---

## 2. Memory Anti-Patterns

### 2.1 `Vec::with_capacity` on Hot Path

**Anti-Pattern:**
```rust
// ❌ WRONG
fn hot_path() {
    let mut buffer = Vec::with_capacity(1024); // Lazy allocation!
    // Causes page faults on first access
}
```

**Problems:**
- Lazy allocation causes page faults
- Non-deterministic timing
- Cache pollution

**Correct Approach:**
```rust
// ✅ CORRECT
fn hot_path(arena: &Arena) {
    let buffer = arena.alloc(1024); // Pre-allocated
    // No page faults
}
```

---

### 2.2 Default System Allocator in HFT

**Anti-Pattern:**
```rust
// ❌ WRONG (no explicit allocator)
// Uses default system allocator with lock contention
```

**Problems:**
- Global lock on malloc/free
- Non-deterministic allocation time
- Fragmentation

**Correct Approach:**
```rust
// ✅ CORRECT
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
```

---

### 2.3 Heap Allocation in Inner Loops

**Anti-Pattern:**
```rust
// ❌ WRONG
fn process_messages(messages: &[Message]) {
    for msg in messages {
        let processed = ProcessedMessage::new(msg); // Heap alloc per message!
    }
}
```

**Problems:**
- Allocation overhead
- Cache misses
- Non-deterministic latency

**Correct Approach:**
```rust
// ✅ CORRECT
fn process_messages<'a>(messages: &[Message], arena: &'a Arena) -> &'a [ProcessedMessage] {
    let result = arena.alloc_slice(messages.len());
    for (i, msg) in messages.iter().enumerate() {
        result[i] = ProcessedMessage::from(msg);
    }
    result
}
```

---

## 3. Error Handling Anti-Patterns

### 3.1 `unwrap()` and `expect()` in Production

**Anti-Pattern:**
```rust
// ❌ WRONG
fn process(input: &str) -> Result {
    let parsed: i32 = input.parse().unwrap(); // Can panic!
    Ok(parsed)
}
```

**Problems:**
- Thread panic on error
- `panic = "abort"` terminates process
- No error recovery

**Correct Approach:**
```rust
// ✅ CORRECT
fn process(input: &str) -> Result<i32, ParseError> {
    let parsed: i32 = input.parse().map_err(|e| ParseError::InvalidInteger(e))?;
    Ok(parsed)
}
```

**Note:** Clippy is configured to deny `unwrap_used` and `expect_used`.

---

### 3.2 `anyhow` or `eyre` in HFT Code

**Anti-Pattern:**
```rust
// ❌ WRONG (in hot path)
use anyhow::Result;

fn hot_path() -> Result<()> {
    // anyhow allocates on error
}
```

**Problems:**
- Heap allocation on error path
- Dynamic dispatch
- Non-zero cost even on success

**Correct Approach:**
```rust
// ✅ CORRECT (in hot path)
#[repr(u8)]
pub enum HotPathError {
    None = 0,
    InvalidInput = 1,
    // ... C-like enum, zero allocation
}
```

---

### 3.3 Ignoring Errors

**Anti-Pattern:**
```rust
// ❌ WRONG
fn process() {
    let _ = risky_operation(); // Error ignored!
}
```

**Problems:**
- Silent failures
- Impossible to debug
- Violates error handling SOP

**Correct Approach:**
```rust
// ✅ CORRECT
fn process() -> Result<()> {
    risky_operation()?; // Propagate error
    Ok(())
}

// Or explicit handling
fn process() {
    if let Err(e) = risky_operation() {
        tracing::warn!("Non-critical error: {}", e);
    }
}
```

---

## 4. Security Anti-Patterns

### 4.1 Passing Secrets to Sandboxes

**Anti-Pattern:**
```rust
// ❌ WRONG
fn run_in_sandbox(code: &str, api_key: &str) {
    let env = format!("API_KEY={}", api_key);
    sandbox.run(code, &[env]); // Secret exposed!
}
```

**Problems:**
- Secret visible to sandboxed code
- Potential exfiltration
- Audit trail broken

**Correct Approach:**
```rust
// ✅ CORRECT
fn run_in_sandbox(code: &str, proxy: &SecretProxy) {
    // Sandbox makes request through proxy
    // Secret never leaves host
    let result = sandbox.run(code, proxy)?;
}
```

---

### 4.2 Executing Unvalidated settings.toml

**Anti-Pattern:**
```rust
// ❌ WRONG
fn load_settings(path: &Path) -> Settings {
    let content = fs::read_to_string(path)?;
    toml::from_str(&content)? // No validation!
}
```

**Problems:**
- Arbitrary code execution via shell commands
- Path traversal
- Injection attacks

**Correct Approach:**
```rust
// ✅ CORRECT
fn load_settings(path: &Path) -> Result<Settings, SettingsError> {
    let content = fs::read_to_string(path)?;
    let raw: RawSettings = toml::from_str(&content)?;
    
    // Validate against safety policy
    validate_no_shell_commands(&raw)?;
    validate_no_path_traversal(&raw)?;
    validate_no_forbidden_keys(&raw)?;
    
    Ok(Settings::from(raw))
}
```

---

### 4.3 Capability Escalation

**Anti-Pattern:**
```rust
// ❌ WRONG
impl CapabilityToken {
    pub fn add_permission(&mut self, perm: Permission) {
        self.permissions.push(perm); // Escalation!
    }
}
```

**Problems:**
- Violates capability monotonicity
- Privilege escalation
- Security audit failure

**Correct Approach:**
```rust
// ✅ CORRECT
impl CapabilityToken {
    pub fn derive(&self, subset: &[Permission]) -> Result<Self, Error> {
        // Only allow subset (attenuation)
        if !self.permissions.contains_all(subset) {
            return Err(Error::AttenuationViolation);
        }
        // ...
    }
}
```

---

## 5. Type System Anti-Patterns

### 5.1 Primitive Obsession

**Anti-Pattern:**
```rust
// ❌ WRONG
fn place_order(symbol: String, quantity: i32, price: f64) { ... }
```

**Problems:**
- No type-level validation
- Easy to swap arguments
- No domain semantics

**Correct Approach:**
```rust
// ✅ CORRECT
use nutype::nutype;

#[nutype(validate(not_empty))]
pub struct Symbol(String);

#[nutype(validate(predicate = |q| *q > 0))]
pub struct Quantity(i32);

#[nutype(validate(predicate = |p| *p > 0.0))]
pub struct Price(Decimal); // Not f64!

fn place_order(symbol: Symbol, quantity: Quantity, price: Price) { ... }
```

---

### 5.2 Boolean Flags for State

**Anti-Pattern:**
```rust
// ❌ WRONG
struct Order {
    is_pending: bool,
    is_filled: bool,
    is_cancelled: bool,
}
```

**Problems:**
- Invalid states possible (all true)
- No exhaustiveness checking
- Unclear semantics

**Correct Approach:**
```rust
// ✅ CORRECT
enum OrderState {
    Pending,
    Filled { fill_price: Decimal },
    Cancelled { reason: CancelReason },
}

struct Order {
    state: OrderState,
}
```

---

### 5.3 Using f64 for Money

**Anti-Pattern:**
```rust
// ❌ WRONG
let total: f64 = price * quantity; // IEEE-754 imprecision!
```

**Problems:**
- Floating-point rounding errors
- Non-associative arithmetic
- Financial regulatory violations

**Correct Approach:**
```rust
// ✅ CORRECT
use rust_decimal::Decimal;

let total: Decimal = price * quantity; // Exact arithmetic
```

---

## 6. Documentation Anti-Patterns

### 6.1 Outdated Code Comments

**Anti-Pattern:**
```rust
// ❌ WRONG
/// Returns the number of phases (12)  // Actually 24!
fn phase_count() -> usize { 24 }
```

**Problems:**
- Misleading information
- Worse than no documentation
- Trust erosion

**Correct Approach:**
- Keep documentation synchronized with code
- Use doc tests to verify examples
- Update docs when changing code

---

### 6.2 Missing Error Documentation

**Anti-Pattern:**
```rust
// ❌ WRONG
/// Process an order
fn process_order(order: Order) -> Result<(), Error>;
```

**Problems:**
- No guidance on error handling
- Hidden failure modes

**Correct Approach:**
```rust
// ✅ CORRECT
/// Process an order
/// 
/// # Errors
/// 
/// Returns `Error::InsufficientFunds` if account balance is too low.
/// Returns `Error::MarketClosed` if market is not accepting orders.
/// Returns `Error::RateLimited` if order rate limit is exceeded.
fn process_order(order: Order) -> Result<(), Error>;
```

---

## 7. Testing Anti-Patterns

### 7.1 Testing Only Happy Path

**Anti-Pattern:**
```rust
// ❌ WRONG
#[test]
fn test_process() {
    let result = process(valid_input).unwrap();
    assert_eq!(result, expected);
}
```

**Problems:**
- Edge cases untested
- Error paths untested
- False confidence

**Correct Approach:**
```rust
// ✅ CORRECT
#[test]
fn test_process_valid() { ... }

#[test]
fn test_process_empty_input() {
    assert!(matches!(process(""), Err(Error::EmptyInput)));
}

#[test]
fn test_process_overflow() {
    assert!(matches!(process(MAX_VALUE), Err(Error::Overflow)));
}

// Plus property-based tests
proptest! {
    #[test]
    fn test_process_never_panics(input: String) {
        let _ = process(&input); // Should never panic
    }
}
```

---

### 7.2 Shared Test State

**Anti-Pattern:**
```rust
// ❌ WRONG
static mut TEST_DB: Option<Database> = None;

#[test]
fn test_a() {
    let db = unsafe { TEST_DB.get_or_insert(Database::new()) };
    db.insert("key", "value_a");
}

#[test]
fn test_b() {
    let db = unsafe { TEST_DB.get_or_insert(Database::new()) };
    // State from test_a may affect test_b!
}
```

**Problems:**
- Test order dependency
- Race conditions
- Flaky tests

**Correct Approach:**
```rust
// ✅ CORRECT
#[test]
fn test_a() {
    let db = Database::new_in_memory(); // Isolated
    db.insert("key", "value_a");
}

#[test]
fn test_b() {
    let db = Database::new_in_memory(); // Fresh instance
    // No interference from test_a
}
```

---

## 8. Summary Checklist

| Category | Anti-Pattern | Correct Pattern |
|----------|--------------|-----------------|
| Concurrency | Mutex on hot path | Lock-free atomics |
| Concurrency | SeqCst everywhere | Acquire/Release |
| Concurrency | Unbounded channels | Bounded with backpressure |
| Memory | Vec::with_capacity hot path | Arena allocation |
| Memory | Default allocator | mimalloc |
| Memory | Heap in inner loops | Stack/arena |
| Error | unwrap/expect | Proper error propagation |
| Error | anyhow in HFT | Flat C-like enums |
| Error | Ignoring errors | Explicit handling |
| Security | Secrets to sandbox | Secret proxy |
| Security | Unvalidated config | Safety policy validation |
| Security | Capability escalation | Attenuation-only |
| Types | Primitive obsession | Domain types |
| Types | Boolean state | Enum state |
| Types | f64 for money | Decimal |
| Docs | Outdated comments | Sync with code |
| Docs | Missing error docs | Document all errors |
| Testing | Happy path only | Boundary + adversarial |
| Testing | Shared state | Isolated fixtures |

---

## 9. Sign-off

| Role | Name | Date | Status |
|------|------|------|--------|
| Quality Lead | QA Agent | 2026-03-01 | ✅ APPROVED |
| Security Lead | Sentinel | 2026-03-01 | ✅ APPROVED |
| Performance Lead | HFT Team | 2026-03-01 | ✅ APPROVED |

---

**Document Status:** APPROVED  
**Next Review:** After Phase 8 implementation
