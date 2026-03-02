---
id: CA-SYNC-001
title: "Synchronization Design"
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
  - BP-BRAIN-001
---

# Synchronization Design

## 1. Executive Summary

This document defines the synchronization primitives, channels, barriers, and atomic types for Clawdius. Given the monoio thread-per-core model, synchronization is minimized and message passing is preferred over shared-state concurrency.

## 2. Synchronization Strategy

### 2.1 Design Principles

| Principle | Rationale |
|-----------|-----------|
| Message passing over shared state | Per monoio model |
| Lock-free on hot paths | Per Rust SOP Part 3 |
| Minimal lock scope | Reduce contention |
| Single-writer pattern | Eliminate write conflicts |
| Typestate over runtime checks | Compile-time safety |

### 2.2 Synchronization by Component

| Component | Primary Mechanism | Secondary |
|-----------|-------------------|-----------|
| Host Kernel | Single-threaded | Arc for sharing |
| Nexus FSM | Typestate (compile-time) | None |
| Sentinel | RwLock | Channels for requests |
| Brain WASM | Mutex | RPC protocol |
| Graph-RAG | Connection pool | RwLock |
| HFT Broker | Lock-free atomics | Per-core isolation |

---

## 3. Channel Definitions

### 3.1 Channel Types

| Channel | Type | Use Case | Bounded |
|---------|------|----------|---------|
| Command Channel | mpsc | Host → Components | Yes (256) |
| Event Channel | mpsc | Components → Host | Yes (1024) |
| Market Data | spsc | Exchange → Strategy | Yes (2^20) |
| Signal Channel | spsc | Strategy → Dispatcher | Yes (4096) |
| Notification | broadcast | Dispatcher → Bridges | Yes (64) |
| Log Channel | mpsc | All → Logger | Yes (4096) |

### 3.2 Command Channel (mpsc)

```rust
use tokio::sync::mpsc;

pub enum Command {
    Transition { event: Event },
    SpawnSandbox { request: SpawnRequest },
    InvokeBrain { request: BrainRequest },
    QueryGraph { query: GraphQuery },
    Shutdown,
}

pub struct CommandChannel {
    tx: mpsc::Sender<Command>,
    rx: mpsc::Receiver<Command>,
}

impl CommandChannel {
    pub fn new(buffer_size: usize) -> Self {
        let (tx, rx) = mpsc::channel(buffer_size);
        Self { tx, rx }
    }
    
    pub fn sender(&self) -> mpsc::Sender<Command> {
        self.tx.clone()
    }
}
```

### 3.3 Market Data Channel (spsc)

```rust
use crossbeam::queue::ArrayQueue;

pub struct MarketDataChannel {
    buffer: Arc<ArrayQueue<MarketData>>,
}

impl MarketDataChannel {
    pub fn new(capacity: usize) -> (Producer, Consumer) {
        let buffer = Arc::new(ArrayQueue::new(capacity));
        (
            Producer { buffer: buffer.clone() },
            Consumer { buffer },
        )
    }
}

pub struct Producer {
    buffer: Arc<ArrayQueue<MarketData>>,
}

pub struct Consumer {
    buffer: Arc<ArrayQueue<MarketData>>,
}

impl Producer {
    #[inline]
    pub fn push(&self, data: MarketData) -> Result<(), MarketData> {
        self.buffer.push(data)
    }
}

impl Consumer {
    #[inline]
    pub fn pop(&self) -> Option<MarketData> {
        self.buffer.pop()
    }
}
```

### 3.4 Notification Channel (broadcast)

```rust
use tokio::sync::broadcast;

pub enum Notification {
    OrderFilled { order_id: OrderId, fill: Fill },
    RiskAlert { alert: RiskAlert },
    SystemAlert { level: AlertLevel, message: String },
}

pub struct NotificationBus {
    tx: broadcast::Sender<Notification>,
}

impl NotificationBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<Notification> {
        self.tx.subscribe()
    }
    
    pub fn broadcast(&self, notification: Notification) -> Result<usize, Notification> {
        self.tx.send(notification)
    }
}
```

---

## 4. Barrier and Latch Definitions

### 4.1 Startup Barrier

```rust
use std::sync::Barrier;

pub struct StartupBarrier {
    barrier: Barrier,
}

impl StartupBarrier {
    pub fn new(component_count: usize) -> Self {
        Self {
            barrier: Barrier::new(component_count),
        }
    }
    
    pub fn wait(&self) -> StartupBarrierWaitResult {
        self.barrier.wait();
        StartupBarrierWaitResult
    }
}

// Usage: All components wait until everyone is ready
pub async fn start_components(
    fsm: Arc<Fsm>,
    sentinel: Arc<Sentinel>,
    brain: Arc<BrainRpc>,
    graph: Arc<GraphRag>,
) {
    let barrier = Arc::new(StartupBarrier::new(4));
    
    let fsm_barrier = barrier.clone();
    let sentinel_barrier = barrier.clone();
    let brain_barrier = barrier.clone();
    let graph_barrier = barrier.clone();
    
    monoio::spawn(async move {
        fsm.initialize().await;
        fsm_barrier.wait();
    });
    
    monoio::spawn(async move {
        sentinel.initialize().await;
        sentinel_barrier.wait();
    });
    
    // ... other components
    
    barrier.wait();
}
```

### 4.2 Shutdown Latch

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct ShutdownLatch {
    is_shutdown: AtomicBool,
}

impl ShutdownLatch {
    pub fn new() -> Self {
        Self {
            is_shutdown: AtomicBool::new(false),
        }
    }
    
    pub fn shutdown(&self) {
        self.is_shutdown.store(true, Ordering::Release);
    }
    
    pub fn is_shutdown(&self) -> bool {
        self.is_shutdown.load(Ordering::Acquire)
    }
}

// Usage: Components check latch periodically
pub async fn run_event_loop(latch: Arc<ShutdownLatch>) {
    while !latch.is_shutdown() {
        // Process events
        if let Some(event) = poll_event().await {
            handle_event(event).await;
        }
    }
}
```

### 4.3 Phase Transition Gate

```rust
use std::sync::Condvar;

pub struct PhaseGate {
    current_phase: Mutex<Phase>,
    condvar: Condvar,
}

impl PhaseGate {
    pub fn new(initial: Phase) -> Self {
        Self {
            current_phase: Mutex::new(initial),
            condvar: Condvar::new(),
        }
    }
    
    pub fn transition(&self, new_phase: Phase) {
        let mut phase = self.current_phase.lock().unwrap();
        *phase = new_phase;
        self.condvar.notify_all();
    }
    
    pub fn wait_for(&self, target: Phase) {
        let mut phase = self.current_phase.lock().unwrap();
        while *phase != target {
            phase = self.condvar.wait(phase).unwrap();
        }
    }
    
    pub fn wait_while<F>(&self, predicate: F) 
    where 
        F: Fn(Phase) -> bool 
    {
        let mut phase = self.current_phase.lock().unwrap();
        while predicate(*phase) {
            phase = self.condvar.wait(phase).unwrap();
        }
    }
}
```

---

## 5. Atomic Types

### 5.1 Required Atomic Types

| Type | Location | Purpose |
|------|----------|---------|
| `AtomicU64` | RingBuffer::head/tail | Index tracking |
| `AtomicU32` | OrderDispatcher::seq | Order sequencing |
| `AtomicBool` | ShutdownLatch | Shutdown signal |
| `AtomicPtr<T>` | Arena allocator | Free list head |
| `AtomicUsize` | Metrics counters | Statistics |

### 5.2 Cache-Padded Atomics

```rust
use crossbeam_utils::CachePadded;

#[repr(align(64))]
pub struct CacheLineAligned<T>(pub T);

pub struct PaddedAtomicU64 {
    inner: CachePadded<AtomicU64>,
}

impl PaddedAtomicU64 {
    pub fn new(value: u64) -> Self {
        Self {
            inner: CachePadded::new(AtomicU64::new(value)),
        }
    }
    
    #[inline]
    pub fn load(&self, ordering: Ordering) -> u64 {
        self.inner.load(ordering)
    }
    
    #[inline]
    pub fn store(&self, value: u64, ordering: Ordering) {
        self.inner.store(value, ordering);
    }
}
```

### 5.3 Atomic Counter

```rust
pub struct AtomicCounter {
    value: AtomicU64,
}

impl AtomicCounter {
    pub fn new(initial: u64) -> Self {
        Self {
            value: AtomicU64::new(initial),
        }
    }
    
    #[inline]
    pub fn increment(&self) -> u64 {
        self.value.fetch_add(1, Ordering::Relaxed)
    }
    
    #[inline]
    pub fn decrement(&self) -> u64 {
        self.value.fetch_sub(1, Ordering::Relaxed)
    }
    
    #[inline]
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
    
    #[inline]
    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }
}
```

---

## 6. Lock Definitions

### 6.1 Mutex Usage

| Location | Lock Type | Hold Time | Contention |
|----------|-----------|-----------|------------|
| FsmCore | `parking_lot::Mutex` | <1ms | Low |
| BrainRpc | `parking_lot::Mutex` | <100ms | Low |
| SQLite write | `parking_lot::Mutex` | <10ms | Low |
| SecretProxy | `parking_lot::Mutex` | <50ms | Low |

### 6.2 RwLock Usage

| Location | Lock Type | Read:Write Ratio | Hold Time |
|----------|-----------|------------------|-----------|
| CapabilityManager | `parking_lot::RwLock` | 1000:1 | Read <1μs |
| McpHost::tools | `parking_lot::RwLock` | ∞:0 (immutable) | Read <1μs |

### 6.3 Lock Implementation

```rust
// Use parking_lot for better performance than std
use parking_lot::{Mutex, RwLock};

pub struct CapabilityManager {
    cache: RwLock<HashMap<TokenHash, Capability>>,
    signing_key: [u8; 32],
}

impl CapabilityManager {
    pub fn validate(&self, token: &[u8]) -> Result<Capability, ValidationError> {
        let hash = blake3::hash(token);
        
        // Read lock - allows concurrent readers
        let cache = self.cache.read();
        cache
            .get(hash.as_bytes())
            .cloned()
            .ok_or(ValidationError::InvalidToken)
    }
    
    pub fn insert(&self, capability: Capability) {
        let hash = capability.hash();
        
        // Write lock - exclusive access
        let mut cache = self.cache.write();
        cache.insert(hash, capability);
    }
}
```

---

## 7. Synchronization Patterns

### 7.1 Actor Pattern

```rust
pub struct ComponentActor {
    command_rx: mpsc::Receiver<Command>,
    response_tx: mpsc::Sender<Response>,
    state: ComponentState,
}

impl ComponentActor {
    pub async fn run(&mut self) {
        while let Some(command) = self.command_rx.recv().await {
            let response = self.handle_command(command).await;
            if self.response_tx.send(response).await.is_err() {
                break;
            }
        }
    }
    
    async fn handle_command(&mut self, command: Command) -> Response {
        // All state access is single-threaded
        match command {
            Command::Transition { event } => {
                self.state.transition(event)
            }
            // ...
        }
    }
}
```

### 7.2 Read-Copy-Update (RCU) Pattern

```rust
use std::sync::Arc;

pub struct Rcued<T> {
    current: AtomicPtr<Arc<T>>,
}

impl<T> Rcued<T> {
    pub fn new(initial: T) -> Self {
        let ptr = Box::into_raw(Box::new(Arc::new(initial)));
        Self {
            current: AtomicPtr::new(ptr),
        }
    }
    
    pub fn read(&self) -> Arc<T> {
        let ptr = self.current.load(Ordering::Acquire);
        unsafe { (*ptr).clone() }
    }
    
    pub fn update(&self, new_value: T) {
        let new_ptr = Box::into_raw(Box::new(Arc::new(new_value)));
        let old_ptr = self.current.swap(new_ptr, Ordering::AcqRel);
        
        // Delayed drop (simplified - real RCU needs epoch-based reclamation)
        // In production, use crossbeam-epoch (but banned in HFT mode per SOP)
        unsafe {
            drop(Box::from_raw(old_ptr));
        }
    }
}
```

### 7.3 Double-Checked Locking (Initialization Only)

```rust
use std::sync::Once;

pub struct LazyInit<T> {
    once: Once,
    value: UnsafeCell<Option<T>>,
}

unsafe impl<T: Send> Sync for LazyInit<T> {}

impl<T> LazyInit<T> {
    pub const fn new() -> Self {
        Self {
            once: Once::new(),
            value: UnsafeCell::new(None),
        }
    }
    
    pub fn get_or_init<F>(&self, f: F) -> &T 
    where 
        F: FnOnce() -> T 
    {
        self.once.call_once(|| {
            unsafe {
                *self.value.get() = Some(f());
            }
        });
        
        unsafe { (*self.value.get()).as_ref().unwrap() }
    }
}
```

---

## 8. Backpressure Implementation

### 8.1 Bounded Channels

```rust
// All channels are bounded to provide backpressure
pub fn create_command_channel() -> (mpsc::Sender<Command>, mpsc::Receiver<Command>) {
    mpsc::channel(256)  // Bounded to 256 pending commands
}
```

### 8.2 Load Shedding

```rust
use tower::load_shed::LoadShed;
use tower::limit::ConcurrencyLimit;

pub fn create_rate_limited_service<S>(service: S, max_concurrent: usize) 
    -> LoadShed<ConcurrencyLimit<S>> 
{
    let limited = ConcurrencyLimit::new(service, max_concurrent);
    LoadShed::new(limited)
}
```

---

## 9. Synchronization Checklist

| Item | Requirement | Status |
|------|-------------|--------|
| All channels bounded | Backpressure | ✅ |
| No unbounded queues | Memory safety | ✅ |
| Lock scope minimized | Contention | ✅ |
| CachePadded for hot atomics | False sharing | ✅ |
| Acquire/Release orderings | Correctness | ✅ |
| Shutdown coordination | Clean exit | ✅ |

---

## 10. Compliance Matrix

| Standard | Requirement | Compliance |
|----------|-------------|------------|
| Rust SOP 2.1 | Load shedding | ✅ |
| Rust SOP 2.1 | Bounded queues | ✅ |
| Rust SOP 3.2 | CachePadded | ✅ |
| Rust SOP 3.2 | Acquire/Release | ✅ |
| IEEE 1016 | Sync design | ✅ |

---

**Document Status:** APPROVED
**Next Review:** Phase 3 Implementation
**Sign-off:** Concurrency Engineer
