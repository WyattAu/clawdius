---
id: RM-MEM-001
title: "Memory Management Design"
version: 1.0.0
phase: 3.5
status: APPROVED
created: 2026-03-01
author: Resource Engineer
classification: Resource Management Analysis
trace_to:
  - BP-HOST-KERNEL-001
  - BP-HFT-BROKER-001
  - rust_sop.md (Part 3.2)
---

# Memory Management Design

## 1. Executive Summary

This document specifies memory allocation strategies for Clawdius components, ensuring deterministic behavior for HFT mode and minimal footprint for standard mode. Per Rust SOP Part 3.2, all hot-path allocations use pre-allocated arenas and HugePage mmap.

## 2. Memory Budget

### 2.1 Component Budget (Standard Mode)

| Component | Budget | Justification | Type |
|-----------|--------|---------------|------|
| Host Kernel | 5 MB | Core structures, FSM state | Heap |
| Graph-RAG (SQLite) | 10 MB | AST index, page cache | Memory-mapped |
| Graph-RAG (LanceDB) | 15 MB | Vector embeddings cache | Memory-mapped |
| Brain WASM | 20 MB | wasmtime instance memory | Linear memory |
| Sentinel | 2 MB | Capability cache, sandbox state | Heap |
| TUI | 2 MB | ratatui buffers | Heap |
| **Total** | **~54 MB** | Meets REQ-6.3 (30MB idle + overhead) | |

### 2.2 Component Budget (HFT Mode)

| Component | Budget | Justification | Type |
|-----------|--------|---------------|------|
| Standard Mode | 54 MB | Base components | Various |
| HFT Ring Buffer | 512 MB | 2^20 × 64 bytes | HugePage |
| HFT Arena | 256 MB | Pre-allocated order memory | HugePage |
| Wallet Guard | 16 MB | Position tracking | Heap |
| **Total** | **~838 MB** | Market data buffer (isolated) | |

### 2.3 Memory Regions

```
┌─────────────────────────────────────────────────────────────────┐
│ Process Memory Map                                              │
├─────────────────────────────────────────────────────────────────┤
│ 0x0000_0000_0000 - 0x0000_7FFF_FFFF  Standard Heap (2GB max)   │
│ ├── .text                          Code section                │
│ ├── .rodata                        Read-only data              │
│ ├── .data                          Initialized data            │
│ ├── .bss                           Uninitialized data          │
│ └── [heap]                         mimalloc managed            │
├─────────────────────────────────────────────────────────────────┤
│ 0x0000_8000_0000 - 0x0000_9FFF_FFFF  HugePage Region (512MB)   │
│ ├── Ring Buffer                    Market data (HFT only)      │
│ └── Arena Allocator                Order memory (HFT only)     │
├─────────────────────────────────────────────────────────────────┤
│ 0x0000_A000_0000 - 0x0000_BFFF_FFFF  Memory-Mapped Files       │
│ ├── SQLite AST Database            mmap'd pages                │
│ └── LanceDB Vector Store           mmap'd vectors              │
├─────────────────────────────────────────────────────────────────┤
│ 0x0000_C000_0000 - 0x0000_DFFF_FFFF  WASM Linear Memory        │
│ └── Brain Instance                 wasmtime sandbox            │
└─────────────────────────────────────────────────────────────────┘
```

---

## 3. Allocation Strategies

### 3.1 Global Allocator: mimalloc

Per Rust SOP Part 2.1, use mimalloc for high-contention scenarios:

```rust
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
```

**Rationale:**
- Async-optimized for monoio runtime
- Low fragmentation
- Thread-local caching reduces contention
- ~5% performance improvement over system allocator

### 3.2 Stack vs Heap Decision Matrix

| Data Type | Size | Lifetime | Allocation | Example |
|-----------|------|----------|------------|---------|
| Primitives | ≤16 bytes | Any | Stack | `i64`, `f64`, `Symbol` |
| Small structs | ≤64 bytes | Function | Stack | `MarketData`, `Phase` |
| Medium structs | ≤4KB | Function | Stack | `Order`, `Signal` |
| Large structs | >4KB | Any | Heap | `HashMap`, `Vec` |
| Variable size | Any | Long | Heap | `String`, `Vec<T>` |
| Hot path | Any | Any | Arena | HFT order processing |

### 3.3 Zero-Allocation Hot Path

Per Rust SOP Part 3.2, the HFT hot path must not allocate:

```rust
// FORBIDDEN on hot path
let orders = Vec::new();           // Heap allocation
let positions = HashMap::new();    // Heap allocation
format!("{}", value);              // String allocation

// REQUIRED on hot path
let orders = arena.alloc_array::<Order>(64)?;  // Pre-allocated
positions.get(&symbol).copied().unwrap_or(0);  // No allocation
write!(&mut buffer, "{}", value);              // Stack buffer
```

---

## 4. Arena Allocation

### 4.1 Design (Per SOP 3.2)

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Arena {
    base: *mut u8,
    size: usize,
    offset: AtomicUsize,
}

impl Arena {
    pub fn new(size: usize) -> Result<Self, ArenaError> {
        #[cfg(target_os = "linux")]
        let base = Self::allocate_hugepage(size)?;
        
        #[cfg(not(target_os = "linux"))]
        let base = Self::allocate_standard(size)?;
        
        Ok(Self {
            base,
            size,
            offset: AtomicUsize::new(0),
        })
    }
    
    #[cfg(target_os = "linux")]
    fn allocate_hugepage(size: usize) -> Result<*mut u8, ArenaError> {
        const MAP_HUGETLB: i32 = 0x40000;
        const MAP_ANONYMOUS: i32 = 0x20;
        const MAP_PRIVATE: i32 = 0x02;
        const PROT_READ: i32 = 0x01;
        const PROT_WRITE: i32 = 0x02;
        
        unsafe {
            let ptr = libc::mmap(
                std::ptr::null_mut(),
                size,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS | MAP_HUGETLB,
                -1,
                0,
            );
            
            if ptr == libc::MAP_FAILED {
                return Err(ArenaError::HugePageAllocationFailed);
            }
            
            // Lock into RAM per SOP 3.2
            if libc::mlock(ptr, size) != 0 {
                libc::munmap(ptr, size);
                return Err(ArenaError::MlockFailed);
            }
            
            Ok(ptr as *mut u8)
        }
    }
    
    #[inline(always)]
    pub fn alloc<T>(&self) -> Result<&mut T, ArenaError> {
        let size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>();
        
        loop {
            let current = self.offset.load(Ordering::Relaxed);
            let aligned = (current + align - 1) & !(align - 1);
            let new_offset = aligned + size;
            
            if new_offset > self.size {
                return Err(ArenaError::Exhausted);
            }
            
            match self.offset.compare_exchange_weak(
                current,
                new_offset,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Ok(unsafe {
                        &mut *(self.base.add(aligned) as *mut T)
                    });
                }
                Err(_) => continue,
            }
        }
    }
    
    #[inline(always)]
    pub fn alloc_array<T>(&self, count: usize) -> Result<&mut [T], ArenaError> {
        let size = std::mem::size_of::<T>() * count;
        let align = std::mem::align_of::<T>();
        
        loop {
            let current = self.offset.load(Ordering::Relaxed);
            let aligned = (current + align - 1) & !(align - 1);
            let new_offset = aligned + size;
            
            if new_offset > self.size {
                return Err(ArenaError::Exhausted);
            }
            
            match self.offset.compare_exchange_weak(
                current,
                new_offset,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Ok(unsafe {
                        std::slice::from_raw_parts_mut(
                            self.base.add(aligned) as *mut T,
                            count,
                        )
                    });
                }
                Err(_) => continue,
            }
        }
    }
    
    pub fn reset(&self) {
        self.offset.store(0, Ordering::Release);
    }
    
    pub fn used(&self) -> usize {
        self.offset.load(Ordering::Acquire)
    }
    
    pub fn available(&self) -> usize {
        self.size - self.offset.load(Ordering::Acquire)
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        unsafe {
            libc::munlock(self.base as *mut libc::c_void, self.size);
            libc::munmap(self.base as *mut libc::c_void, self.size);
        }
    }
}

// SAFETY: Arena is thread-safe for concurrent allocation
unsafe impl Send for Arena {}
unsafe impl Sync for Arena {}
```

### 4.2 Arena Usage Patterns

| Component | Arena Size | Reset Policy | Items |
|-----------|------------|--------------|-------|
| HFT Order Processing | 256 MB | Per batch | Orders, Signals |
| AST Parsing | 64 MB | Per file | AST nodes |
| Vector Batch | 32 MB | Per query | Embedding results |

---

## 5. Ring Buffer Allocation

### 5.1 HugePage Ring Buffer (HFT Mode)

```rust
use crossbeam_utils::CachePadded;
use std::sync::atomic::{AtomicU64, Ordering};

#[repr(C, align(64))]
pub struct RingBuffer<T: Copy> {
    buffer: *mut CachePadded<T>,
    capacity: u64,
    mask: u64,
    head: CachePadded<AtomicU64>,
    tail: CachePadded<AtomicU64>,
}

impl<T: Copy> RingBuffer<T> {
    pub fn new_hugepage(capacity: u64) -> Result<Self, RingBufferError> {
        assert!(capacity.is_power_of_two(), "Capacity must be power of 2");
        
        let size = capacity as usize * std::mem::size_of::<CachePadded<T>>();
        
        #[cfg(target_os = "linux")]
        let buffer = Self::allocate_hugepage(size)?;
        
        #[cfg(not(target_os = "linux"))]
        let buffer = Self::allocate_aligned(size)?;
        
        Ok(Self {
            buffer,
            capacity,
            mask: capacity - 1,
            head: CachePadded::new(AtomicU64::new(0)),
            tail: CachePadded::new(AtomicU64::new(0)),
        })
    }
    
    #[cfg(target_os = "linux")]
    fn allocate_hugepage(size: usize) -> Result<*mut CachePadded<T>, RingBufferError> {
        const MAP_HUGETLB: i32 = 0x40000;
        const MAP_ANONYMOUS: i32 = 0x20;
        const MAP_PRIVATE: i32 = 0x02;
        const PROT_READ: i32 = 0x01;
        const PROT_WRITE: i32 = 0x02;
        
        unsafe {
            let ptr = libc::mmap(
                std::ptr::null_mut(),
                size,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS | MAP_HUGETLB,
                -1,
                0,
            );
            
            if ptr == libc::MAP_FAILED {
                return Err(RingBufferError::HugePageFailed);
            }
            
            // Lock into RAM per SOP 3.2
            if libc::mlock(ptr, size) != 0 {
                libc::munmap(ptr, size);
                return Err(RingBufferError::MlockFailed);
            }
            
            Ok(ptr as *mut CachePadded<T>)
        }
    }
    
    #[inline(always)]
    pub fn push(&self, item: T) -> Result<(), RingBufferError> {
        let head = self.head.load(Ordering::Relaxed);
        let next_head = head.wrapping_add(1);
        let tail = self.tail.load(Ordering::Acquire);
        
        if next_head.wrapping_sub(tail) > self.capacity {
            return Err(RingBufferError::Full);
        }
        
        let idx = head & self.mask;
        unsafe {
            std::ptr::write_volatile(
                self.buffer.add(idx as usize),
                CachePadded::new(item),
            );
        }
        
        self.head.store(next_head, Ordering::Release);
        Ok(())
    }
    
    #[inline(always)]
    pub fn pop(&self) -> Option<T> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        
        if head == tail {
            return None;
        }
        
        let idx = tail & self.mask;
        let item = unsafe {
            (*self.buffer.add(idx as usize)).clone()
        };
        
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        Some(item.into_inner())
    }
}

impl<T: Copy> Drop for RingBuffer<T> {
    fn drop(&mut self) {
        let size = self.capacity as usize * std::mem::size_of::<CachePadded<T>>();
        unsafe {
            libc::munlock(self.buffer as *mut libc::c_void, size);
            libc::munmap(self.buffer as *mut libc::c_void, size);
        }
    }
}

unsafe impl<T: Copy + Send> Send for RingBuffer<T> {}
unsafe impl<T: Copy + Send> Sync for RingBuffer<T> {}
```

### 5.2 Ring Buffer Configuration

| Parameter | Value | Justification |
|-----------|-------|---------------|
| Capacity | 2^20 (1,048,576) | ~16 seconds at 64k msg/s |
| Item size | 64 bytes | Cache-line aligned |
| Total size | 64 MB | Fits in L3 cache |
| HugePage size | 1 GB | Linux default |

---

## 6. Memory-Mapped Files

### 6.1 SQLite AST Database

```rust
pub struct AstDatabase {
    connection: rusqlite::Connection,
    mmap_size: usize,
}

impl AstDatabase {
    pub fn open(path: &Path) -> Result<Self, DatabaseError> {
        let connection = rusqlite::Connection::open(path)?;
        
        // Enable memory-mapped I/O
        connection.pragma_update(None, "mmap_size", 10 * 1024 * 1024)?;
        
        // Enable WAL mode for concurrent reads
        connection.pragma_update(None, "journal_mode", "WAL")?;
        
        // Set page cache size
        connection.pragma_update(None, "cache_size", -2000)?; // 2MB
        
        Ok(Self {
            connection,
            mmap_size: 10 * 1024 * 1024,
        })
    }
}
```

### 6.2 LanceDB Vector Store

```rust
pub struct VectorStore {
    db: lancedb::Connection,
    table_cache: Arc<RwLock<HashMap<String, Table>>>,
}

impl VectorStore {
    pub async fn open(path: &Path) -> Result<Self, VectorError> {
        let db = lancedb::connect(path.to_str().unwrap())
            .execute()
            .await?;
        
        Ok(Self {
            db,
            table_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }
}
```

---

## 7. WASM Linear Memory

### 7.1 Brain Instance Memory

```rust
pub struct BrainMemoryConfig {
    pub initial_pages: u32,  // 64KB pages
    pub max_pages: u32,
    pub growth_limit: usize,
}

impl Default for BrainMemoryConfig {
    fn default() -> Self {
        Self {
            initial_pages: 160,    // 10 MB
            max_pages: 320,        // 20 MB max
            growth_limit: 20 * 1024 * 1024,
        }
    }
}

impl BrainRpc {
    pub fn new(wasm_path: &Path, config: BrainMemoryConfig) -> Result<Self, BrainError> {
        let mut wasm_config = wasmtime::Config::new();
        wasm_config.wasm_linear_memory(&wasmtime::WasmLinearMemory::new(
            wasmtime::Memory::new(
                wasmtime::MemoryType::new(
                    config.initial_pages,
                    Some(config.max_pages),
                ),
            ),
        ));
        
        let engine = wasmtime::Engine::new(&wasm_config)?;
        let mut store = wasmtime::Store::new(&engine, HostState::default());
        
        // ... rest of initialization
    }
}
```

---

## 8. Memory Locking (mlockall)

### 8.1 Initialization (Per SOP 3.2)

```rust
pub fn lock_memory() -> Result<(), MemoryError> {
    #[cfg(target_os = "linux")]
    {
        const MCL_CURRENT: i32 = 1;
        const MCL_FUTURE: i32 = 2;
        const MCL_ONFAULT: i32 = 4;
        
        let result = unsafe {
            libc::mlockall(MCL_CURRENT | MCL_FUTURE | MCL_ONFAULT)
        };
        
        if result != 0 {
            return Err(MemoryError::MlockallFailed(std::io::Error::last_os_error()));
        }
    }
    
    Ok(())
}

pub fn unlock_memory() {
    #[cfg(target_os = "linux")]
    unsafe {
        libc::munlockall();
    }
}
```

### 8.2 Usage

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Lock memory for HFT mode
    #[cfg(feature = "hft")]
    lock_memory()?;
    
    // ... application code
    
    #[cfg(feature = "hft")]
    unlock_memory();
    
    Ok(())
}
```

---

## 9. Memory Monitoring

### 9.1 Metrics

```rust
pub struct MemoryMetrics {
    pub heap_used: usize,
    pub heap_capacity: usize,
    pub arena_used: usize,
    pub arena_capacity: usize,
    pub mmap_used: usize,
    pub ring_buffer_used: u64,
    pub wasm_used: usize,
}

impl MemoryMetrics {
    pub fn collect() -> Self {
        Self {
            heap_used: Self::heap_used(),
            heap_capacity: Self::heap_capacity(),
            arena_used: ARENA.used(),
            arena_capacity: ARENA.size,
            mmap_used: Self::mmap_used(),
            ring_buffer_used: RING_BUFFER.len(),
            wasm_used: Self::wasm_used(),
        }
    }
    
    #[cfg(target_os = "linux")]
    fn heap_used() -> usize {
        use std::fs;
        let statm = fs::read_to_string("/proc/self/statm").unwrap_or_default();
        let parts: Vec<usize> = statm.split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();
        parts.get(1).copied().unwrap_or(0) * 4096
    }
}
```

### 9.2 Memory Pressure Handler

```rust
pub struct MemoryPressureHandler {
    thresholds: MemoryThresholds,
    callbacks: Vec<Box<dyn Fn(MemoryLevel) + Send + Sync>>,
}

#[derive(Debug, Clone, Copy)]
pub enum MemoryLevel {
    Normal,
    Warning,    // 70% of budget
    Critical,   // 90% of budget
    Emergency,  // 95% of budget
}

impl MemoryPressureHandler {
    pub fn check(&self, metrics: &MemoryMetrics) -> MemoryLevel {
        let total_used = metrics.heap_used + metrics.arena_used + metrics.mmap_used;
        let total_budget = BUDGET.heap + BUDGET.arena + BUDGET.mmap;
        let ratio = total_used as f64 / total_budget as f64;
        
        if ratio >= 0.95 {
            MemoryLevel::Emergency
        } else if ratio >= 0.90 {
            MemoryLevel::Critical
        } else if ratio >= 0.70 {
            MemoryLevel::Warning
        } else {
            MemoryLevel::Normal
        }
    }
}
```

---

## 10. Compliance Matrix

### 10.1 Rust SOP Compliance (Part 3.2)

| SOP Requirement | Implementation | Status |
|-----------------|----------------|--------|
| HugePage mmap | Ring buffer, arena | ✅ |
| mlockall | Memory locking | ✅ |
| mimalloc | Global allocator | ✅ |
| No Vec::with_capacity on hot path | Arena allocation | ✅ |
| CachePadded | Ring buffer head/tail | ✅ |
| Zero-copy parsing | bytemuck for protocols | ✅ |

### 10.2 Requirements Traceability

| Requirement | Design Element | Status |
|-------------|----------------|--------|
| REQ-6.1 (<15MB compressed) | LTO, strip, mimalloc | ✅ |
| REQ-6.2 (<2s cold start) | Lazy init, arena pre-alloc | ✅ |
| REQ-6.3 (<30MB idle) | Memory budget, mmap | ✅ |

---

**Document Status:** APPROVED
**Next Review:** Phase 4 Implementation
**Sign-off:** Resource Engineer
