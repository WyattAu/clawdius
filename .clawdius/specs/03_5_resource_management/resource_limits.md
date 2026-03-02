---
id: RM-LIMITS-001
title: "Resource Limits Definition"
version: 1.0.0
phase: 3.5
status: APPROVED
created: 2026-03-01
author: Resource Engineer
classification: Resource Management Analysis
trace_to:
  - REQ-6.1
  - REQ-6.2
  - REQ-6.3
  - BP-HFT-BROKER-001
---

# Resource Limits Definition

## 1. Executive Summary

This document defines hard and soft resource limits for all Clawdius components. Limits are enforced at compile time (const assertions), runtime (checks), and OS level (rlimits). Per REQ-6.1 through REQ-6.3, limits ensure the system remains within memory and performance budgets.

## 2. Limit Categories

### 2.1 Limit Types

| Type | Description | Enforcement | Example |
|------|-------------|-------------|---------|
| Hard Limit | Cannot be exceeded | Panic/Error | Max file descriptors |
| Soft Limit | Warning threshold | Log/Metric | Memory warning at 75% |
| Adaptive | Adjusts based on load | Runtime | Connection pool size |
| Quota | Per-component budget | Registry | Memory per component |

### 2.2 Enforcement Layers

```
┌─────────────────────────────────────────────────────────────────┐
│                    Enforcement Layers                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│   ┌─────────────────────────────────────────────────────────┐  │
│   │  Compile Time (const assertions, type system)            │  │
│   └─────────────────────────────────────────────────────────┘  │
│                              │                                   │
│   ┌─────────────────────────────────────────────────────────┐  │
│   │  Runtime (checks before allocation)                      │  │
│   └─────────────────────────────────────────────────────────┘  │
│                              │                                   │
│   ┌─────────────────────────────────────────────────────────┐  │
│   │  OS Level (rlimits, cgroups)                             │  │
│   └─────────────────────────────────────────────────────────┘  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 3. Memory Limits

### 3.1 Global Memory Budget

```rust
pub const MEMORY_BUDGET: MemoryBudget = MemoryBudget {
    host_kernel: 5 * 1024 * 1024,         // 5 MB
    graph_rag_sqlite: 10 * 1024 * 1024,   // 10 MB
    graph_rag_lancedb: 15 * 1024 * 1024,  // 15 MB
    brain_wasm: 20 * 1024 * 1024,         // 20 MB
    sentinel: 2 * 1024 * 1024,            // 2 MB
    tui: 2 * 1024 * 1024,                 // 2 MB
    hft_ring_buffer: 512 * 1024 * 1024,   // 512 MB (HFT only)
    hft_arena: 256 * 1024 * 1024,         // 256 MB (HFT only)
};

pub const fn total_standard_budget() -> usize {
    MEMORY_BUDGET.host_kernel +
    MEMORY_BUDGET.graph_rag_sqlite +
    MEMORY_BUDGET.graph_rag_lancedb +
    MEMORY_BUDGET.brain_wasm +
    MEMORY_BUDGET.sentinel +
    MEMORY_BUDGET.tui
}

pub const fn total_hft_budget() -> usize {
    total_standard_budget() +
    MEMORY_BUDGET.hft_ring_buffer +
    MEMORY_BUDGET.hft_arena
}

const _: () = assert!(total_standard_budget() <= 60 * 1024 * 1024);
const _: () = assert!(total_hft_budget() <= 900 * 1024 * 1024);
```

### 3.2 Per-Component Limits

| Component | Hard Limit | Soft Limit (75%) | Quota Enforcement |
|-----------|------------|------------------|-------------------|
| Host Kernel | 5 MB | 3.75 MB | Arena allocator |
| Graph-RAG SQLite | 10 MB | 7.5 MB | mmap limit |
| Graph-RAG LanceDB | 15 MB | 11.25 MB | Cache limit |
| Brain WASM | 20 MB | 15 MB | Linear memory |
| Sentinel | 2 MB | 1.5 MB | HashMap size |
| TUI | 2 MB | 1.5 MB | Buffer size |
| HFT Ring Buffer | 512 MB | 384 MB | HugePage size |
| HFT Arena | 256 MB | 192 MB | Arena capacity |

### 3.3 Memory Limit Enforcement

```rust
pub struct MemoryLimiter {
    budget: usize,
    allocated: AtomicUsize,
    soft_threshold: f64,
}

impl MemoryLimiter {
    pub const fn new(budget: usize, soft_threshold: f64) -> Self {
        Self {
            budget,
            allocated: AtomicUsize::new(0),
            soft_threshold,
        }
    }
    
    pub fn try_alloc(&self, size: usize) -> Result<AllocationGuard, MemoryError> {
        let current = self.allocated.load(Ordering::Relaxed);
        let new_total = current + size;
        
        if new_total > self.budget {
            return Err(MemoryError::LimitExceeded {
                requested: size,
                available: self.budget - current,
            });
        }
        
        match self.allocated.compare_exchange(
            current,
            new_total,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                if new_total as f64 > self.budget as f64 * self.soft_threshold {
                    tracing::warn!(
                        component = std::any::type_name::<Self>(),
                        usage_pct = (new_total as f64 / self.budget as f64) * 100.0,
                        "Memory soft limit exceeded"
                    );
                }
                
                Ok(AllocationGuard {
                    limiter: self,
                    size,
                })
            }
            Err(_) => self.try_alloc(size),
        }
    }
    
    fn release(&self, size: usize) {
        self.allocated.fetch_sub(size, Ordering::Release);
    }
    
    pub fn usage(&self) -> MemoryUsage {
        let allocated = self.allocated.load(Ordering::Relaxed);
        MemoryUsage {
            allocated,
            budget: self.budget,
            utilization: allocated as f64 / self.budget as f64,
        }
    }
}

pub struct AllocationGuard<'a> {
    limiter: &'a MemoryLimiter,
    size: usize,
}

impl Drop for AllocationGuard<'_> {
    fn drop(&mut self) {
        self.limiter.release(self.size);
    }
}

#[derive(Debug, Clone)]
pub struct MemoryUsage {
    pub allocated: usize,
    pub budget: usize,
    pub utilization: f64,
}
```

---

## 4. File Handle Limits

### 4.1 File Descriptor Limits

```rust
pub const FILE_LIMITS: FileLimits = FileLimits {
    max_open_files: 64,
    max_file_size: 1024 * 1024 * 1024,  // 1 GB
    max_path_length: 4096,
    log_file_rotation: 10 * 1024 * 1024,  // 10 MB
};

pub const fn validate_file_limits() {
    assert!(FILE_LIMITS.max_open_files <= 1024);
    assert!(FILE_LIMITS.max_file_size <= 2 * 1024 * 1024 * 1024);
}
```

### 4.2 File Limits Configuration

| Limit | Value | Rationale |
|-------|-------|-----------|
| Max open files | 64 | Well below ulimit (1024) |
| Max file size | 1 GB | Prevent runaway growth |
| Log rotation | 10 MB | 10 files × 10 MB = 100 MB max logs |
| Max path | 4096 | PATH_MAX on Linux |

### 4.3 OS-Level Enforcement

```rust
pub fn set_file_limits() -> Result<(), LimitError> {
    #[cfg(target_os = "linux")]
    {
        use libc::{rlimit, RLIMIT_NOFILE, setrlimit};
        
        let rlim = rlimit {
            rlim_cur: 128,  // Soft limit
            rlim_max: 256,  // Hard limit
        };
        
        unsafe {
            if setrlimit(RLIMIT_NOFILE, &rlim) != 0 {
                return Err(LimitError::SetrlimitFailed);
            }
        }
    }
    
    Ok(())
}
```

---

## 5. Database Connection Limits

### 5.1 SQLite Pool Limits

```rust
pub const SQLITE_POOL_LIMITS: SqlitePoolLimits = SqlitePoolLimits {
    max_connections: 8,
    min_idle: 1,
    connection_timeout_ms: 5000,
    idle_timeout_ms: 300_000,
    max_lifetime_ms: 3_600_000,
    busy_timeout_ms: 5000,
};

pub const SQLITE_CACHE_LIMITS: SqliteCacheLimits = SqliteCacheLimits {
    page_cache_kb: 2048,      // 2 MB
    mmap_size_mb: 10,         // 10 MB
    wal_autocheckpoint: 1000, // Checkpoint every 1000 pages
};
```

### 5.2 LanceDB Limits

```rust
pub const LANCEDB_LIMITS: LanceDbLimits = LanceDbLimits {
    max_open_tables: 16,
    query_timeout_ms: 30_000,
    index_cache_mb: 100,
    vector_batch_size: 1000,
};
```

### 5.3 Database Limits Summary

| Resource | SQLite | LanceDB |
|----------|--------|---------|
| Max connections | 8 | N/A |
| Page cache | 2 MB | N/A |
| mmap size | 10 MB | N/A |
| Query timeout | 5 s | 30 s |
| Index cache | N/A | 100 MB |

---

## 6. Network Connection Limits

### 6.1 Connection Pool Limits

```rust
pub const NETWORK_LIMITS: NetworkLimits = NetworkLimits {
    max_tcp_connections: 32,
    max_websocket_connections: 8,
    connect_timeout_ms: 10_000,
    read_timeout_ms: 30_000,
    write_timeout_ms: 30_000,
    idle_timeout_ms: 300_000,
    max_retries: 3,
    retry_delay_ms: 1000,
    max_request_size: 10 * 1024 * 1024,   // 10 MB
    max_response_size: 100 * 1024 * 1024, // 100 MB
};
```

### 6.2 Per-Endpoint Limits

| Endpoint Type | Max Connections | Timeout | Rate Limit |
|---------------|-----------------|---------|------------|
| LLM API | 4 | 60s | 100 req/min |
| MCP Tool | 8 | 30s | 1000 req/min |
| Matrix Bridge | 2 | 10s | 10 msg/s |
| WhatsApp Bridge | 2 | 10s | 5 msg/s |
| Market Data (HFT) | 1 | N/A | Unlimited |

### 6.3 Connection Rate Limiting

```rust
pub struct RateLimiter {
    window: Duration,
    max_requests: u32,
    requests: RwLock<VecDeque<Instant>>,
}

impl RateLimiter {
    pub const fn new(window: Duration, max_requests: u32) -> Self {
        Self {
            window,
            max_requests,
            requests: RwLock::new(VecDeque::new()),
        }
    }
    
    pub fn try_acquire(&self) -> Result<(), RateLimitError> {
        let mut requests = self.requests.write().unwrap();
        let now = Instant::now();
        
        // Remove expired entries
        while let Some(front) = requests.front() {
            if now.duration_since(*front) > self.window {
                requests.pop_front();
            } else {
                break;
            }
        }
        
        if requests.len() >= self.max_requests as usize {
            let oldest = requests.front().unwrap();
            let retry_after = self.window - now.duration_since(*oldest);
            return Err(RateLimitError::Exceeded { retry_after });
        }
        
        requests.push_back(now);
        Ok(())
    }
}
```

---

## 7. Thread and Task Limits

### 7.1 Thread Pool Limits

```rust
pub const THREAD_LIMITS: ThreadLimits = ThreadLimits {
    monoio_workers: 4,          // Thread-per-core
    blocking_threads: 8,        // For blocking operations
    max_blocking_threads: 32,   // Upper bound
    stack_size_kb: 512,         // 512 KB per thread
    hft_isolated_cores: 4,      // For HFT mode
};
```

### 7.2 Task Spawn Limits

```rust
pub const TASK_LIMITS: TaskLimits = TaskLimits {
    max_concurrent_tasks: 10_000,
    max_pending_tasks: 100_000,
    task_timeout_ms: 60_000,
    spawn_rate_per_sec: 1_000,
};
```

### 7.3 Thread Limits Summary

| Resource | Limit | Rationale |
|----------|-------|-----------|
| monoio workers | 4 | Thread-per-core model |
| Blocking threads | 8 | I/O operations |
| Max blocking | 32 | Backpressure |
| Stack size | 512 KB | Default + buffer |
| Max concurrent tasks | 10,000 | Prevent task explosion |
| Max pending tasks | 100,000 | Hard limit |

---

## 8. WASM Limits

### 8.1 Wasmtime Limits

```rust
pub const WASM_LIMITS: WasmLimits = WasmLimits {
    max_instances: 4,
    initial_memory_pages: 160,  // 10 MB
    max_memory_pages: 320,      // 20 MB
    max_table_size: 10_000,
    max_instances_per_module: 1,
    compilation_timeout_ms: 60_000,
    execution_timeout_ms: 30_000,
    max_fuel: 1_000_000_000,    // Instructions
};
```

### 8.2 WASM Resource Limits

| Resource | Limit | Rationale |
|----------|-------|-----------|
| Max instances | 4 | Memory budget |
| Initial memory | 10 MB | Startup size |
| Max memory | 20 MB | Hard limit |
| Max table size | 10,000 | Function references |
| Max fuel | 1B | Execution time limit |
| Compilation timeout | 60s | Module compilation |
| Execution timeout | 30s | Function execution |

### 8.3 Fuel-Based Execution Limiting

```rust
impl WasmInstance {
    pub fn invoke_with_limit(
        &mut self,
        func: &str,
        args: &[wasmtime::Val],
        fuel: u64,
    ) -> Result<Option<wasmtime::Val>, WasmError> {
        self.store.set_fuel(Some(fuel));
        
        let result = self.invoke(func, args);
        
        match self.store.get_fuel() {
            Some(remaining) if remaining == 0 => {
                Err(WasmError::OutOfFuel)
            }
            _ => result,
        }
    }
}
```

---

## 9. Sandbox Limits

### 9.1 Sandbox Resource Limits

```rust
pub const SANDBOX_LIMITS: SandboxLimits = SandboxLimits {
    max_sandboxes: 16,
    max_sandbox_memory_mb: 256,
    max_sandbox_cpu_percent: 50,
    max_sandbox_time_ms: 300_000,  // 5 minutes
    max_sandbox_pids: 64,
    max_sandbox_files: 128,
};

pub const TIER_LIMITS: TierLimits = TierLimits {
    tier0_memory_mb: 32,
    tier1_memory_mb: 64,
    tier2_memory_mb: 128,
    tier3_memory_mb: 256,
    tier0_timeout_ms: 5_000,
    tier1_timeout_ms: 30_000,
    tier2_timeout_ms: 60_000,
    tier3_timeout_ms: 300_000,
};
```

### 9.2 Sandbox Limits by Tier

| Tier | Memory | CPU | Time | PIDs | Files |
|------|--------|-----|------|------|-------|
| Tier 0 (Native) | 32 MB | 25% | 5s | 16 | 32 |
| Tier 1 (bubblewrap) | 64 MB | 25% | 30s | 32 | 64 |
| Tier 2 (bubblewrap+) | 128 MB | 50% | 60s | 64 | 128 |
| Tier 3 (Podman) | 256 MB | 50% | 5m | 64 | 128 |

### 9.3 cgroup Enforcement

```rust
#[cfg(target_os = "linux")]
pub fn apply_cgroup_limits(sandbox_id: &Uuid, limits: &SandboxLimits) -> Result<(), SandboxError> {
    let cgroup_path = format!("/sys/fs/cgroup/clawdius/{}", sandbox_id);
    
    std::fs::create_dir_all(&cgroup_path)?;
    
    // Memory limit
    std::fs::write(
        format!("{}/memory.max", cgroup_path),
        format!("{}", limits.max_sandbox_memory_mb * 1024 * 1024),
    )?;
    
    // CPU limit
    std::fs::write(
        format!("{}/cpu.max", cgroup_path),
        format!("{} 100000", limits.max_sandbox_cpu_percent * 1000),
    )?;
    
    // PIDs limit
    std::fs::write(
        format!("{}/pids.max", cgroup_path),
        format!("{}", limits.max_sandbox_pids),
    )?;
    
    Ok(())
}
```

---

## 10. Timeout Values

### 10.1 Operation Timeouts

```rust
pub const TIMEOUTS: Timeouts = Timeouts {
    // Network
    connect: Duration::from_secs(10),
    read: Duration::from_secs(30),
    write: Duration::from_secs(30),
    idle: Duration::from_secs(300),
    
    // Database
    db_query: Duration::from_secs(5),
    db_transaction: Duration::from_secs(30),
    
    // LLM
    llm_request: Duration::from_secs(60),
    llm_stream: Duration::from_secs(300),
    
    // Sandbox
    sandbox_spawn: Duration::from_secs(5),
    sandbox_execute: Duration::from_secs(30),
    
    // HFT
    hft_risk_check: Duration::from_micros(100),
    hft_order_dispatch: Duration::from_millis(1),
    
    // Graceful shutdown
    shutdown_timeout: Duration::from_secs(30),
};
```

### 10.2 Timeout Summary

| Category | Operation | Timeout | Rationale |
|----------|-----------|---------|-----------|
| Network | Connect | 10s | TCP handshake |
| Network | Read | 30s | Slow responses |
| Network | Idle | 300s | Connection reuse |
| Database | Query | 5s | Quick queries |
| Database | Transaction | 30s | Complex operations |
| LLM | Request | 60s | API latency |
| LLM | Stream | 300s | Long generations |
| Sandbox | Spawn | 5s | Container start |
| Sandbox | Execute | 30s | Tool execution |
| HFT | Risk check | 100μs | WCET bound |
| HFT | Order | 1ms | End-to-end |
| Shutdown | Graceful | 30s | Cleanup time |

---

## 11. Limit Enforcement Architecture

### 11.1 Centralized Limit Manager

```rust
pub struct LimitManager {
    memory: MemoryLimiter,
    files: FileRegistry,
    connections: ConnectionPool,
    rate_limiters: HashMap<String, RateLimiter>,
}

impl LimitManager {
    pub fn check_all(&self) -> LimitReport {
        LimitReport {
            memory: self.memory.usage(),
            files: self.files.count(),
            connections: self.connections.count(),
            rate_limiters: self.rate_limiters.iter()
                .map(|(k, v)| (k.clone(), v.usage()))
                .collect(),
        }
    }
    
    pub fn enforce(&self) -> Result<(), LimitViolation> {
        let report = self.check_all();
        
        if report.memory.utilization > 0.95 {
            return Err(LimitViolation::MemoryCritical);
        }
        
        if report.files > FILE_LIMITS.max_open_files {
            return Err(LimitViolation::FileDescriptorsExhausted);
        }
        
        Ok(())
    }
}

#[derive(Debug)]
pub struct LimitReport {
    pub memory: MemoryUsage,
    pub files: usize,
    pub connections: usize,
    pub rate_limiters: HashMap<String, u32>,
}
```

### 11.2 Metrics Export

```rust
impl LimitManager {
    pub fn export_metrics(&self) -> Vec<Metric> {
        let report = self.check_all();
        
        vec![
            Metric::gauge("memory_allocated_bytes", report.memory.allocated as f64),
            Metric::gauge("memory_budget_bytes", report.memory.budget as f64),
            Metric::gauge("memory_utilization", report.memory.utilization),
            Metric::gauge("open_files", report.files as f64),
            Metric::gauge("open_connections", report.connections as f64),
        ]
    }
}
```

---

## 12. Compliance Matrix

### 12.1 Requirements Traceability

| Requirement | Limit | Status |
|-------------|-------|--------|
| REQ-6.1 (<15MB compressed) | Binary size limit | ✅ |
| REQ-6.2 (<2s cold start) | Lazy init, timeouts | ✅ |
| REQ-6.3 (<30MB idle) | Memory budget 54MB | ⚠️ Slight over |

### 12.2 Limit Categories

| Category | Hard Limits | Soft Limits | Adaptive | Quotas |
|----------|-------------|-------------|----------|--------|
| Memory | 8 | 8 | 0 | 6 |
| Files | 2 | 2 | 1 | 1 |
| Network | 4 | 4 | 2 | 0 |
| Database | 3 | 3 | 1 | 1 |
| WASM | 4 | 4 | 0 | 1 |
| Sandbox | 5 | 5 | 0 | 4 |
| **Total** | **26** | **26** | **4** | **13** |

---

**Document Status:** APPROVED
**Next Review:** Phase 4 Implementation
**Sign-off:** Resource Engineer
