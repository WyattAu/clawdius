# Clawdius Performance Benchmarks

> Measured on the actual library types in release mode (`-O`).
> 1,000,000 iterations per benchmark.
> Date: 2026-04-02

## HFT Critical Path SLOs

All Service Level Objectives are met with significant margin.

| Component | Operation | Latency | SLO Target | Margin |
|-----------|-----------|---------|------------|--------|
| Ring Buffer | `push` (SPSC, cache-padded) | **2 ns** | <100 ns | **50x** |
| Ring Buffer | `pop` (SPSC, cache-padded) | **1 ns** | <100 ns | **100x** |
| Ring Buffer | `push+pop` roundtrip | **2 ns** | <200 ns | **100x** |
| Wallet Guard | `check` (approve order) | **16 ns** | <100 µs | **6,250x** |
| Wallet Guard | `check` (reject order) | **9 ns** | <100 µs | **11,111x** |
| Ring Buffer | init (capacity 4096) | **<1 ns** | — | — |
| Wallet Guard | init (default params) | **<1 ns** | — | — |

## Methodology

- **Framework**: `std::time::Instant` with `black_box` to prevent optimization
- **Build**: `cargo run --release --package clawdius-core --example quick_perf`
- **Platform**: Linux x86_64, Rust 1.93.1
- **Types**: Actual `clawdius_core::broker::ring_buffer::RingBuffer` and `clawdius_core::broker::wallet_guard::WalletGuard`
- **Iterations**: 1,000,000 per benchmark (init: 10,000)

## Standalone Validation (Algorithm-Level)

The standalone `scripts/quick_bench.rs` validates the algorithm patterns independently:

| Operation | Latency | Notes |
|-----------|---------|-------|
| Ring buffer push | 8.56 ns | Standalone implementation |
| Ring buffer pop | 9.66 ns | Standalone implementation |
| Wallet guard hash insert | 45.07 ns | HashSet-based restricted symbol check |
| Wallet guard restricted check | 14.79 ns | Symbol lookup in restricted set |

## Criterion Benchmarks (Extended)

The project includes 8 Criterion benchmark suites for detailed profiling:

| Suite | File | Coverage |
|-------|------|----------|
| WCET | `benches/wcet_bench.rs` | Ring buffer, WalletGuard, signal-to-risk pipeline |
| HFT | `benches/hft_bench.rs` | Full HFT pipeline, boot simulation, throughput |
| Core | `benches/core_bench.rs` | Session store, context mentions, JSON-RPC, diffs, token counting |
| Messaging | `benches/messaging_bench.rs` | Command parsing, routing, rate limiting, chunking |
| Session | `benches/session_benchmark.rs` | Session CRUD, message operations, persistence |
| Tools | `benches/tools_benchmark.rs` | File read/write/list operations |
| LLM | `benches/llm_benchmark.rs` | Message creation, serialization |
| CLI | `benches/cli_bench.rs` | CLI parsing, output formatting, TUI components |

Run extended benchmarks:
```bash
cargo bench --package clawdius-core
cargo bench --package clawdius
```

## Architecture

The HFT data path uses lock-free SPSC ring buffers with cache-padded atomics
(`crossbeam_utils::CachePadded`) to prevent false sharing between head/tail
pointers. The WalletGuard implements SEC 15c3-5 position limit checks using
constant-time hash lookups for restricted symbol lists.
