# Benchmark Suite Design

## Document Information

| Attribute | Value |
|-----------|-------|
| **Document ID** | PERF-BENCH-001 |
| **Version** | 1.0.0 |
| **Phase** | 4 (Performance Engineering) |
| **Status** | APPROVED |
| **Created** | 2026-03-01 |
| **Classification** | Performance Specification |

---

## 1. Executive Summary

This document defines the benchmark suite for Clawdius using the `criterion` framework. The suite provides:

- Micro-benchmarks for individual operations
- Integration benchmarks for component interactions
- Load benchmarks for system capacity
- HFT-specific benchmarks for deterministic latency verification

All benchmarks integrate with CI/CD for regression detection.

---

## 2. Benchmark Framework

### 2.1 Tool Selection

| Tool | Purpose | Rationale |
|------|---------|-----------|
| `criterion` | Micro-benchmarks | Statistical analysis, regression detection |
| `iai` | Call-count benchmarks | Cache-agnostic, CI-friendly |
| `divan` | Alternative micro-benchmarks | Compile-time separation |
| `hyperfine` | CLI benchmarks | External process timing |
| `perf` | CPU profiling | Hardware counters |
| `flamegraph` | Visualization | Call graph hotspots |

### 2.2 Criterion Configuration

```rust
// benches/config.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn configure_criterion() -> Criterion {
    Criterion::default()
        .sample_size(1000)
        .measurement_time(std::time::Duration::from_secs(10))
        .warm_up_time(std::time::Duration::from_secs(1))
        .confidence_level(0.99)
        .significance_level(0.01)
        .with_plots()
}
```

### 2.3 Benchmark Categories

| Category | Tool | Frequency | Purpose |
|----------|------|-----------|---------|
| Micro | criterion | Every PR | Unit-level performance |
| Integration | criterion | Every PR | Component interaction |
| Load | custom | Nightly | Capacity planning |
| HFT | criterion + perf | Every PR | Latency verification |
| End-to-end | hyperfine | Release | User experience |

---

## 3. Micro-Benchmarks

### 3.1 FSM Benchmarks

```rust
// benches/fsm_bench.rs
use criterion::{black_box, criterion_group, Criterion};
use clawdius_nexus::fsm::{NexusFSM, Phase};

fn fsm_phase_transition(c: &mut Criterion) {
    let mut group = c.benchmark_group("fsm");
    
    group.bench_function("phase_0_to_1", |b| {
        let mut fsm = NexusFSM::new();
        b.iter(|| {
            fsm.transition(black_box(Phase::Discovery));
        });
    });
    
    group.bench_function("full_cycle_24_phases", |b| {
        b.iter(|| {
            let mut fsm = NexusFSM::new();
            for _ in 0..24 {
                fsm.advance(black_box(()));
            }
        });
    });
    
    group.finish();
}

criterion_group!(fsm_benches, fsm_phase_transition);
```

**Targets:**
| Benchmark | Target | P99 Target |
|-----------|--------|------------|
| `phase_0_to_1` | < 1µs | < 5µs |
| `full_cycle_24_phases` | < 50µs | < 100µs |

### 3.2 Ring Buffer Benchmarks

```rust
// benches/ring_buffer_bench.rs
use criterion::{black_box, criterion_group, Criterion};
use clawdius_broker::ring_buffer::RingBuffer;

fn ring_buffer_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer");
    group.throughput(criterion::Throughput::Elements(1));
    
    let buffer: RingBuffer<MarketData> = RingBuffer::new(1024);
    
    group.bench_function("write_single", |b| {
        let data = MarketData::default();
        b.iter(|| {
            buffer.push(black_box(data)).unwrap();
        });
    });
    
    group.bench_function("read_single", |b| {
        let data = MarketData::default();
        buffer.push(data).unwrap();
        b.iter(|| {
            buffer.pop(black_box())
        });
    });
    
    group.bench_function("write_read_roundtrip", |b| {
        let data = MarketData::default();
        b.iter(|| {
            buffer.push(black_box(data)).unwrap();
            buffer.pop(black_box())
        });
    });
    
    group.finish();
}

criterion_group!(ring_buffer_benches, ring_buffer_operations);
```

**Targets:**
| Benchmark | Target | P99 Target |
|-----------|--------|------------|
| `write_single` | < 100ns | < 200ns |
| `read_single` | < 100ns | < 200ns |
| `write_read_roundtrip` | < 200ns | < 400ns |

### 3.3 Wallet Guard Benchmarks

```rust
// benches/wallet_guard_bench.rs
use criterion::{black_box, criterion_group, Criterion};
use clawdius_broker::wallet_guard::{WalletGuard, Wallet, RiskParameters, Order};

fn wallet_guard_checks(c: &mut Criterion) {
    let mut group = c.benchmark_group("wallet_guard");
    
    let wallet = Wallet::default();
    let params = RiskParameters::default();
    let guard = WalletGuard::new(wallet, params);
    let order = Order::default();
    
    group.bench_function("full_validation", |b| {
        b.iter(|| {
            guard.validate(black_box(&order))
        });
    });
    
    group.bench_function("position_check_only", |b| {
        b.iter(|| {
            guard.check_position_limit(black_box(&order))
        });
    });
    
    group.bench_function("margin_check_only", |b| {
        b.iter(|| {
            guard.check_margin(black_box(&order))
        });
    });
    
    group.finish();
}

criterion_group!(wallet_guard_benches, wallet_guard_checks);
```

**Targets:**
| Benchmark | Target | P99 Target |
|-----------|--------|------------|
| `full_validation` | < 100µs | < 200µs |
| `position_check_only` | < 10µs | < 20µs |
| `margin_check_only` | < 50µs | < 100µs |

### 3.4 Sandbox Benchmarks

```rust
// benches/sandbox_bench.rs
use criterion::{black_box, criterion_group, Criterion};
use clawdius_sentinel::{SandboxManager, SandboxTier};

fn sandbox_spawn(c: &mut Criterion) {
    let mut group = c.benchmark_group("sandbox_spawn");
    
    let manager = SandboxManager::new();
    
    group.bench_function("spawn_tier_1_readonly", |b| {
        b.iter(|| {
            let sandbox = manager.spawn(black_box(SandboxTier::ReadOnly))?;
            sandbox.cleanup();
            Ok::<(), ()>(())
        });
    });
    
    group.bench_function("spawn_tier_2_network", |b| {
        b.iter(|| {
            let sandbox = manager.spawn(black_box(SandboxTier::Network))?;
            sandbox.cleanup();
            Ok::<(), ()>(())
        });
    });
    
    group.bench_function("spawn_tier_3_write", |b| {
        b.iter(|| {
            let sandbox = manager.spawn(black_box(SandboxTier::Write))?;
            sandbox.cleanup();
            Ok::<(), ()>(())
        });
    });
    
    group.bench_function("spawn_tier_4_full", |b| {
        b.iter(|| {
            let sandbox = manager.spawn(black_box(SandboxTier::Full))?;
            sandbox.cleanup();
            Ok::<(), ()>(())
        });
    });
    
    group.finish();
}

criterion_group!(sandbox_benches, sandbox_spawn);
```

**Targets:**
| Benchmark | Target | P99 Target |
|-----------|--------|------------|
| `spawn_tier_1_readonly` | < 50ms | < 75ms |
| `spawn_tier_2_network` | < 75ms | < 100ms |
| `spawn_tier_3_write` | < 100ms | < 150ms |
| `spawn_tier_4_full` | < 150ms | < 200ms |

### 3.5 WASM RPC Benchmarks

```rust
// benches/wasm_rpc_bench.rs
use criterion::{black_box, criterion_group, Criterion};
use clawdius_brain::{WasmRuntime, RpcRequest};

fn wasm_rpc(c: &mut Criterion) {
    let mut group = c.benchmark_group("wasm_rpc");
    
    let runtime = WasmRuntime::new();
    let request = RpcRequest::ping();
    
    group.bench_function("rpc_roundtrip_ping", |b| {
        b.iter(|| {
            runtime.call(black_box(&request))
        });
    });
    
    group.bench_function("rpc_roundtrip_chat", |b| {
        let request = RpcRequest::chat("Hello, world!");
        b.iter(|| {
            runtime.call(black_box(&request))
        });
    });
    
    group.bench_function("rpc_roundtrip_code", |b| {
        let request = RpcRequest::code("fn main() {}");
        b.iter(|| {
            runtime.call(black_box(&request))
        });
    });
    
    group.finish();
}

criterion_group!(wasm_rpc_benches, wasm_rpc);
```

**Targets:**
| Benchmark | Target | P99 Target |
|-----------|--------|------------|
| `rpc_roundtrip_ping` | < 100µs | < 500µs |
| `rpc_roundtrip_chat` | < 1ms | < 2ms |
| `rpc_roundtrip_code` | < 1ms | < 2ms |

---

## 4. Integration Benchmarks

### 4.1 Graph-RAG Benchmarks

```rust
// benches/graph_rag_bench.rs
use criterion::{black_box, criterion_group, Criterion};
use clawdius_graph::{GraphRAG, Query};

fn graph_rag_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_rag");
    
    let rag = GraphRAG::open("test_repo.db").unwrap();
    
    group.bench_function("parse_10k_files", |b| {
        b.iter(|| {
            rag.parse_repository(black_box("test_repos/medium/"))
        });
    });
    
    group.bench_function("semantic_search", |b| {
        let query = Query::semantic("error handling pattern");
        b.iter(|| {
            rag.search(black_box(&query))
        });
    });
    
    group.bench_function("ast_lookup", |b| {
        let query = Query::ast("fn main");
        b.iter(|| {
            rag.search(black_box(&query))
        });
    });
    
    group.bench_function("cross_file_ref", |b| {
        let query = Query::cross_file("struct Config");
        b.iter(|| {
            rag.search(black_box(&query))
        });
    });
    
    group.finish();
}

criterion_group!(graph_rag_benches, graph_rag_operations);
```

**Targets:**
| Benchmark | Target | P99 Target |
|-----------|--------|------------|
| `parse_10k_files` | < 5s | < 8s |
| `semantic_search` | < 50ms | < 100ms |
| `ast_lookup` | < 10ms | < 20ms |
| `cross_file_ref` | < 100ms | < 200ms |

### 4.2 End-to-End Chat Benchmarks

```rust
// benches/e2e_chat_bench.rs
use criterion::{black_box, criterion_group, Criterion};
use clawdius::{ClawdiusApp, ChatRequest};

fn e2e_chat(c: &mut Criterion) {
    let mut group = c.benchmark_group("e2e_chat");
    
    let app = ClawdiusApp::new().await;
    
    group.bench_function("simple_query", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let request = ChatRequest::new("What is Rust?");
            app.chat(black_box(request)).await
        });
    });
    
    group.bench_function("code_generation", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let request = ChatRequest::new("Write a function to reverse a string");
            app.chat(black_box(request)).await
        });
    });
    
    group.bench_function("multi_turn_conversation", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
            let mut session = app.new_session();
            session.chat("What is async?").await;
            session.chat("How does it differ from threads?").await;
            session.chat("Show me an example").await
        });
    });
    
    group.finish();
}

criterion_group!(e2e_chat_benches, e2e_chat);
```

**Targets:**
| Benchmark | Target | P99 Target |
|-----------|--------|------------|
| `simple_query` | < 2s | < 5s |
| `code_generation` | < 5s | < 10s |
| `multi_turn_conversation` | < 15s | < 30s |

### 4.3 HFT Signal Pipeline Benchmarks

```rust
// benches/hft_pipeline_bench.rs
use criterion::{black_box, criterion_group, Criterion};
use clawdius_broker::{HftPipeline, MarketData, Signal};

fn hft_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("hft_pipeline");
    
    let pipeline = HftPipeline::new();
    let market_data = MarketData::default();
    
    group.bench_function("market_data_ingestion", |b| {
        b.iter(|| {
            pipeline.ingest(black_box(market_data))
        });
    });
    
    group.bench_function("signal_generation", |b| {
        b.iter(|| {
            pipeline.generate_signal(black_box(&market_data))
        });
    });
    
    group.bench_function("full_pipeline", |b| {
        b.iter(|| {
            pipeline.process(black_box(market_data))
        });
    });
    
    group.bench_function("burst_1000_messages", |b| {
        b.iter(|| {
            for i in 0..1000 {
                pipeline.ingest(black_box(MarketData { seq: i, ..Default::default() }));
            }
            pipeline.drain()
        });
    });
    
    group.finish();
}

criterion_group!(hft_pipeline_benches, hft_pipeline);
```

**Targets:**
| Benchmark | Target | P99 Target |
|-----------|--------|------------|
| `market_data_ingestion` | < 1µs | < 2µs |
| `signal_generation` | < 200µs | < 500µs |
| `full_pipeline` | < 1ms | < 2ms |
| `burst_1000_messages` | < 1ms | < 2ms |

---

## 5. Load Benchmarks

### 5.1 Repository Parsing Load Test

```rust
// benches/load/repo_parse_load.rs
use std::time::{Duration, Instant};

pub fn repo_parse_load_test() -> LoadTestResult {
    let rag = GraphRAG::open("load_test.db").unwrap();
    
    let start = Instant::now();
    let mut latencies = Vec::new();
    
    for repo in &["small", "medium", "large", "huge"] {
        let repo_start = Instant::now();
        rag.parse_repository(format!("test_repos/{}/", repo));
        latencies.push(repo_start.elapsed());
    }
    
    LoadTestResult {
        total_time: start.elapsed(),
        latencies,
        throughput: 10000.0 / start.elapsed().as_secs_f64(),
    }
}

#[derive(Debug)]
pub struct LoadTestResult {
    pub total_time: Duration,
    pub latencies: Vec<Duration>,
    pub throughput: f64,
}
```

**Targets:**
| Repository Size | File Count | Target | Peak Memory |
|-----------------|------------|--------|-------------|
| Small | 100 | < 100ms | 10 MB |
| Medium | 1,000 | < 500ms | 30 MB |
| Large | 10,000 | < 5s | 100 MB |
| Huge | 100,000 | < 60s | 500 MB |

### 5.2 Concurrent Sandbox Load Test

```rust
// benches/load/concurrent_sandbox_load.rs
use tokio::task::JoinSet;

pub async fn concurrent_sandbox_load(concurrency: usize) -> LoadTestResult {
    let manager = SandboxManager::new();
    let start = Instant::now();
    
    let mut tasks = JoinSet::new();
    for _ in 0..concurrency {
        let manager = manager.clone();
        tasks.spawn(async move {
            let sandbox = manager.spawn(SandboxTier::ReadOnly).unwrap();
            sandbox.execute("ls -la").await.unwrap();
            sandbox.cleanup();
        });
    }
    
    while tasks.join_next().await.is_some() {}
    
    LoadTestResult {
        total_time: start.elapsed(),
        throughput: concurrency as f64 / start.elapsed().as_secs_f64(),
        ..Default::default()
    }
}
```

**Targets:**
| Concurrency | Target Time | Throughput |
|-------------|-------------|------------|
| 4 sandboxes | < 500ms | 8/s |
| 16 sandboxes | < 2s | 8/s |
| 32 sandboxes | < 4s | 8/s |
| 64 sandboxes | < 8s | 8/s |

### 5.3 Market Data Throughput Test

```rust
// benches/load/market_data_load.rs
use std::sync::atomic::{AtomicU64, Ordering};

pub fn market_data_throughput_test(duration: Duration) -> ThroughputResult {
    let buffer: RingBuffer<MarketData> = RingBuffer::new(1_048_576);
    let counter = AtomicU64::new(0);
    
    let start = Instant::now();
    let producer = std::thread::spawn(|| {
        let data = MarketData::default();
        while start.elapsed() < duration {
            buffer.push(data).unwrap();
            counter.fetch_add(1, Ordering::Relaxed);
        }
    });
    
    let consumer_counter = AtomicU64::new(0);
    let consumer = std::thread::spawn(|| {
        while start.elapsed() < duration {
            if buffer.pop().is_some() {
                consumer_counter.fetch_add(1, Ordering::Relaxed);
            }
        }
    });
    
    producer.join().unwrap();
    consumer.join().unwrap();
    
    ThroughputResult {
        messages_produced: counter.load(Ordering::Relaxed),
        messages_consumed: consumer_counter.load(Ordering::Relaxed),
        duration: start.elapsed(),
        throughput: counter.load(Ordering::Relaxed) as f64 / duration.as_secs_f64(),
    }
}
```

**Targets:**
| Duration | Target Throughput | Loss Rate |
|----------|-------------------|-----------|
| 10s | > 5M msg/s | < 0.001% |
| 60s | > 5M msg/s | < 0.001% |
| 300s | > 5M msg/s | < 0.001% |

---

## 6. CI/CD Integration

### 6.1 GitHub Actions Configuration

```yaml
# .github/workflows/benchmarks.yml
name: Benchmarks

on:
  pull_request:
    branches: [main]
  schedule:
    - cron: '0 2 * * *'  # Nightly at 2 AM

jobs:
  micro-benchmarks:
    runs-on: self-hosted
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run micro-benchmarks
        run: cargo bench --bench fsm_bench --bench ring_buffer_bench --bench wallet_guard_bench
      - name: Check regressions
        run: |
          cargo critcmp main...HEAD --threshold 5%
  
  integration-benchmarks:
    runs-on: self-hosted
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run integration benchmarks
        run: cargo bench --bench graph_rag_bench --bench e2e_chat_bench
      - name: Upload results
        uses: actions/upload-artifact@v4
        with:
          name: bench-results
          path: target/criterion/

  hft-benchmarks:
    runs-on: self-hosted-hft
    steps:
      - uses: actions/checkout@v4
      - name: Configure isolated cores
        run: sudo setisolatedcores 0-3
      - name: Run HFT benchmarks
        run: cargo bench --bench hft_pipeline_bench
      - name: Check latency bounds
        run: |
          if grep -q "EXCEEDED" hft_results.txt; then
            echo "HFT latency bounds exceeded"
            exit 1
          fi
```

### 6.2 Regression Detection

```rust
// benches/regression_check.rs
use criterion::BenchmarkId;

pub fn check_regression(current: f64, baseline: f64, threshold: f64) -> Result<(), RegressionError> {
    let change = (current - baseline) / baseline * 100.0;
    
    if change > threshold {
        return Err(RegressionError {
            current,
            baseline,
            change_pct: change,
            threshold,
        });
    }
    
    Ok(())
}

#[derive(Debug)]
pub struct RegressionError {
    pub current: f64,
    pub baseline: f64,
    pub change_pct: f64,
    pub threshold: f64,
}
```

### 6.3 Baseline Management

| Baseline Type | Update Frequency | Storage |
|---------------|------------------|---------|
| Main branch | Every merge | Git LFS |
| Release | Every release | Git tag |
| Nightly | Daily | S3 |
| HFT | Weekly | S3 + audit |

---

## 7. Benchmark Execution

### 7.1 Local Execution

```bash
# Run all micro-benchmarks
cargo bench -- micro

# Run specific benchmark
cargo bench -- ring_buffer_write_single

# Run with profiling
cargo bench -- --profile-time 10 ring_buffer

# Compare against baseline
cargo bench -- --baseline main ring_buffer

# Save as new baseline
cargo bench -- --save-baseline new_feature ring_buffer
```

### 7.2 CI Execution

| Trigger | Benchmarks | Threshold | Action |
|---------|------------|-----------|--------|
| PR | Micro + Integration | 5% | Comment on PR |
| Merge to main | All | 5% | Update baseline |
| Nightly | All + Load | 10% | Create issue |
| Release | All + PGO | 2% | Block release |

### 7.3 HFT-Specific Execution

```bash
# Run on isolated cores
sudo cset proc --exec isolated -- cargo bench -- hft

# With perf counters
perf stat -e cycles,instructions,cache-misses cargo bench -- hft

# Generate flamegraph
cargo bench -- --profile-time 30 hft && flamegraph target/criterion/profile.svg
```

---

## 8. Reporting

### 8.1 Benchmark Report Format

```markdown
# Benchmark Report: [Component]

## Summary
- Baseline: [commit]
- Current: [commit]
- Date: [timestamp]

## Results

| Benchmark | Baseline | Current | Change | Status |
|-----------|----------|---------|--------|--------|
| [name] | [value] | [value] | [%] | [PASS/FAIL] |

## Details
[Detailed analysis]
```

### 8.2 Dashboard Metrics

| Metric | Visualization | Update Frequency |
|--------|---------------|------------------|
| Latency P50 | Line chart | Per PR |
| Latency P99 | Line chart | Per PR |
| Throughput | Line chart | Per PR |
| Memory | Bar chart | Per PR |
| HFT latency | Heatmap | Per PR |

---

## 9. Compliance Checklist

| Item | Status | Notes |
|------|--------|-------|
| Micro-benchmarks defined | Yes | Section 3 |
| Integration benchmarks defined | Yes | Section 4 |
| Load benchmarks defined | Yes | Section 5 |
| CI/CD integration documented | Yes | Section 6 |
| Regression thresholds defined | Yes | Section 6.2 |
| Execution instructions provided | Yes | Section 7 |
| Reporting format defined | Yes | Section 8 |

---

**Document Status:** APPROVED  
**Next Review:** After benchmark implementation  
**Sign-off:** Performance Engineering Team
