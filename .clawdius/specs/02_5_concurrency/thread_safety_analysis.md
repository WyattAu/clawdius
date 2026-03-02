---
id: CA-THREAD-001
title: "Thread Safety Analysis"
version: 1.0.0
phase: 2.5
status: APPROVED
created: 2026-03-01
author: Concurrency Engineer
classification: Concurrency Analysis
trace_to:
  - BP-HOST-KERNEL-001
  - BP-NEXUS-FSM-001
  - BP-SENTINEL-001
  - BP-BRAIN-001
  - BP-GRAPH-RAG-001
  - BP-HFT-BROKER-001
---

# Thread Safety Analysis

## 1. Executive Summary

This document analyzes thread safety requirements for each Clawdius component based on the monoio thread-per-core runtime model. Per Rust SOP Part 3.2, shared-state concurrency is prohibited on hot paths.

## 2. Component Thread Models

### 2.1 Host Kernel (COMP-HOST-001)

| Attribute | Value |
|-----------|-------|
| Thread Model | Single-threaded (monoio) |
| Shared State | FSM state via Arc<Mutex> |
| Send Bound | Required for components |
| Sync Bound | Required for HAL trait objects |

**Thread Safety Analysis:**

```rust
pub struct HostKernel {
    runtime: monoio::Runtime,        // !Send, !Sync (thread-local)
    state: KernelState,               // Owned, no sharing
    components: ComponentRegistry,    // Arc-wrapped for sharing
}

pub struct ComponentRegistry {
    fsm: Arc<Mutex<FsmCore>>,         // Shared with potential workers
    sentinel: Arc<Sentinel>,          // Read-only after init
    brain: Arc<Mutex<BrainRpc>>,      // WASM instance protection
    graph: Arc<GraphRag>,             // Internally synchronized
    hal: Box<dyn Hal>,                // Send + Sync required
}
```

**Send/Sync Requirements:**
- `Hal` trait: `Send + Sync` (may be called from any thread)
- `FsmCore`: `Send` (wrapped in Arc<Mutex>)
- `Sentinel`: `Send + Sync` (read-only capability cache)
- `BrainRpc`: `Send` (wrapped in Arc<Mutex>)
- `GraphRag`: `Send + Sync` (internal synchronization)

**Risk Level:** LOW
- monoio is single-threaded per core
- No cross-thread mutation of hot path data

---

### 2.2 Nexus FSM (COMP-FSM-001)

| Attribute | Value |
|-----------|-------|
| Thread Model | Single owner |
| Shared State | None (typestate pattern) |
| Send Bound | Required for phase types |
| Sync Bound | Not required |

**Thread Safety Analysis:**

```rust
pub struct Fsm<S: PhaseState> {
    state: S,                          // Consumed on transition
    artifacts: ArtifactRegistry,       // Owned
    gates: GateEvaluator,              // Owned
}
```

The typestate pattern consumes `self` on transition, making data races impossible:

```rust
impl<S: PhaseState> Fsm<S> {
    pub fn transition(self, event: Event) -> Result<Fsm<S::Next>, TransitionError> {
        // self is consumed - no aliasing possible
        let next_state = self.state.validate_transition(event)?;
        Ok(Fsm { state: next_state, ... })
    }
}
```

**Send/Sync Requirements:**
- All `PhaseState` types: `Send` (for potential thread handoff)
- `ArtifactRegistry`: `Send` (owned data)
- `GateEvaluator`: `Send + Sync` (read-only gates)

**Risk Level:** NONE
- Typestate pattern enforces single ownership at compile time
- No shared mutable state

---

### 2.3 Sentinel Sandbox (COMP-SENTINEL-001)

| Attribute | Value |
|-----------|-------|
| Thread Model | Message passing |
| Shared State | Capability cache |
| Send Bound | Required for all types |
| Sync Bound | Required for capability manager |

**Thread Safety Analysis:**

```rust
pub struct Sentinel {
    tier_selector: TierSelector,       // Stateless, Send + Sync
    spawner: SandboxSpawner,           // Platform-specific
    capabilities: CapabilityManager,   // Arc<RwLock> for cache
    validator: SettingsValidator,      // Stateless, Send + Sync
    secret_proxy: SecretProxy,         // Arc<Mutex> for keyring
}

pub struct CapabilityManager {
    cache: RwLock<HashMap<TokenHash, Capability>>,
    signing_key: [u8; 32],             // Read-only after init
}
```

**Capability Cache Synchronization:**

| Operation | Lock Type | Contention Risk |
|-----------|-----------|-----------------|
| Validate capability | Read | Low (read-heavy) |
| Derive capability | Write | Low (rare operation) |
| Cache lookup | Read | Low |

**Send/Sync Requirements:**
- `Capability`: `Send + Sync + Clone` (shared across sandboxes)
- `SandboxSpawner`: `Send + Sync` (platform backend trait)
- `SecretProxy`: `Send` (wrapped in Arc<Mutex>)

**Risk Level:** MEDIUM
- Capability cache requires RwLock
- Mitigation: Read-heavy workload, short lock holds

---

### 2.4 Brain WASM (COMP-BRAIN-001)

| Attribute | Value |
|-----------|-------|
| Thread Model | Isolated (WASM sandbox) |
| Shared State | None (RPC boundary) |
| Send Bound | Required for RPC types |
| Sync Bound | Not required for WASM |

**Thread Safety Analysis:**

```rust
pub struct BrainRpc {
    version: ProtocolVersion,          // Copy, no sharing
    wasm_store: Store<HostState>,      // !Send (wasmtime limitation)
    brain_instance: Instance,          // !Send (wasmtime limitation)
}
```

**WASM Runtime Constraints:**
- `wasmtime::Store` is `!Send` - must run on single thread
- `wasmtime::Instance` is `!Send` - bound to store
- Solution: Wrap in `Arc<Mutex<BrainRpc>>` if cross-thread access needed

**RPC Protocol Types:**

```rust
// All RPC types must be Send for channel transmission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainRequest {
    pub id: Uuid,                      // Send + Sync
    pub method: BrainMethod,           // Send + Sync (enum)
    pub params: serde_json::Value,     // Send + Sync
    pub capability: Capability,        // Send + Sync (validated above)
}
```

**Host Function Isolation:**

```rust
// Host functions execute in WASM thread context
#[wasmtime::witx::host_function]
pub fn host_llm_call(
    provider: Ptr<String>, 
    prompt: Ptr<String>
) -> Result<Ptr<String>, Error> {
    // Called from WASM thread - no synchronization needed
    // API key obtained from Host keyring (proxied)
}
```

**Risk Level:** LOW
- WASM provides memory isolation
- RPC boundary prevents shared state
- wasmtime guarantees sandbox security

---

### 2.5 Graph-RAG (COMP-GRAPH-001)

| Attribute | Value |
|-----------|-------|
| Thread Model | Read-heavy |
| Shared State | SQLite, LanceDB |
| Send Bound | Required for query types |
| Sync Bound | Required for store handles |

**Thread Safety Analysis:**

```rust
pub struct GraphRag {
    ast_db: AstDatabase,               // rusqlite::Connection (Send but not Sync)
    vector_store: VectorStore,         // lancedb::Connection (Send + Sync)
    parser_pipeline: ParserPipeline,   // Stateless workers
    mcp_host: McpHost,                 // Arc<RwLock> for tool registry
}

pub struct AstDatabase {
    // rusqlite::Connection is Send but NOT Sync
    // Solution: One connection per thread or connection pool
    read_pool: r2d2::Pool<SqliteConnectionManager>,
    write_conn: Mutex<SqliteConnection>,
}
```

**SQLite Concurrency Model:**

| Operation | Strategy | Notes |
|-----------|----------|-------|
| Read query | Connection pool | Multiple readers |
| Write transaction | Single writer | Serialized via Mutex |
| Schema migration | Exclusive lock | Startup only |

**LanceDB Concurrency:**
- Native `Send + Sync` support
- MVCC for concurrent reads
- Writes serialized internally

**MCP Host Synchronization:**

```rust
pub struct McpHost {
    tools: RwLock<HashMap<String, Box<dyn McpTool>>>,
    // Tool registration: Write lock (rare)
    // Tool execution: Read lock (common)
}
```

**Risk Level:** MEDIUM
- SQLite connection management required
- Mitigation: Connection pooling, short write transactions

---

### 2.6 HFT Broker (COMP-BROKER-001)

| Attribute | Value |
|-----------|-------|
| Thread Model | Thread-per-core |
| Shared State | Ring buffer (lock-free SPSC) |
| Send Bound | Required for market data |
| Sync Bound | NOT required (per-core isolation) |

**Thread Safety Analysis:**

```rust
pub struct RingBuffer<T: Copy> {
    buffer: Box<[CachePadded<T>]>,     // Cache-line aligned
    capacity: usize,
    head: CachePadded<AtomicU64>,      // Producer-only write
    tail: CachePadded<AtomicU64>,      // Consumer-only write
}

// Per Rust SOP Part 3.2:
// - CachePadded prevents false sharing
// - Acquire/Release orderings (not SeqCst)
// - crossbeam-epoch GC banned
```

**Memory Ordering (Per Rust SOP):**

```rust
// Producer (write side)
pub fn push(&self, item: T) -> Result<(), RingBufferError> {
    let head = self.head.load(Ordering::Relaxed);  // Only we write head
    let next_head = (head + 1) % self.capacity as u64;
    
    // Check against consumer's tail
    if next_head == self.tail.load(Ordering::Acquire) {
        return Err(RingBufferError::Full);
    }
    
    unsafe { *self.buffer.as_ptr().add(head as usize).get() = item; }
    
    // Release ensures write is visible before head update
    self.head.store(next_head, Ordering::Release);
    Ok(())
}

// Consumer (read side)
pub fn pop(&self) -> Option<T> {
    let tail = self.tail.load(Ordering::Relaxed);  // Only we write tail
    
    // Check against producer's head
    if tail == self.head.load(Ordering::Acquire) {
        return None;
    }
    
    let item = unsafe { *self.buffer.as_ptr().add(tail as usize).get() };
    
    // Release ensures read completes before tail update
    self.tail.store((tail + 1) % self.capacity as u64, Ordering::Release);
    Some(item)
}
```

**Wallet Guard Isolation:**

```rust
// Wallet Guard runs on dedicated core (no sharing)
pub struct WalletGuard {
    wallet: Wallet,                    // Thread-local
    params: RiskParameters,            // Read-only
}
// No synchronization needed - single owner per core
```

**Thread Affinity (Per Rust SOP Part 3.1):**

| Thread | Core | Isolation |
|--------|------|-----------|
| Market Data | 0 | isolcpus=0 |
| Strategy | 1 | isolcpus=1 |
| Risk (Wallet Guard) | 2 | isolcpus=2 |
| Order Dispatch | 3 | isolcpus=3 |

**Risk Level:** LOW (with proper isolation)
- Lock-free SPSC eliminates mutex contention
- Cache padding prevents false sharing
- Per-core memory regions (no sharing)

---

## 3. Send/Sync Bound Summary

### 3.1 Required Trait Implementations

| Type | Send | Sync | Justification |
|------|------|------|---------------|
| `Phase` | ✓ | ✓ | Copy enum |
| `Event` | ✓ | ✓ | Copy enum |
| `Capability` | ✓ | ✓ | Shared across sandboxes |
| `Permission` | ✓ | ✓ | Bitflags |
| `SandboxTier` | ✓ | ✓ | Copy enum |
| `BrainRequest` | ✓ | ✓ | Channel transmission |
| `BrainResponse` | ✓ | ✓ | Channel transmission |
| `MarketData` | ✓ | - | SPSC only |
| `Signal` | ✓ | ✓ | Cross-thread dispatch |
| `Hal` (trait) | ✓ | ✓ | Cross-platform |
| `SandboxBackend` (trait) | ✓ | ✓ | Platform backends |
| `McpTool` (trait) | ✓ | ✓ | Tool registry |

### 3.2 Types Requiring Synchronization

| Type | Synchronization | Pattern |
|------|-----------------|---------|
| `FsmCore` | `Arc<Mutex<_>>` | Rare transitions |
| `BrainRpc` | `Arc<Mutex<_>>` | WASM instance |
| `CapabilityManager::cache` | `RwLock<_>` | Read-heavy |
| `AstDatabase::write_conn` | `Mutex<_>` | Write serialization |
| `McpHost::tools` | `RwLock<_>` | Read-heavy |

### 3.3 Types Explicitly !Send/!Sync

| Type | Reason | Mitigation |
|------|--------|------------|
| `monoio::Runtime` | Thread-local | One per thread |
| `wasmtime::Store` | Thread-local | Wrap in Mutex |
| `wasmtime::Instance` | Bound to store | Wrap in Mutex |
| `rusqlite::Connection` | Not thread-safe | Connection pool |

---

## 4. Thread Safety Checklist

### 4.1 Per-Component Verification

| Component | Checklist Item | Status |
|-----------|----------------|--------|
| Host Kernel | All Arc<Mutex> bounds satisfied | ✅ |
| Host Kernel | Hal trait is Send + Sync | ✅ |
| Nexus FSM | PhaseState types are Send | ✅ |
| Nexus FSM | Typestate prevents aliasing | ✅ |
| Sentinel | Capability cache RwLock tested | ⏳ |
| Sentinel | SandboxBackend trait is Send + Sync | ✅ |
| Brain | RPC types are Send + Sync | ✅ |
| Brain | WASM instance Mutex protected | ✅ |
| Graph-RAG | SQLite pool configured | ⏳ |
| Graph-RAG | MCP tools RwLock tested | ⏳ |
| HFT Broker | Ring buffer is lock-free SPSC | ✅ |
| HFT Broker | CachePadded applied | ✅ |
| HFT Broker | Acquire/Release orderings | ✅ |

### 4.2 Rust SOP Compliance (Part 3.2)

| SOP Requirement | Implementation | Status |
|-----------------|----------------|--------|
| CachePadded for producer/consumer | Ring buffer head/tail | ✅ |
| Acquire/Release over SeqCst | Ring buffer atomics | ✅ |
| crossbeam-epoch GC banned | No epoch-based reclamation | ✅ |
| HugePage mmap | Ring buffer allocation | ⏳ |
| False-sharing elimination | CachePadded everywhere | ✅ |

---

## 5. Testing Requirements

### 5.1 Concurrency Tests

```rust
// Thread safety test for capability cache
#[test]
fn capability_cache_thread_safety() {
    let manager = Arc::new(CapabilityManager::new());
    
    let readers: Vec<_> = (0..10)
        .map(|_| {
            let m = manager.clone();
            thread::spawn(move || {
                for _ in 0..1000 {
                    m.validate(&sample_capability()).unwrap();
                }
            })
        })
        .collect();
    
    let writer = thread::spawn(|| {
        for _ in 0..100 {
            manager.derive(&parent, subset).unwrap();
            thread::sleep(Duration::from_micros(10));
        }
    });
    
    readers.into_iter().for_each(|h| h.join().unwrap());
    writer.join().unwrap();
}
```

### 5.2 Lock-free Verification

```rust
// Ring buffer stress test
#[test]
fn ring_buffer_lock_free_stress() {
    let buffer = Arc::new(RingBuffer::new(1024));
    
    let producer = thread::spawn({
        let b = buffer.clone();
        move || {
            for i in 0u64..1_000_000 {
                while b.push(MarketData { sequence: i, .. }).is_err() {}
            }
        }
    });
    
    let consumer = thread::spawn({
        let b = buffer.clone();
        move || {
            let mut last = 0u64;
            while last < 999_999 {
                if let Some(data) = b.pop() {
                    assert_eq!(data.sequence, last);
                    last += 1;
                }
            }
        }
    });
    
    producer.join().unwrap();
    consumer.join().unwrap();
}
```

---

## 6. Compliance Matrix

| Standard | Requirement | Compliance |
|----------|-------------|------------|
| Rust SOP 3.1 | Thread isolation | ✅ |
| Rust SOP 3.2 | CachePadded | ✅ |
| Rust SOP 3.2 | Acquire/Release | ✅ |
| Rust SOP 3.2 | No crossbeam-epoch | ✅ |
| IEEE 1016 | Thread model documented | ✅ |
| NIST SP 800-53 | SC-3 isolation | ✅ |

---

**Document Status:** APPROVED
**Next Review:** Phase 3 Implementation
**Sign-off:** Concurrency Engineer
