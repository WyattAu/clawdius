# Optimization Roadmap

## Document Information

| Attribute | Value |
|-----------|-------|
| **Document ID** | PERF-OPT-001 |
| **Version** | 1.0.0 |
| **Phase** | 4 (Performance Engineering) |
| **Status** | APPROVED |
| **Created** | 2026-03-01 |
| **Classification** | Performance Specification |

---

## 1. Executive Summary

This document defines the optimization roadmap for Clawdius, prioritized by impact and effort. Key optimization strategies include:

- Profile-Guided Optimization (PGO) with cargo-pgo
- BOLT (Post-Link Optimization) for I-Cache alignment
- SIMD opportunities for parsing and computation
- Cache optimization for hot paths
- Memory layout optimization for HFT

---

## 2. Optimization Priorities

### 2.1 Priority Matrix

| Priority | Impact | Effort | Timeline | Focus |
|----------|--------|--------|----------|-------|
| P0 | Critical | Low | Week 1-2 | HFT critical path |
| P1 | High | Low | Week 2-4 | Boot time, memory |
| P2 | High | Medium | Week 4-8 | Graph-RAG, SIMD |
| P3 | Medium | Medium | Week 8-12 | TUI, I/O |
| P4 | Low | High | Ongoing | Nice-to-haves |

### 2.2 Optimization Roadmap

```
Week 1-2:  P0 - HFT Critical Path
           ├── Ring buffer zero-copy
           ├── Wallet Guard inlining
           └── Cache-padded counters
           
Week 2-4:  P1 - Boot & Memory
           ├── Lazy initialization
           ├── Memory pooling
           └── PGO baseline
           
Week 4-8:  P2 - Graph-RAG & SIMD
           ├── Parallel parsing
           ├── SIMD text search
           └── Vector indexing
           
Week 8-12: P3 - TUI & I/O
           ├── Incremental rendering
           ├── Async I/O batching
           └── Buffer pooling
           
Ongoing:   P4 - Continuous
           ├── BOLT optimization
           ├── SIMD opportunities
           └── Cache tuning
```

---

## 3. P0: HFT Critical Path

### 3.1 Ring Buffer Optimization

**Current State:**
- Lock-free SPSC queue
- Cache-padded counters
- ~200ns round-trip

**Target State:**
- Zero-copy throughout
- HugePage mmap
- ~100ns round-trip

**Implementation:**

```rust
// Optimized ring buffer
#[repr(C, align(4096))]
pub struct RingBuffer<T: Copy> {
    buffer: *mut T,
    capacity: usize,
    head: CachePadded<AtomicU64>,
    tail: CachePadded<AtomicU64>,
}

impl<T: Copy> RingBuffer<T> {
    pub fn new_hugepage(capacity: usize) -> Result<Self, Error> {
        let size = capacity * std::mem::size_of::<T>();
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_HUGETLB,
                -1,
                0,
            )
        };
        
        if ptr == libc::MAP_FAILED {
            return Err(Error::MmapFailed);
        }
        
        Ok(Self {
            buffer: ptr as *mut T,
            capacity,
            head: CachePadded::new(AtomicU64::new(0)),
            tail: CachePadded::new(AtomicU64::new(0)),
        })
    }
    
    #[inline(always)]
    pub fn push(&self, item: T) -> Result<(), RingBufferError> {
        let head = self.head.load(Ordering::Relaxed);
        let next_head = (head + 1) & (self.capacity - 1) as u64; // Bitmask for power of 2
        
        if next_head == self.tail.load(Ordering::Acquire) {
            return Err(RingBufferError::Full);
        }
        
        unsafe {
            std::ptr::write_volatile(self.buffer.add(head as usize), item);
        }
        
        self.head.store(next_head, Ordering::Release);
        Ok(())
    }
}
```

**Metrics:**
| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Write latency | 200ns | 100ns | 2x |
| Read latency | 200ns | 100ns | 2x |
| Cache misses | 5% | 1% | 5x |

### 3.2 Wallet Guard Inlining

**Current State:**
- Function calls for each check
- ~100µs for full validation
- Not inlined

**Target State:**
- Force inlined hot path
- ~50µs for full validation
- Branch prediction hints

**Implementation:**

```rust
impl WalletGuard {
    #[inline(always)]
    pub fn validate(&self, order: &Order) -> Result<(), RiskRejection> {
        // Inline all checks with branch hints
        if std::intrinsics::unlikely(self.check_position_limit(order).is_err()) {
            return self.check_position_limit(order);
        }
        
        if std::intrinsics::unlikely(self.check_order_size(order).is_err()) {
            return self.check_order_size(order);
        }
        
        if std::intrinsics::unlikely(self.check_drawdown().is_err()) {
            return self.check_drawdown();
        }
        
        if std::intrinsics::unlikely(self.check_margin(order).is_err()) {
            return self.check_margin(order);
        }
        
        Ok(())
    }
    
    #[inline(always)]
    fn check_position_limit(&self, order: &Order) -> Result<(), RiskRejection> {
        let current = self.wallet.positions.get(&order.symbol).copied().unwrap_or_default();
        let new_position = current + order.quantity;
        
        if std::intrinsics::unlikely(new_position.abs() > self.params.max_position_size) {
            return Err(RiskRejection::PositionLimitExceeded);
        }
        Ok(())
    }
}

// Mark error paths as cold
impl RiskRejection {
    #[cold]
    fn into_error(self) -> Result<(), RiskRejection> {
        Err(self)
    }
}
```

**Metrics:**
| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Validation latency | 100µs | 50µs | 2x |
| I-Cache misses | 10% | 2% | 5x |
| Branch mispredictions | 5% | 1% | 5x |

### 3.3 Cache-Padded Counters

**Implementation:**

```rust
#[repr(C, align(64))]
pub struct CachePadded<T> {
    value: T,
    _padding: [u8; 64 - std::mem::size_of::<T>()],
}

impl<T> CachePadded<T> {
    pub fn new(value: T) -> Self {
        assert!(std::mem::size_of::<T>() <= 64);
        Self {
            value,
            _padding: [0u8; 64 - std::mem::size_of::<T>()],
        }
    }
}

// Ensure counters don't share cache lines
pub struct RingBufferCounters {
    head: CachePadded<AtomicU64>,
    tail: CachePadded<AtomicU64>,
}
```

---

## 4. P1: Boot & Memory Optimization

### 4.1 Lazy Initialization

**Current State:**
- All components initialized at boot
- ~20ms boot time

**Target State:**
- Lazy init for non-critical components
- < 10ms to interactive

**Implementation:**

```rust
use std::sync::OnceLock;

pub struct ClawdiusApp {
    // Eager: Required for interactive
    tui: Tui,
    fsm: NexusFSM,
    
    // Lazy: Only when needed
    graph_rag: OnceLock<GraphRAG>,
    sandbox_manager: OnceLock<SandboxManager>,
    wasm_runtime: OnceLock<WasmRuntime>,
}

impl ClawdiusApp {
    pub fn new() -> Self {
        // Only init what's needed for interactive
        Self {
            tui: Tui::new(),
            fsm: NexusFSM::new(),
            graph_rag: OnceLock::new(),
            sandbox_manager: OnceLock::new(),
            wasm_runtime: OnceLock::new(),
        }
    }
    
    pub fn graph_rag(&self) -> &GraphRAG {
        self.graph_rag.get_or_init(|| {
            GraphRAG::open("clawdius.db").unwrap()
        })
    }
}
```

**Metrics:**
| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Boot time | 20ms | 8ms | 2.5x |
| Memory at boot | 54MB | 10MB | 5x |
| Time to interactive | 20ms | 5ms | 4x |

### 4.2 Memory Pooling

**Implementation:**

```rust
pub struct MemoryPool<T> {
    pool: Mutex<Vec<Box<T>>>,
    factory: fn() -> T,
}

impl<T> MemoryPool<T> {
    pub fn new(capacity: usize, factory: fn() -> T) -> Self {
        let pool = (0..capacity)
            .map(|_| Box::new(factory()))
            .collect();
        
        Self {
            pool: Mutex::new(pool),
            factory,
        }
    }
    
    pub fn acquire(&self) -> Pooled<T> {
        let mut pool = self.pool.lock().unwrap();
        
        let item = pool.pop().unwrap_or_else(|| {
            Box::new((self.factory)())
        });
        
        Pooled {
            item: Some(item),
            pool: self,
        }
    }
}

pub struct Pooled<'a, T> {
    item: Option<Box<T>>,
    pool: &'a MemoryPool<T>,
}

impl<'a, T> Drop for Pooled<'a, T> {
    fn drop(&mut self) {
        if let Some(item) = self.item.take() {
            self.pool.pool.lock().unwrap().push(item);
        }
    }
}
```

### 4.3 PGO Baseline

**Implementation:**

```bash
# Step 1: Build with PGO instrumentation
RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data" \
    cargo build --release

# Step 2: Run representative workload
./target/release/clawdius bench --all

# Step 3: Merge profile data
llvm-profdata merge -o /tmp/pgo-data/merged.profdata /tmp/pgo-data/*.profraw

# Step 4: Build with PGO
RUSTFLAGS="-Cprofile-use=/tmp/pgo-data/merged.profdata" \
    cargo build --release
```

**Expected Improvement:**
| Component | Before PGO | After PGO | Improvement |
|-----------|------------|-----------|-------------|
| Boot time | 20ms | 15ms | 25% |
| HFT latency | 1ms | 0.8ms | 20% |
| I-Cache misses | 10% | 5% | 50% |

---

## 5. P2: Graph-RAG & SIMD

### 5.1 Parallel Parsing

**Implementation:**

```rust
use rayon::prelude::*;

impl GraphRAG {
    pub fn parse_repository(&self, path: &Path) -> Result<ParseStats, Error> {
        let files = self.collect_source_files(path)?;
        
        let stats: Vec<ParseStats> = files
            .par_iter()
            .map(|file| self.parse_file(file))
            .collect();
        
        Ok(stats.into_iter().sum())
    }
    
    fn parse_file(&self, path: &Path) -> ParseStats {
        let content = std::fs::read_to_string(path).unwrap();
        let tree = self.parser.parse(&content, None).unwrap();
        
        // Extract AST nodes
        self.extract_nodes(path, &tree)
    }
}
```

**Metrics:**
| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| 10K files | 10s | 2.5s | 4x |
| CPU usage | 25% | 100% | 4x |
| Memory | 100MB | 200MB | 2x (trade-off) |

### 5.2 SIMD Text Search

**Implementation:**

```rust
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

pub fn find_pattern_simd(haystack: &[u8], needle: u8) -> Vec<usize> {
    let mut results = Vec::new();
    
    if is_x86_feature_detected!("avx2") {
        unsafe { find_pattern_avx2(haystack, needle, &mut results); }
    } else {
        for (i, &b) in haystack.iter().enumerate() {
            if b == needle {
                results.push(i);
            }
        }
    }
    
    results
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn find_pattern_avx2(haystack: &[u8], needle: u8, results: &mut Vec<usize>) {
    let needle_vec = _mm256_set1_epi8(needle as i8);
    
    for (chunk_idx, chunk) in haystack.chunks_exact(32).enumerate() {
        let data = _mm256_loadu_si256(chunk.as_ptr() as *const __m256i);
        let cmp = _mm256_cmpeq_epi8(data, needle_vec);
        let mask = _mm256_movemask_epi8(cmp);
        
        if mask != 0 {
            for bit in 0..32 {
                if mask & (1 << bit) != 0 {
                    results.push(chunk_idx * 32 + bit);
                }
            }
        }
    }
}
```

**Metrics:**
| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Search 1MB | 5ms | 0.5ms | 10x |
| CPU cycles | 5M | 0.5M | 10x |

### 5.3 Vector Indexing

**Implementation:**

```rust
use usearch::Index;

pub struct VectorIndex {
    index: Index,
}

impl VectorIndex {
    pub fn new(dimensions: usize) -> Self {
        let index = Index::new(IndexOptions {
            dimensions,
            metric: MetricKind::Cos,
            quantization: ScalarKind::F16,
            connectivity: 16,
            expansion_add: 128,
            expansion_search: 64,
        });
        
        Self { index }
    }
    
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(u64, f32)> {
        self.index.search(query, k).unwrap()
    }
}
```

---

## 6. P3: TUI & I/O

### 6.1 Incremental Rendering

**Implementation:**

```rust
pub struct IncrementalRenderer {
    last_frame: Vec<Cell>,
    current_frame: Vec<Cell>,
}

impl IncrementalRenderer {
    pub fn render(&mut self, frame: &[Cell]) -> Vec<Diff> {
        let mut diffs = Vec::new();
        
        for (i, (last, current)) in self.last_frame.iter().zip(frame.iter()).enumerate() {
            if last != current {
                diffs.push(Diff {
                    position: i,
                    cell: current.clone(),
                });
            }
        }
        
        self.last_frame = frame.to_vec();
        diffs
    }
}
```

**Metrics:**
| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Frame time | 16ms | 5ms | 3x |
| Bandwidth | 1MB/frame | 10KB/frame | 100x |

### 6.2 Async I/O Batching

**Implementation:**

```rust
pub struct BatchedWriter {
    buffer: Vec<u8>,
    batch_size: usize,
    writer: Box<dyn AsyncWrite + Unpin>,
}

impl BatchedWriter {
    pub async fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        self.buffer.extend_from_slice(data);
        
        if self.buffer.len() >= self.batch_size {
            self.flush().await?;
        }
        
        Ok(())
    }
    
    pub async fn flush(&mut self) -> Result<(), Error> {
        if !self.buffer.is_empty() {
            self.writer.write_all(&self.buffer).await?;
            self.buffer.clear();
        }
        Ok(())
    }
}
```

---

## 7. P4: BOLT Optimization

### 7.1 BOLT Overview

BOLT (Binary Optimization and Layout Tool) is a post-link optimizer that:

1. Collects execution profile
2. Reorders functions for better I-Cache usage
3. Optimizes branch layout
4. Improves code locality

### 7.2 BOLT Workflow

```bash
# Step 1: Build with debug info
cargo build --release

# Step 2: Collect profile with perf
perf record -F 99 -e cycles -o perf.data -- ./target/release/clawdius bench

# Step 3: Convert to BOLT format
perf2bolt -p perf.data -o perf.fdata ./target/release/clawdius

# Step 4: Run BOLT
llvm-bolt ./target/release/clawdius \
    -o ./target/release/clawdius.bolt \
    -data=perf.fdata \
    -reorder-blocks=cache+ \
    -reorder-functions=hfsort \
    -split-functions=3 \
    -split-all-cold \
    -dyno-stats

# Step 5: Replace binary
mv ./target/release/clawdius.bolt ./target/release/clawdius
```

### 7.3 Expected Improvements

| Metric | Before BOLT | After BOLT | Improvement |
|--------|-------------|------------|-------------|
| Boot time | 15ms | 12ms | 20% |
| HFT latency | 0.8ms | 0.6ms | 25% |
| I-Cache misses | 5% | 2% | 60% |
| Code size | 10MB | 10.5MB | +5% |

---

## 8. SIMD Opportunities

### 8.1 Identified SIMD Targets

| Component | Operation | Current | SIMD | Priority |
|-----------|-----------|---------|------|----------|
| Graph-RAG | Text search | Scalar | AVX2 | P2 |
| HFT | Price comparison | Scalar | AVX2 | P2 |
| Graph-RAG | Vector distance | Scalar | AVX-512 | P3 |
| TUI | Cell comparison | Scalar | SSE4.2 | P3 |

### 8.2 SIMD Implementation Strategy

1. **Use portable SIMD** via `std::simd` when stable
2. **Fallback to scalar** for unsupported CPUs
3. **Runtime dispatch** via `is_x86_feature_detected`
4. **Benchmark all paths** for regression detection

---

## 9. Cache Optimization

### 9.1 Cache-Friendly Data Structures

```rust
// Before: Cache-unfriendly
struct BadStructure {
    items: Vec<Item>,
    flags: Vec<bool>,
    counts: Vec<u32>,
}

// After: Cache-friendly (Structure of Arrays)
struct GoodStructure {
    items: Vec<Item>,
    flags: Vec<bool>,
    counts: Vec<u32>,
}

// Best: Cache-optimized (Array of Structures)
#[repr(C)]
struct Item {
    data: ItemData,
    flag: bool,
    count: u32,
    _padding: [u8; 3], // Align to 8 bytes
}

struct OptimizedStructure {
    items: Vec<Item>,
}
```

### 9.2 Prefetching

```rust
use std::intrinsics::prefetch_read_data;

pub fn process_with_prefetch(data: &[Item]) -> u64 {
    let mut sum = 0u64;
    
    for (i, item) in data.iter().enumerate() {
        // Prefetch next cache line
        if i + 8 < data.len() {
            unsafe {
                prefetch_read_data(&data[i + 8], 3);
            }
        }
        
        sum += item.count as u64;
    }
    
    sum
}
```

---

## 10. Monitoring & Regression Detection

### 10.1 Key Metrics

| Metric | Tool | Threshold | Alert |
|--------|------|-----------|-------|
| Boot time | criterion | +10% | Warning |
| HFT latency | criterion | +5% | Critical |
| Memory usage | custom | +10% | Warning |
| I-Cache misses | perf | +10% | Warning |
| L3 misses | perf | +10% | Warning |

### 10.2 Regression Tests

```rust
#[test]
fn test_no_performance_regression() {
    let baseline = load_baseline("main");
    let current = run_benchmarks();
    
    for (name, baseline_time) in baseline {
        let current_time = current.get(&name).unwrap();
        let change = (current_time - baseline_time) / baseline_time;
        
        assert!(
            change < 0.10,
            "Performance regression in {}: {:.1}%",
            name,
            change * 100.0
        );
    }
}
```

---

## 11. Implementation Timeline

```
Week 1-2:  P0 Optimizations
           ├── Ring buffer HugePage
           ├── Wallet Guard inline
           └── Cache padding
           
Week 2-4:  P1 Optimizations
           ├── Lazy init
           ├── Memory pooling
           └── PGO setup
           
Week 4-8:  P2 Optimizations
           ├── Parallel parsing
           ├── SIMD search
           └── Vector indexing
           
Week 8-12: P3 Optimizations
           ├── Incremental render
           ├── I/O batching
           └── Buffer pooling
           
Week 12+:  P4 Optimizations
           ├── BOLT integration
           ├── SIMD expansion
           └── Cache tuning
```

---

## 12. Compliance Checklist

| Item | Status | Notes |
|------|--------|-------|
| P0 optimizations defined | Yes | Section 3 |
| P1 optimizations defined | Yes | Section 4 |
| P2 optimizations defined | Yes | Section 5 |
| P3 optimizations defined | Yes | Section 6 |
| BOLT strategy documented | Yes | Section 7 |
| SIMD opportunities identified | Yes | Section 8 |
| Cache optimization documented | Yes | Section 9 |
| Timeline provided | Yes | Section 11 |

---

**Document Status:** APPROVED  
**Next Review:** After optimization implementation  
**Sign-off:** Performance Engineering Team
