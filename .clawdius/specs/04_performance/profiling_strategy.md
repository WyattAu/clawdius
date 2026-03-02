# Profiling Strategy

## Document Information

| Attribute | Value |
|-----------|-------|
| **Document ID** | PERF-PROF-001 |
| **Version** | 1.0.0 |
| **Phase** | 4 (Performance Engineering) |
| **Status** | APPROVED |
| **Created** | 2026-03-01 |
| **Classification** | Performance Specification |

---

## 1. Executive Summary

This document defines the profiling methodology for Clawdius, covering:

- CPU profiling with `perf` and flamegraphs
- Memory profiling with valgrind and custom allocators
- I/O profiling with system tools
- HFT-specific profiling with hardware counters

All profiling integrates with CI/CD for continuous monitoring.

---

## 2. CPU Profiling

### 2.1 perf Configuration

```bash
# Enable perf for user space
sudo sysctl -w kernel.perf_event_paranoid=-1
sudo sysctl -w kernel.kptr_restrict=0

# Profile with perf
perf record -F 99 -g -- ./target/release/clawdius

# Generate report
perf report --stdio

# Generate flamegraph
perf script | stackcollapse-perf.pl | flamegraph.pl > flamegraph.svg
```

### 2.2 perf Events

| Event | Purpose | When to Use |
|-------|---------|-------------|
| `cycles` | CPU cycles | General profiling |
| `instructions` | Instructions retired | IPC analysis |
| `cache-misses` | L1/L2/L3 misses | Cache optimization |
| `cache-references` | Cache accesses | Cache analysis |
| `branch-misses` | Branch mispredictions | Branch optimization |
| `stalled-cycles-frontend` | Fetch stalls | Pipeline analysis |
| `stalled-cycles-backend` | Execute stalls | Memory bound |

### 2.3 perf Profiling Commands

```bash
# Basic CPU profile
perf record -F 99 -g -o perf.data -- ./target/release/clawdius bench

# Cache analysis
perf stat -e cache-misses,cache-references,L1-dcache-load-misses,L1-dcache-loads \
    ./target/release/clawdius bench -- hft

# Branch analysis
perf stat -e branch-misses,branches,instructions \
    ./target/release/clawdius bench -- hft

# IPC analysis
perf stat -e cycles,instructions,ipc \
    ./target/release/clawdius bench -- hft

# Memory bandwidth
perf stat -e dram_reads,dram_writes \
    ./target/release/clawdius bench -- load

# Full hardware counters
perf stat -d -d -d ./target/release/clawdius bench -- hft
```

### 2.4 Flamegraph Generation

```bash
# Install flamegraph tools
cargo install flamegraph

# Generate flamegraph directly
cargo flamegraph --root --bin clawdius -- bench -- hft

# Generate differential flamegraph
perf script | stackcollapse-perf.pl > baseline.folded
# ... make changes ...
perf script | stackcollapse-perf.pl > current.folded
diff-folded.pl baseline.folded current.folded | flamegraph.pl > diff.svg

# Generate cold/hot flamegraph
flamegraph.pl --colors=aqua --bgcolors=grey baseline.folded current.folded > hotcold.svg
```

### 2.5 CPU Profiling Scenarios

| Scenario | Command | Duration | Output |
|----------|---------|----------|--------|
| Boot profile | `perf record -g -- clawdius` | Until ready | boot_flame.svg |
| HFT hot path | `perf record -F 999 -g -a -- clawdius hft` | 30s | hft_flame.svg |
| Graph-RAG parse | `perf record -g -- clawdius parse` | Until done | parse_flame.svg |
| TUI render | `perf record -g -- clawdius tui` | 60s | tui_flame.svg |

---

## 3. Memory Profiling

### 3.1 Valgrind Configuration

```bash
# Install valgrind
sudo apt install valgrind

# Memory leak check
valgrind --leak-check=full \
    --show-leak-kinds=all \
    --track-origins=yes \
    --error-exitcode=1 \
    ./target/debug/clawdius test

# Heap profiling
valgrind --tool=massif \
    --massif-out-file=massif.out \
    ./target/release/clawdius bench

# Heap analysis
ms_print massif.out

# Cache profiling
valgrind --tool=cachegrind \
    --cachegrind-out-file=cachegrind.out \
    ./target/release/clawdius bench

# Cache analysis
cg_annotate cachegrind.out
```

### 3.2 DHAT (Dynamic Heap Analysis Tool)

```bash
# Heap allocation profiling
valgrind --tool=dhat \
    --dhat-out-file=dhat.out \
    ./target/release/clawdius bench

# View with DHAT viewer
# https://valgrind.org/docs/manual/dh-manual.html
```

### 3.3 Heaptrack

```bash
# Install heaptrack
sudo apt install heaptrack

# Profile memory
heaptrack ./target/release/clawdius bench

# Analyze
heaptrack_print heaptrack.clawdius.*.gz
```

### 3.4 Custom Allocator Profiling

```rust
// src/profiling_allocator.rs
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

pub struct ProfilingAllocator {
    allocations: AtomicU64,
    deallocations: AtomicU64,
    bytes_allocated: AtomicU64,
    bytes_freed: AtomicU64,
}

unsafe impl GlobalAlloc for ProfilingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocations.fetch_add(1, Ordering::Relaxed);
        self.bytes_allocated.fetch_add(layout.size() as u64, Ordering::Relaxed);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.deallocations.fetch_add(1, Ordering::Relaxed);
        self.bytes_freed.fetch_add(layout.size() as u64, Ordering::Relaxed);
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static ALLOCATOR: ProfilingAllocator = ProfilingAllocator {
    allocations: AtomicU64::new(0),
    deallocations: AtomicU64::new(0),
    bytes_allocated: AtomicU64::new(0),
    bytes_freed: AtomicU64::new(0),
};

pub fn memory_stats() -> (u64, u64, u64) {
    (
        ALLOCATOR.allocations.load(Ordering::Relaxed),
        ALLOCATOR.deallocations.load(Ordering::Relaxed),
        ALLOCATOR.bytes_allocated.load(Ordering::Relaxed) - 
            ALLOCATOR.bytes_freed.load(Ordering::Relaxed),
    )
}
```

### 3.5 Memory Profiling Scenarios

| Scenario | Tool | Duration | Output |
|----------|------|----------|--------|
| Boot memory | valgrind massif | Until ready | boot_massif.out |
| HFT memory | heaptrack | 60s | hft_heaptrack.gz |
| Graph-RAG memory | DHAT | Until done | parse_dhat.out |
| Leak detection | valgrind memcheck | Full test | leaks.txt |

---

## 4. I/O Profiling

### 4.1 File I/O

```bash
# strace for file operations
strace -f -e trace=openat,read,write,close ./target/release/clawdius bench

# Count file operations
strace -f -c -e trace=openat,read,write,close ./target/release/clawdius bench

# iotop for real-time I/O
sudo iotop -p $(pgrep clawdius)

# iostat for disk statistics
iostat -x 1 60 > iostat.log
```

### 4.2 Network I/O

```bash
# tcpdump for packet capture
sudo tcpdump -i any -w clawdius.pcap port 443 or port 80

# Analyze with wireshark
wireshark clawdius.pcap

# ss for socket statistics
ss -tulnp | grep clawdius

# nethogs for per-process network
sudo nethogs
```

### 4.3 Database I/O

```bash
# SQLite profiling
sqlite3 :memory: "PRAGMA compile_options;"
sqlite3 clawdius.db "PRAGMA stats;"

# LanceDB profiling
# Via application metrics
```

### 4.4 I/O Profiling Scenarios

| Scenario | Tool | Duration | Output |
|----------|------|----------|--------|
| Boot I/O | strace -c | Until ready | boot_io.txt |
| Graph-RAG I/O | iotop | Until done | rag_io.log |
| HFT network | tcpdump | 60s | hft.pcap |
| Database I/O | iostat | 60s | db_io.log |

---

## 5. HFT-Specific Profiling

### 5.1 Hardware Counters

```bash
# Intel PCM (Performance Counter Monitor)
pcm.x ./target/release/clawdius hft

# RDPMC for cycle-accurate timing
# (requires kernel module or direct access)

# PEBS (Precise Event-Based Sampling)
perf record -e cycles:pp -F 999 -- ./target/release/clawdius hft
```

### 5.2 Latency Distribution

```rust
// src/hft/latency_tracker.rs
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub struct LatencyTracker {
    buckets: [AtomicU64; 20], // 0-1µs, 1-2µs, ..., 19µs+
    min: AtomicU64,
    max: AtomicU64,
    sum: AtomicU64,
    count: AtomicU64,
}

impl LatencyTracker {
    pub fn record(&self, latency_ns: u64) {
        let bucket = if latency_ns < 1000 {
            0 // 0-1µs
        } else if latency_ns < 2000 {
            1 // 1-2µs
        } else if latency_ns < 5000 {
            2 // 2-5µs
        } else if latency_ns < 10000 {
            3 // 5-10µs
        } else if latency_ns < 50000 {
            4 // 10-50µs
        } else if latency_ns < 100000 {
            5 // 50-100µs
        } else if latency_ns < 500000 {
            6 // 100-500µs
        } else if latency_ns < 1_000_000 {
            7 // 500µs-1ms
        } else {
            8 // >1ms
        };
        
        self.buckets[bucket].fetch_add(1, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum.fetch_add(latency_ns, Ordering::Relaxed);
        
        // Update min/max
        loop {
            let current_min = self.min.load(Ordering::Relaxed);
            if latency_ns >= current_min || self.min.compare_exchange(
                current_min,
                latency_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ).is_ok() {
                break;
            }
        }
        
        loop {
            let current_max = self.max.load(Ordering::Relaxed);
            if latency_ns <= current_max || self.max.compare_exchange(
                current_max,
                latency_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ).is_ok() {
                break;
            }
        }
    }
    
    pub fn percentile(&self, p: f64) -> u64 {
        let total = self.count.load(Ordering::Relaxed);
        let target = (total as f64 * p / 100.0) as u64;
        
        let mut cumulative = 0u64;
        for (i, bucket) in self.buckets.iter().enumerate() {
            cumulative += bucket.load(Ordering::Relaxed);
            if cumulative >= target {
                return match i {
                    0 => 500,      // 0-1µs -> 500ns median
                    1 => 1500,     // 1-2µs
                    2 => 3500,     // 2-5µs
                    3 => 7500,     // 5-10µs
                    4 => 30000,    // 10-50µs
                    5 => 75000,    // 50-100µs
                    6 => 300000,   // 100-500µs
                    7 => 750000,   // 500µs-1ms
                    _ => 2_000_000, // >1ms
                };
            }
        }
        0
    }
}
```

### 5.3 Cache Analysis

```bash
# perf cache analysis
perf stat -e L1-dcache-loads,L1-dcache-load-misses,L1-dcache-stores \
    -e LLC-loads,LLC-load-misses,LLC-stores \
    -e dTLB-loads,dTLB-load-misses \
    ./target/release/clawdius hft

# cachegrind for cache simulation
valgrind --tool=cachegrind \
    --I1=32768,8,64 \
    --D1=32768,8,64 \
    --LL=8388608,16,64 \
    ./target/release/clawdius hft
```

### 5.4 False Sharing Detection

```rust
// src/hft/false_sharing.rs
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

pub fn detect_false_sharing() {
    let shared = [AtomicU64::new(0), AtomicU64::new(0)];
    
    let start = std::time::Instant::now();
    
    let h1 = thread::spawn(|| {
        for i in 0..1_000_000 {
            shared[0].fetch_add(1, Ordering::Relaxed);
        }
    });
    
    let h2 = thread::spawn(|| {
        for i in 0..1_000_000 {
            shared[1].fetch_add(1, Ordering::Relaxed);
        }
    });
    
    h1.join().unwrap();
    h2.join().unwrap();
    
    let elapsed = start.elapsed();
    println!("False sharing test: {:?}", elapsed);
    // If this is slow, cache-padded version will be ~3-5x faster
}
```

---

## 6. Continuous Profiling

### 6.1 Production Profiling

```rust
// src/profiling.rs
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

pub struct ContinuousProfiler {
    enabled: AtomicBool,
    sample_interval: Duration,
}

impl ContinuousProfiler {
    pub fn start(&self) {
        while self.enabled.load(Ordering::Relaxed) {
            let stats = memory_stats();
            metrics::gauge!("memory_live_bytes", stats.2 as f64);
            metrics::gauge!("memory_allocations", stats.0 as f64);
            
            thread::sleep(self.sample_interval);
        }
    }
    
    pub fn stop(&self) {
        self.enabled.store(false, Ordering::Relaxed);
    }
}
```

### 6.2 CI Profiling Integration

```yaml
# .github/workflows/profiling.yml
name: Profiling

on:
  schedule:
    - cron: '0 3 * * 0'  # Weekly at 3 AM Sunday

jobs:
  cpu-profiling:
    runs-on: self-hosted
    steps:
      - uses: actions/checkout@v4
      - name: CPU Profile
        run: |
          perf record -F 99 -g -o perf.data -- ./target/release/clawdius bench
          perf script | stackcollapse-perf.pl | flamegraph.pl > flamegraph.svg
      - name: Upload flamegraph
        uses: actions/upload-artifact@v4
        with:
          name: flamegraph
          path: flamegraph.svg

  memory-profiling:
    runs-on: self-hosted
    steps:
      - uses: actions/checkout@v4
      - name: Memory Profile
        run: |
          valgrind --tool=massif --massif-out-file=massif.out ./target/release/clawdius bench
          ms_print massif.out > memory_report.txt
      - name: Upload memory report
        uses: actions/upload-artifact@v4
        with:
          name: memory-report
          path: memory_report.txt
```

---

## 7. Profiling Checklist

### 7.1 Pre-Release Profiling

| Check | Tool | Threshold | Action |
|-------|------|-----------|--------|
| No memory leaks | valgrind | 0 bytes | Block release |
| No cache regressions | perf | < 5% | Block release |
| No I/O bottlenecks | iostat | < 10% | Block release |
| HFT latency | custom | < 1ms P99 | Block release |

### 7.2 Regular Profiling

| Check | Frequency | Tool | Action |
|-------|-----------|------|--------|
| CPU hotspots | Weekly | perf + flamegraph | Optimize |
| Memory growth | Weekly | heaptrack | Investigate |
| I/O patterns | Weekly | strace | Optimize |
| HFT latency | Daily | custom | Alert |

---

## 8. Troubleshooting Guide

### 8.1 Common Issues

| Issue | Symptom | Diagnosis | Solution |
|-------|---------|-----------|----------|
| Cache misses | High L3 miss rate | perf cache-misses | Cache-padded structs |
| False sharing | Poor scaling | perf stalled-cycles-backend | Align to 64 bytes |
| Memory leak | Growing RSS | valgrind massif | Fix leak |
| I/O bottleneck | High iowait | iostat | Buffer or async |

### 8.2 Profiling Commands Reference

```bash
# Quick CPU profile
perf record -F 99 -g -- ./target/release/clawdius && perf script | flamegraph.pl > cpu.svg

# Quick memory check
valgrind --leak-check=full ./target/debug/clawdius test

# Quick I/O check
strace -c ./target/release/clawdius bench

# Full HFT profile
perf stat -d -d -d ./target/release/clawdius hft
```

---

## 9. Compliance Checklist

| Item | Status | Notes |
|------|--------|-------|
| CPU profiling documented | Yes | Section 2 |
| Memory profiling documented | Yes | Section 3 |
| I/O profiling documented | Yes | Section 4 |
| HFT profiling documented | Yes | Section 5 |
| Continuous profiling documented | Yes | Section 6 |
| Troubleshooting guide provided | Yes | Section 8 |

---

**Document Status:** APPROVED  
**Next Review:** After profiling implementation  
**Sign-off:** Performance Engineering Team
