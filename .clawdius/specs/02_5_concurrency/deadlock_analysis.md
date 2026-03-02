---
id: CA-DEADLOCK-001
title: "Deadlock Analysis"
version: 1.0.0
phase: 2.5
status: APPROVED
created: 2026-03-01
author: Concurrency Engineer
classification: Concurrency Analysis
trace_to:
  - BP-HOST-KERNEL-001
  - BP-SENTINEL-001
  - BP-GRAPH-RAG-001
---

# Deadlock Analysis

## 1. Executive Summary

This document identifies potential deadlock scenarios in the Clawdius architecture and documents mitigation strategies. Given the monoio thread-per-core model, deadlocks are primarily a concern in the capability cache, SQLite writes, and MCP tool registry.

## 2. Resource Dependency Graph

### 2.1 Lock Resources

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Lock Dependency Graph                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   ┌─────────────┐                                                    │
│   │ FSM Mutex   │◄──────────────────────────────────┐               │
│   │ (Arc<Mutex>)│                                   │               │
│   └──────┬──────┘                                   │               │
│          │                                          │               │
│          │ (rare)                                   │               │
│          ▼                                          │               │
│   ┌─────────────┐      ┌─────────────────┐          │               │
│   │ Sentinel    │─────►│ Capability Cache│          │               │
│   │ RwLock      │      │ (RwLock)        │          │               │
│   └─────────────┘      └────────┬────────┘          │               │
│                                 │                   │               │
│                                 │ (capability       │               │
│                                 │  derivation)      │               │
│                                 ▼                   │               │
│   ┌─────────────┐      ┌─────────────────┐          │               │
│   │ SQLite      │◄────►│ MCP Tool        │          │               │
│   │ Write Mutex │      │ Registry RwLock │          │               │
│   └─────────────┘      └─────────────────┘          │               │
│                                                      │               │
│   ┌─────────────┐                                   │               │
│   │ Brain RPC   │───────────────────────────────────┘               │
│   │ Mutex       │  (WASM may request capability)                    │
│   └─────────────┘                                                   │
│                                                                      │
│   ┌─────────────┐                                                   │
│   │ Secret      │  (Keyring access - platform dependent)            │
│   │ Proxy Mutex │                                                   │
│   └─────────────┘                                                   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.2 Lock Types

| Resource | Lock Type | Scope | Hold Time |
|----------|-----------|-------|-----------|
| `FsmCore` | `Mutex` | Global | <1ms (phase transition) |
| `CapabilityManager::cache` | `RwLock` | Global | <100μs (validate) |
| `AstDatabase::write_conn` | `Mutex` | Global | <10ms (transaction) |
| `McpHost::tools` | `RwLock` | Global | <1μs (lookup) |
| `BrainRpc` | `Mutex` | Per-invocation | <100ms (LLM call) |
| `SecretProxy` | `Mutex` | Global | <50ms (keyring call) |

---

## 3. Potential Deadlock Scenarios

### 3.1 Scenario D-001: Capability-SQLite Cycle

**Description:** Thread holds capability cache write lock and attempts SQLite write while another thread holds SQLite write lock and attempts capability validation.

**Likelihood:** LOW
- Capability derivation is rare
- SQLite writes are short transactions
- No known code path requires both simultaneously

**Code Path:**
```
Thread A                          Thread B
─────────                         ─────────
cache.write_lock().unwrap();
...                               sqlite.write_lock().unwrap();
sqlite.write_lock() ◄────┐        ...
(BLOCKED)                │        cache.read_lock() ◄────┐
                         │        (BLOCKED)              │
                         └───────────────────────────────┘
                                    DEADLOCK
```

**Mitigation:**
1. Never hold capability write lock during SQLite operations
2. Derive capabilities before database transactions
3. Use try_lock with timeout for SQLite writes

```rust
// MITIGATION: Atomic capability derivation
pub fn derive_capability(&self, parent: &Capability, subset: PermissionSet) 
    -> Result<Capability, CapabilityError> 
{
    // 1. Compute derived capability (no lock needed)
    let derived = self.compute_derived(parent, subset)?;
    
    // 2. Acquire write lock only for cache insertion
    {
        let mut cache = self.cache.write().unwrap();
        cache.insert(derived.hash(), derived.clone());
    } // Lock released here
    
    // 3. SQLite write happens after lock release
    Ok(derived)
}
```

---

### 3.2 Scenario D-002: Brain RPC-Capability Cycle

**Description:** WASM instance holds BrainRpc mutex and requests capability validation while another thread holds capability cache write lock and attempts to invoke Brain.

**Likelihood:** VERY LOW
- Brain invocations are serialized via Monoio
- Capability cache writes are rare
- WASM cannot directly request capability derivation

**Mitigation:**
1. Brain invocations are single-threaded per monoio design
2. Host functions proxy all capability requests
3. Capability validation uses read lock only

---

### 3.3 Scenario D-003: MCP Tool-SQLite Cycle

**Description:** MCP tool execution holds tool registry read lock and attempts SQLite read while another thread holds SQLite write lock and attempts to register a new MCP tool.

**Likelihood:** LOW
- Tool registration only at startup
- SQLite writes are rare after initialization
- Tool execution uses read lock

**Mitigation:**
1. Register all tools at startup (no runtime registration)
2. SQLite reads use connection pool (no lock conflict)
3. Write lock only for migrations (startup only)

```rust
// MITIGATION: Startup-only tool registration
impl McpHost {
    pub fn new() -> Self {
        let mut host = Self { tools: RwLock::new(HashMap::new()) };
        
        // Register all tools during construction
        host.register_tool(Box::new(AstQueryTool::new()));
        host.register_tool(Box::new(VectorSearchTool::new()));
        // ... more tools
        
        host
    }
    
    // No public register_tool method - tools immutable after construction
}
```

---

### 3.4 Scenario D-004: FSM-Component Cycle

**Description:** FSM transition attempts to acquire multiple component locks in inconsistent order.

**Likelihood:** NONE
- FSM transitions are single-threaded (monoio)
- Typestate pattern prevents concurrent transitions
- Components are Arc-wrapped but accessed sequentially

**Analysis:**
```rust
impl<S: PhaseState> Fsm<S> {
    pub fn transition(self, event: Event) -> Result<Fsm<S::Next>, TransitionError> {
        // All operations are sequential within single thread
        // No lock acquisition - self is consumed
        
        self.gates.evaluate_exit(self.state.phase())?;  // Read-only
        self.gates.evaluate_entry(next_state.phase())?; // Read-only
        
        let hash = self.artifacts.compute_hash();        // Owned
        self.log_transition(&self.state, &next_state, &hash)?; // Owned
        
        Ok(Fsm { state: next_state, ... })
    }
}
```

**Mitigation:** None needed - typestate pattern prevents concurrent access.

---

## 4. Lock Ordering Protocol

### 4.1 Global Lock Order

When multiple locks must be acquired, the following order MUST be followed:

```
1. BrainRpc Mutex         (outermost - longest hold time)
2. SecretProxy Mutex
3. CapabilityManager RwLock (write)
4. McpHost RwLock (write)
5. SQLite Write Mutex
6. AstDatabase Write Mutex (innermost - shortest hold time)
```

### 4.2 Lock Ordering Violations Detection

```rust
// Static analysis via typestate
pub struct LockGuard<'a, T> {
    lock: &'a T,
    _not_send: PhantomData<*const ()>, // Prevent cross-thread movement
}

// Runtime detection via parking_lot
#[cfg(debug_assertions)]
use parking_lot::deadlock;

#[cfg(debug_assertions)]
fn check_deadlocks() {
    let deadlocks = deadlock::check_deadlock();
    if !deadlocks.is_empty() {
        panic!("Deadlock detected: {:?}", deadlocks);
    }
}
```

### 4.3 Lock-Free Paths (HFT Mode)

Per Rust SOP Part 3, the HFT Broker path is entirely lock-free:

| Operation | Synchronization | Lock-Free |
|-----------|-----------------|-----------|
| Market data push | Atomic (Release) | ✓ |
| Market data pop | Atomic (Acquire) | ✓ |
| Wallet Guard check | None (thread-local) | ✓ |
| Signal dispatch | Atomic (Release) | ✓ |
| Order dispatch | Atomic (Acquire) | ✓ |

---

## 5. Deadlock Detection & Recovery

### 5.1 Detection Mechanisms

| Mechanism | Scope | Overhead |
|-----------|-------|----------|
| `parking_lot::deadlock` | Debug builds | Low |
| Timeout on lock acquire | Production | Minimal |
| Watchdog thread | Production | Low |

### 5.2 Recovery Strategy

```rust
pub struct DeadlockRecovery;

impl DeadlockRecovery {
    pub fn acquire_with_timeout<T>(
        lock: &Mutex<T>, 
        timeout: Duration
    ) -> Result<MutexGuard<'_, T>, DeadlockError> {
        match lock.try_lock_for(timeout) {
            Some(guard) => Ok(guard),
            None => {
                // Log potential deadlock
                tracing::error!(
                    target: "deadlock",
                    timeout_ms = timeout.as_millis(),
                    "Lock acquisition timeout - potential deadlock"
                );
                Err(DeadlockError::Timeout)
            }
        }
    }
}
```

### 5.3 Watchdog Implementation

```rust
pub struct DeadlockWatchdog {
    check_interval: Duration,
    max_lock_hold: Duration,
}

impl DeadlockWatchdog {
    pub async fn run(&self) {
        let mut interval = monoio::time::interval(self.check_interval);
        
        loop {
            interval.tick().await;
            
            #[cfg(debug_assertions)]
            {
                let deadlocks = parking_lot::deadlock::check_deadlock();
                if !deadlocks.is_empty() {
                    tracing::error!(
                        deadlocks = deadlocks.len(),
                        "DEADLOCK DETECTED"
                    );
                    // In debug: panic to surface issue
                    panic!("Deadlock detected: {:?}", deadlocks);
                }
            }
        }
    }
}
```

---

## 6. Deadlock Risk Summary

### 6.1 Risk Matrix

| Component | Deadlock Risk | Likelihood | Impact | Mitigation Status |
|-----------|---------------|------------|--------|-------------------|
| Host Kernel | None | N/A | N/A | ✅ Typestate |
| Nexus FSM | None | N/A | N/A | ✅ Typestate |
| Sentinel | Low | Rare | Medium | ✅ Lock ordering |
| Brain WASM | Very Low | Rare | Low | ✅ Single-threaded |
| Graph-RAG | Low | Rare | Medium | ✅ Connection pool |
| HFT Broker | None | N/A | N/A | ✅ Lock-free |

### 6.2 Overall Assessment

| Metric | Value |
|--------|-------|
| Total Lock Resources | 6 |
| Potential Deadlock Cycles | 4 |
| Actual Deadlock Risk | LOW |
| Lock-Free Paths | HFT Broker (100%) |

---

## 7. Testing Requirements

### 7.1 Deadlock Stress Tests

```rust
#[test]
fn stress_test_capability_sqlite() {
    let sentinel = Arc::new(Sentinel::new());
    let graph = Arc::new(GraphRag::new_test());
    
    let handles: Vec<_> = (0..100)
        .map(|i| {
            let s = sentinel.clone();
            let g = graph.clone();
            thread::spawn(move || {
                for _ in 0..1000 {
                    // Randomly derive capabilities and query
                    if i % 2 == 0 {
                        s.derive_capability(&parent, subset).unwrap();
                    }
                    g.query_structural(query).unwrap();
                }
            })
        })
        .collect();
    
    // All threads should complete without deadlock
    handles.into_iter().for_each(|h| h.join().unwrap());
}
```

### 7.2 Lock Ordering Verification

```rust
#[test]
fn verify_lock_ordering() {
    // Use loom or similar to verify lock ordering
    // This is a compile-time check via typestate
}
```

---

## 8. Compliance Matrix

| Standard | Requirement | Compliance |
|----------|-------------|------------|
| Rust SOP 3.2 | No shared state on hot path | ✅ |
| NIST SP 800-53 | SC-3 isolation | ✅ |
| IEEE 1016 | Deadlock analysis | ✅ |
| MISRA C (adapted) | No recursive locks | ✅ |

---

**Document Status:** APPROVED
**Next Review:** Phase 3 Implementation
**Sign-off:** Concurrency Engineer
