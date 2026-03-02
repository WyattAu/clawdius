---
id: RM-THREAD-001
title: "Thread Pool Analysis"
version: 1.0.0
phase: 3.5
status: APPROVED
created: 2026-03-01
author: Resource Engineer
classification: Resource Management Analysis
trace_to:
  - BP-HOST-KERNEL-001
  - BP-HFT-BROKER-001
  - rust_sop.md (Part 3.1)
---

# Thread Pool Analysis

## 1. Executive Summary

This document specifies the thread-per-core architecture using monoio runtime, blocking task isolation, and CPU affinity requirements for Clawdius. Per Rust SOP Part 3.1, threads must be pinned to isolated cores for HFT mode to eliminate scheduler jitter.

## 2. Runtime Architecture

### 2.1 monoio Thread-Per-Core Model

monoio uses a thread-per-core architecture instead of work-stealing, providing deterministic latency for HFT workloads.

```
┌─────────────────────────────────────────────────────────────────┐
│                    monoio Runtime Architecture                   │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│   ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌───────────┐│
│   │   Core 0    │ │   Core 1    │ │   Core 2    │ │   Core 3  ││
│   │             │ │             │ │             │ │           ││
│   │ ┌─────────┐ │ │ ┌─────────┐ │ │ ┌─────────┐ │ │ ┌───────┐ ││
│   │ │ monoio  │ │ │ │ monoio  │ │ │ │ monoio  │ │ │ │monoio │ ││
│   │ │ Runtime │ │ │ │ Runtime │ │ │ │ Runtime │ │ │ │Runtime│ ││
│   │ └────┬────┘ │ │ └────┬────┘ │ │ └────┬────┘ │ │ └───┬───┘ ││
│   │      │      │ │      │      │ │      │      │ │     │     ││
│   │  ┌───┴───┐  │ │  ┌───┴───┐  │ │  ┌───┴───┐  │ │ ┌───┴───┐ ││
│   │  │  Run  │  │ │  │  Run  │  │ │  │  Run  │  │ │ │  Run  │ ││
│   │  │ Queue │  │ │  │ Queue │  │ │  │ Queue │  │ │ │ Queue │ ││
│   │  └───────┘  │ │  └───────┘  │ │  └───────┘  │ │ └───────┘ ││
│   └─────────────┘ └─────────────┘ └─────────────┘ └───────────┘│
│                                                                  │
│   No work-stealing between cores                                 │
│   Each core has its own run queue                                │
│   Tasks remain on the same core for cache locality              │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Runtime Comparison

| Feature | monoio | tokio | glommio |
|---------|--------|-------|---------|
| Threading | Thread-per-core | Work-stealing | Thread-per-core |
| I/O driver | io_uring | epoll | io_uring |
| Scheduler | Single queue | Multi-queue | Single queue |
| Latency | Deterministic | Variable | Deterministic |
| Best for | HFT, API gateway | General purpose | High I/O |

---

## 3. Thread Configuration

### 3.1 Standard Mode Threads

```rust
pub struct StandardThreadConfig {
    pub monoio_workers: usize,
    pub blocking_threads: usize,
    pub max_blocking_threads: usize,
}

impl Default for StandardThreadConfig {
    fn default() -> Self {
        let num_cpus = num_cpus::get();
        Self {
            monoio_workers: num_cpus.min(4),
            blocking_threads: 4,
            max_blocking_threads: 16,
        }
    }
}
```

### 3.2 HFT Mode Threads

```rust
pub struct HftThreadConfig {
    pub isolated_cores: Vec<usize>,
    pub market_data_core: usize,
    pub strategy_core: usize,
    pub risk_core: usize,
    pub order_core: usize,
    pub control_cores: Vec<usize>,
}

impl HftThreadConfig {
    pub fn from_isolated_cores(cores: Vec<usize>) -> Self {
        assert!(cores.len() >= 4, "Need at least 4 isolated cores for HFT");
        
        Self {
            isolated_cores: cores.clone(),
            market_data_core: cores[0],
            strategy_core: cores[1],
            risk_core: cores[2],
            order_core: cores[3],
            control_cores: cores[4..].to_vec(),
        }
    }
}
```

### 3.3 Thread Roles

| Thread | Core | Role | Priority |
|--------|------|------|----------|
| monoio-worker-0 | 0 | General async tasks | Normal |
| monoio-worker-1 | 1 | General async tasks | Normal |
| monoio-worker-2 | 2 | General async tasks | Normal |
| monoio-worker-3 | 3 | General async tasks | Normal |
| blocking-0 | Any | Blocking I/O | Normal |
| hft-market | Isolated 0 | Market data ingestion | RT |
| hft-strategy | Isolated 1 | Signal generation | RT |
| hft-risk | Isolated 2 | Wallet Guard | RT |
| hft-order | Isolated 3 | Order dispatch | RT |

---

## 4. CPU Affinity Configuration

### 4.1 Thread Pinning (Per SOP 3.1)

```rust
use core_affinity::CoreId;

pub fn pin_to_core(core_id: usize) -> Result<(), AffinityError> {
    let core_ids = core_affinity::get_core_ids()
        .ok_or(AffinityError::DetectionFailed)?;
    
    if core_id >= core_ids.len() {
        return Err(AffinityError::InvalidCore { 
            requested: core_id, 
            available: core_ids.len() 
        });
    }
    
    if !core_affinity::set_for_current(core_ids[core_id]) {
        return Err(AffinityError::SetFailed);
    }
    
    #[cfg(target_os = "linux")]
    set_thread_name(&format!("clawdius-{}", core_id))?;
    
    Ok(())
}

#[cfg(target_os = "linux")]
fn set_thread_name(name: &str) -> Result<(), AffinityError> {
    let cname = std::ffi::CString::new(name)
        .map_err(|_| AffinityError::InvalidName)?;
    
    unsafe {
        if libc::pthread_setname_np(libc::pthread_self(), cname.as_ptr()) != 0 {
            return Err(AffinityError::SetNameFailed);
        }
    }
    
    Ok(())
}
```

### 4.2 HFT Core Isolation

```rust
pub struct HftCoreManager {
    config: HftThreadConfig,
    handles: Vec<std::thread::JoinHandle<()>>,
}

impl HftCoreManager {
    pub fn new(config: HftThreadConfig) -> Self {
        Self {
            config,
            handles: Vec::new(),
        }
    }
    
    pub fn spawn_market_data_thread<F>(&mut self, f: F) -> Result<(), ThreadError>
    where
        F: FnOnce() + Send + 'static,
    {
        let core = self.config.market_data_core;
        
        let handle = std::thread::Builder::new()
            .name(format!("hft-market-{}", core))
            .spawn(move || {
                pin_to_core(core).expect("Failed to pin market data thread");
                
                // Per SOP 3.1: Set real-time priority
                #[cfg(target_os = "linux")]
                set_realtime_priority(99).expect("Failed to set RT priority");
                
                f()
            })?;
        
        self.handles.push(handle);
        Ok(())
    }
    
    pub fn spawn_strategy_thread<F>(&mut self, f: F) -> Result<(), ThreadError>
    where
        F: FnOnce() + Send + 'static,
    {
        let core = self.config.strategy_core;
        
        let handle = std::thread::Builder::new()
            .name(format!("hft-strategy-{}", core))
            .spawn(move || {
                pin_to_core(core).expect("Failed to pin strategy thread");
                
                #[cfg(target_os = "linux")]
                set_realtime_priority(90).expect("Failed to set RT priority");
                
                f()
            })?;
        
        self.handles.push(handle);
        Ok(())
    }
    
    pub fn spawn_risk_thread<F>(&mut self, f: F) -> Result<(), ThreadError>
    where
        F: FnOnce() + Send + 'static,
    {
        let core = self.config.risk_core;
        
        let handle = std::thread::Builder::new()
            .name(format!("hft-risk-{}", core))
            .spawn(move || {
                pin_to_core(core).expect("Failed to pin risk thread");
                
                #[cfg(target_os = "linux")]
                set_realtime_priority(95).expect("Failed to set RT priority");
                
                f()
            })?;
        
        self.handles.push(handle);
        Ok(())
    }
    
    pub fn spawn_order_thread<F>(&mut self, f: F) -> Result<(), ThreadError>
    where
        F: FnOnce() + Send + 'static,
    {
        let core = self.config.order_core;
        
        let handle = std::thread::Builder::new()
            .name(format!("hft-order-{}", core))
            .spawn(move || {
                pin_to_core(core).expect("Failed to pin order thread");
                
                #[cfg(target_os = "linux")]
                set_realtime_priority(85).expect("Failed to set RT priority");
                
                f()
            })?;
        
        self.handles.push(handle);
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn set_realtime_priority(priority: i32) -> Result<(), ThreadError> {
    unsafe {
        let mut attr: libc::sched_param = std::mem::zeroed();
        attr.sched_priority = priority;
        
        if libc::sched_setscheduler(0, libc::SCHED_FIFO, &attr) != 0 {
            return Err(ThreadError::PrioritySetFailed);
        }
    }
    
    Ok(())
}
```

### 4.3 GRUB Configuration (Per SOP 3.1)

```bash
# /etc/default/grub
# CPU isolation for HFT mode
GRUB_CMDLINE_LINUX="\
    isolcpus=0-3 \
    nohz_full=0-3 \
    rcu_nocbs=0-3 \
    irqaffinity=4-7 \
    intel_idle.max_cstate=0 \
    processor.max_cstate=0 \
    idle=poll \
    mce=off \
    nmi_watchdog=0 \
    nosoftlockup"

# Apply with: sudo update-grub && sudo reboot
```

---

## 5. Blocking Task Isolation

### 5.1 Blocking Thread Pool

```rust
use std::sync::Arc;
use std::thread::{self, JoinHandle};

pub struct BlockingPool {
    threads: Mutex<Vec<JoinHandle<()>>>,
    sender: crossbeam_channel::Sender<BlockingTask>,
    config: BlockingPoolConfig,
}

struct BlockingTask {
    task: Box<dyn FnOnce() + Send>,
    completion: crossbeam_channel::Sender<()>,
}

#[derive(Debug, Clone)]
pub struct BlockingPoolConfig {
    pub min_threads: usize,
    pub max_threads: usize,
    pub idle_timeout: Duration,
    pub thread_stack_size: usize,
}

impl Default for BlockingPoolConfig {
    fn default() -> Self {
        Self {
            min_threads: 4,
            max_threads: 32,
            idle_timeout: Duration::from_secs(60),
            thread_stack_size: 512 * 1024,
        }
    }
}

impl BlockingPool {
    pub fn new(config: BlockingPoolConfig) -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        
        let pool = Self {
            threads: Mutex::new(Vec::new()),
            sender,
            config,
        };
        
        // Start minimum threads
        for _ in 0..pool.config.min_threads {
            pool.spawn_thread(receiver.clone());
        }
        
        pool
    }
    
    fn spawn_thread(&self, receiver: crossbeam_channel::Receiver<BlockingTask>) {
        let idle_timeout = self.config.idle_timeout;
        
        let handle = thread::Builder::new()
            .stack_size(self.config.thread_stack_size)
            .name("blocking-worker".to_string())
            .spawn(move || {
                loop {
                    match receiver.recv_timeout(idle_timeout) {
                        Ok(task) => {
                            (task.task)();
                            let _ = task.completion.send(());
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                            // Idle timeout, check if we should exit
                            continue;
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                            break;
                        }
                    }
                }
            })
            .expect("Failed to spawn blocking thread");
        
        self.threads.lock().unwrap().push(handle);
    }
    
    pub fn spawn<F>(&self, f: F) -> BlockingHandle
    where
        F: FnOnce() + Send + 'static,
    {
        let (completion_tx, completion_rx) = crossbeam_channel::bounded(1);
        
        let task = BlockingTask {
            task: Box::new(f),
            completion: completion_tx,
        };
        
        self.sender.send(task).expect("Blocking pool disconnected");
        
        BlockingHandle { completion: completion_rx }
    }
    
    pub fn status(&self) -> BlockingPoolStatus {
        BlockingPoolStatus {
            active_threads: self.threads.lock().unwrap().len(),
            pending_tasks: self.sender.len(),
        }
    }
}

pub struct BlockingHandle {
    completion: crossbeam_channel::Receiver<()>,
}

impl BlockingHandle {
    pub fn wait(self) {
        let _ = self.completion.recv();
    }
    
    pub fn try_wait(&self) -> bool {
        self.completion.try_recv().is_ok()
    }
}

#[derive(Debug, Clone)]
pub struct BlockingPoolStatus {
    pub active_threads: usize,
    pub pending_tasks: usize,
}
```

### 5.2 Blocking Operation Categories

| Category | Operations | Pool | Timeout |
|----------|------------|------|---------|
| File I/O | Read, write, sync | Blocking | 30s |
| Database | SQLite queries | Blocking | 5s |
| Network | DNS, connect | Blocking | 10s |
| Sandbox | Process spawn | Blocking | 30s |
| WASM | Compilation | Blocking | 60s |

### 5.3 Blocking Task Wrapper

```rust
impl BlockingPool {
    pub fn spawn_with_timeout<F>(
        &self,
        f: F,
        timeout: Duration,
    ) -> Result<(), BlockingError>
    where
        F: FnOnce() + Send + 'static,
    {
        let handle = self.spawn(f);
        
        match handle.completion.recv_timeout(timeout) {
            Ok(()) => Ok(()),
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                Err(BlockingError::Timeout)
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                Err(BlockingError::Disconnected)
            }
        }
    }
}
```

---

## 6. monoio Integration

### 6.1 Runtime Builder

```rust
use monoio::RuntimeBuilder;

pub fn build_runtime(config: &StandardThreadConfig) -> Result<monoio::Runtime, RuntimeError> {
    let mut builder = RuntimeBuilder::new()
        .with_entries(1024);
    
    #[cfg(target_os = "linux")]
    {
        builder = builder.enable_io_uring();
    }
    
    let runtime = builder.build()?;
    
    Ok(runtime)
}

pub fn build_hft_runtime(core: usize) -> Result<monoio::Runtime, RuntimeError> {
    pin_to_core(core)?;
    
    let mut builder = RuntimeBuilder::new()
        .with_entries(4096);
    
    #[cfg(target_os = "linux")]
    {
        builder = builder.enable_io_uring();
    }
    
    let runtime = builder.build()?;
    
    Ok(runtime)
}
```

### 6.2 Task Spawning Patterns

```rust
pub struct TaskSpawner {
    runtime: monoio::Runtime,
}

impl TaskSpawner {
    pub fn spawn<F, T>(&self, task: F) -> JoinHandle<T>
    where
        F: Future<Output = T> + 'static,
        T: 'static,
    {
        self.runtime.spawn(task)
    }
    
    pub fn spawn_local<F, T>(&self, task: F) -> LocalJoinHandle<T>
    where
        F: Future<Output = T> + 'static,
        T: 'static,
    {
        monoio::spawn_local(task)
    }
    
    pub fn block_on<F, T>(&self, future: F) -> T
    where
        F: Future<Output = T>,
    {
        self.runtime.block_on(future)
    }
}
```

---

## 7. Thread Metrics

### 7.1 Thread Statistics

```rust
pub struct ThreadMetrics {
    pub id: usize,
    pub name: String,
    pub core: Option<usize>,
    pub priority: i32,
    pub state: ThreadState,
    pub cpu_time: Duration,
    pub context_switches: u64,
    pub minor_faults: u64,
    pub major_faults: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum ThreadState {
    Running,
    Runnable,
    Blocked,
    Idle,
    Unknown,
}

pub fn collect_thread_metrics() -> Vec<ThreadMetrics> {
    #[cfg(target_os = "linux")]
    {
        collect_linux_thread_metrics()
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        vec![]
    }
}

#[cfg(target_os = "linux")]
fn collect_linux_thread_metrics() -> Vec<ThreadMetrics> {
    let mut metrics = Vec::new();
    
    let dir = match std::fs::read_dir("/proc/self/task") {
        Ok(d) => d,
        Err(_) => return metrics,
    };
    
    for entry in dir.flatten() {
        let tid = entry.file_name().to_string_lossy().to_string();
        if let Ok(tid) = tid.parse::<u64>() {
            if let Some(m) = read_thread_stats(tid) {
                metrics.push(m);
            }
        }
    }
    
    metrics
}

#[cfg(target_os = "linux")]
fn read_thread_stats(tid: u64) -> Option<ThreadMetrics> {
    let stat_path = format!("/proc/self/task/{}/stat", tid);
    let stat = std::fs::read_to_string(&stat_path).ok()?;
    
    let comm_path = format!("/proc/self/task/{}/comm", tid);
    let name = std::fs::read_to_string(&comm_path)
        .ok()?
        .trim()
        .to_string();
    
    let fields: Vec<&str> = stat.split_whitespace().collect();
    
    Some(ThreadMetrics {
        id: tid as usize,
        name,
        core: None,
        priority: fields.get(17).and_then(|s| s.parse().ok()).unwrap_or(0),
        state: parse_thread_state(fields.get(2).unwrap_or(&"?")),
        cpu_time: Duration::ZERO,
        context_switches: 0,
        minor_faults: fields.get(9).and_then(|s| s.parse().ok()).unwrap_or(0),
        major_faults: fields.get(11).and_then(|s| s.parse().ok()).unwrap_or(0),
    })
}

#[cfg(target_os = "linux")]
fn parse_thread_state(s: &str) -> ThreadState {
    match s {
        "R" => ThreadState::Running,
        "S" => ThreadState::Runnable,
        "D" | "T" => ThreadState::Blocked,
        "I" => ThreadState::Idle,
        _ => ThreadState::Unknown,
    }
}
```

### 7.2 Runtime Metrics

```rust
pub struct RuntimeMetrics {
    pub num_workers: usize,
    pub active_tasks: u64,
    pub pending_tasks: u64,
    pub total_tasks_spawned: u64,
    pub io_uring_entries: u32,
    pub io_uring_sqes_submitted: u64,
    pub io_uring_cqes_completed: u64,
}

impl RuntimeMetrics {
    pub fn collect(runtime: &monoio::Runtime) -> Self {
        Self {
            num_workers: 1, // monoio is single-threaded per runtime
            active_tasks: 0,
            pending_tasks: 0,
            total_tasks_spawned: 0,
            io_uring_entries: 0,
            io_uring_sqes_submitted: 0,
            io_uring_cqes_completed: 0,
        }
    }
}
```

---

## 8. Thread Safety Analysis

### 8.1 Thread Safety by Component

| Component | Thread Model | Synchronization |
|-----------|--------------|-----------------|
| Host Kernel | monoio | None (single-threaded) |
| Nexus FSM | monoio | None (typestate) |
| Sentinel | Message passing | RwLock for cache |
| Brain | monoio + Mutex | Arc<Mutex<BrainRpc>> |
| Graph-RAG | monoio + Pool | Connection pool |
| HFT Broker | Isolated threads | Lock-free SPSC |

### 8.2 Lock-Free Guarantees (HFT)

Per Rust SOP Part 3.2, the HFT hot path uses no locks:

```rust
// HFT Thread Communication - Lock-Free
pub struct HftThreadChannels {
    market_to_strategy: RingBuffer<MarketData>,  // SPSC
    strategy_to_risk: RingBuffer<Signal>,        // SPSC
    risk_to_order: RingBuffer<Order>,            // SPSC
}

// No Mutex, RwLock, or other blocking synchronization
// Only AtomicU64 with Acquire/Release ordering
```

---

## 9. Thread Lifecycle

### 9.1 Startup Sequence

```
┌─────────────────────────────────────────────────────────────────┐
│                    Thread Startup Sequence                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. Main Thread                                                  │
│     └── Parse config, detect platform                           │
│                                                                  │
│  2. Create monoio Runtime(s)                                    │
│     └── One per worker core                                     │
│                                                                  │
│  3. Start Blocking Pool                                         │
│     └── Min 4 threads                                           │
│                                                                  │
│  4. [HFT Only] Pin isolated cores                               │
│     ├── Market data thread (core 0)                             │
│     ├── Strategy thread (core 1)                                │
│     ├── Risk thread (core 2)                                    │
│     └── Order thread (core 3)                                   │
│                                                                  │
│  5. Start component actors                                      │
│     ├── FSM actor                                               │
│     ├── Sentinel actor                                          │
│     ├── Brain actor                                             │
│     └── Graph-RAG actor                                         │
│                                                                  │
│  6. Enter main event loop                                       │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 9.2 Shutdown Sequence

```
┌─────────────────────────────────────────────────────────────────┐
│                    Thread Shutdown Sequence                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. Receive shutdown signal (SIGTERM, SIGINT)                   │
│                                                                  │
│  2. Set shutdown flag                                           │
│     └── AtomicBool::store(SeqCst)                               │
│                                                                  │
│  3. Stop accepting new tasks                                    │
│     └── Drop task spawner                                       │
│                                                                  │
│  4. Drain pending tasks                                         │
│     └── Timeout: 30 seconds                                     │
│                                                                  │
│  5. Stop HFT threads (if active)                                │
│     └── Wait for in-flight orders                               │
│                                                                  │
│  6. Drop blocking pool                                          │
│     └── Wait for idle threads to exit                           │
│                                                                  │
│  7. Drop monoio runtime                                         │
│     └── All tasks completed                                     │
│                                                                  │
│  8. Exit main thread                                            │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 10. Compliance Matrix

### 10.1 Rust SOP Compliance (Part 3.1)

| SOP Requirement | Implementation | Status |
|-----------------|----------------|--------|
| Thread isolation (isolcpus) | GRUB config | ✅ |
| core_affinity2 | core_affinity crate | ✅ |
| nohz_full | GRUB config | ✅ |
| rcu_nocbs | GRUB config | ✅ |
| irqaffinity | GRUB config | ✅ |
| intel_idle.max_cstate=0 | GRUB config | ✅ |
| Thread pinning | pin_to_core() | ✅ |
| RT priority | sched_setscheduler | ✅ |

### 10.2 Thread Count Budget

| Category | Count | Rationale |
|----------|-------|-----------|
| monoio workers | 4 | Thread-per-core |
| Blocking pool min | 4 | I/O operations |
| Blocking pool max | 32 | Burst handling |
| HFT isolated | 4 | Deterministic latency |
| **Total (standard)** | 8-40 | Dynamic |
| **Total (HFT)** | 12-44 | Isolated + blocking |

---

**Document Status:** APPROVED
**Next Review:** Phase 4 Implementation
**Sign-off:** Resource Engineer
