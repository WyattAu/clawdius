# Benchmark Baseline Report

**Date:** 2026-05-14
**Profile:** `bench` (optimized)
**Platform:** Linux x86_64
**Note:** Build takes ~3-7 min per crate due to heavy deps (wasmtime, tree-sitter, tiktoken-rs).

---

## Benchmarks by Crate

### `clawdius-core` (6 benches)

| Bench File | Status | Groups |
|---|---|---|
| `simd_bench` | OK | simd/checksum, simd/hash, simd/checksum_vs_scalar |
| `llm_benchmark` | OK | llm_message_creation, llm_message_serialization, llm_message_collections |
| `core_bench` | OK | session_store, context_mentions, diff_computation, json_rpc_serialization, token_counting |
| `tools_benchmark` | OK | file_read, file_write, file_list, file_read_offset_limit |
| `session_benchmark` | OK | session_create, session_persistence, session_message_operations, session_with_messages, session_list_operations |
| `memory_bench` | **FAILED** | FK constraint error at `save_message` before `create_session` |
| `performance` | **EMPTY** | Runs 0 tests (criterion_group defined but harness skips) |

### `clawdius` (1 bench)

| Bench File | Status | Groups |
|---|---|---|
| `cli_bench` | OK | cli_parsing, output_formatting, tui_components, string_operations |

---

## Results Summary

### SIMD Operations (`clawdius-core::simd`)

| Benchmark | Size | Time | Throughput |
|---|---|---|---|
| fast_checksum | 64B | 184 ns | 331 MiB/s |
| fast_checksum | 1KB | 3.66 us | 267 MiB/s |
| fast_checksum | 64KB | 110 us | 568 MiB/s |
| fast_checksum | 1MB | 3.53 ms | 284 MiB/s |
| fast_hash | 64B | 7.6 ns | 7.8 GiB/s |
| fast_hash | 1KB | 79 ns | 12 GiB/s |
| fast_hash | 64KB | 5.5 us | 11.1 GiB/s |
| fast_hash | 1MB | 105 us | 9.3 GiB/s |
| simd vs scalar checksum | 1KB | 1.2 us vs 1.15 us | scalar slightly faster |
| simd vs scalar checksum | 64KB | 80 us vs 75 us | scalar slightly faster |
| simd vs scalar checksum | 1MB | 1.14 ms vs 1.21 ms | SIMD ~7% faster at 1MB |

### LLM Message Operations (`clawdius-core::llm`)

| Benchmark | Time |
|---|---|
| chat_message_create (simple) | 15 ns |
| chat_message_create (system) | 19 ns |
| chat_message_create (long) | 156 ns |
| message_serialize | 79 ns |
| message_deserialize | 162 ns |
| create_message_vec (10) | 657 ns |
| create_message_vec (100) | 10.3 us |

### Session Store (`clawdius-core`)

| Benchmark | Time |
|---|---|
| create_session | 81 us |
| load_session | 34 us |
| save_message | 135 us |
| load_session_full (100 msgs) | 154 us |
| session_new (in-memory) | 310 ns |
| session_store_create | 156 us |
| session_load | 64 us |
| session_load_full (empty) | 96 us |
| session_save_message | 163 us |
| session_load (10 msgs) | 82 us |
| session_load (100 msgs) | 294 us |
| session_load (1000 msgs) | 1.27 ms |
| list_sessions (10) | 42 us |
| list_sessions (100) | 149 us |

### File Tool Operations (`clawdius-core::tools`)

| Benchmark | Time | Throughput |
|---|---|---|
| file_read/small | 12 us | 12 MiB/s |
| file_read/medium | 13 us | 1.4 GiB/s |
| file_read/large | 12 us | 16 GiB/s |
| file_write/small | 16 us | 9 MiB/s |
| file_write/medium | 17 us | 1.1 GiB/s |
| file_list (100 files) | 7.9 us | - |
| read_offset (first 100) | 7.7 us | - |
| read_offset (middle 100) | 9.2 us | - |
| read_offset (last 100) | 8.0 us | - |

### Context & Diff (`clawdius-core`)

| Benchmark | Time |
|---|---|
| mention_parse/single_file | 2.1 ms |
| mention_parse/multiple | 1.7 ms |
| mention_parse/git | 1.4 ms |
| mention_parse/complex | 1.8 ms |
| diff_compute/small | 589 ns |
| diff_compute/medium | 14 us |
| diff_compute/large | 2.3 ms |

### JSON-RPC Serialization (`clawdius-core`)

| Benchmark | Time |
|---|---|
| request_serialize | 819 ns |
| request_deserialize | 549 ns |
| response_serialize | 587 ns |
| response_deserialize | 1.04 us |

### Token Counting (tiktoken cl100k_base) (`clawdius-core`)

| Benchmark | Time |
|---|---|
| count/small (47 B) | 183 ms |
| count/medium (1.7 KB) | 143 ms |
| count/large (17 KB) | 201 ms |

> **Note:** First-call initialization dominates. Each call re-initializes the tokenizer (~140 ms overhead).

### CLI & TUI (`clawdius`)

| Benchmark | Time |
|---|---|
| cli_parse/minimal | 2.8 us |
| cli_parse/with_flags | 4.4 us |
| cli_parse/with_cwd | 3.9 us |
| cli_parse/full_options | 5.9 us |
| json_serialize_compact | 487 ns |
| json_serialize_pretty | 598 ns |
| json_deserialize | 1.25 us |
| tui_text_styling | 4.8 ns |
| tui_paragraph_creation | 112 ns |
| tui_layout_calculation | 55 ns |
| tui_buffer_rendering (80x24) | 9.4 us |
| text_truncation | 294 ns |
| text_wrapping | 909 ns |

---

## Known Issues

1. **`memory_bench` PANICS** - `memory_bench.rs:97`: calls `save_message` before `create_session`, causing a FOREIGN KEY constraint failure. Fix: swap `create_session` to run before `save_message`.

2. **`performance` bench runs 0 tests** - The `performance.rs` file has a valid `criterion_group!`/`criterion_main!` but the harness reports 0 tests. Likely a Cargo.toml bench registration issue or the binary name collides.

3. **Token counting overhead** - `tiktoken_rs::cl100k_base()` is called inside the hot loop. Should be cached outside `iter()`.

---

## Saved Baselines

Criterion baselines saved to `target/criterion/<bench_group>/<bench_id>/main/` using `--save-baseline main`.

Saved for: `simd_bench`, `llm_benchmark`, `core_bench`, `tools_benchmark`, `session_benchmark`, `cli_bench`.

Not saved: `memory_bench` (crashed), `performance` (no tests).

---

## CI Integration Recommendations

1. **GitHub Actions workflow** - Add a `benchmarks` job triggered on push to `main`:
   ```yaml
   benchmarks:
     runs-on: ubuntu-latest
     steps:
       - uses: actions/checkout@v4
       - uses: dtolnay/rust-toolchain@stable
       - run: cargo bench --workspace -- --save-baseline ci 2>&1 | tee bench-output.txt
       - uses: actions/upload-artifact@v4
         with:
           name: benchmark-results
           path: |
             bench-output.txt
             target/criterion/
   ```

2. **Regression detection** - Compare against saved baselines:
   ```yaml
   - run: cargo bench --workspace -- --baseline ci 2>&1 | tee bench-compare.txt
   ```

3. **Fix failing benches first** - `memory_bench` FK bug and `performance` empty harness should be resolved before CI integration.

4. **Reduce compile time** - Consider a separate `benchmarks` workspace member or feature gate to avoid compiling all deps for every bench run. The current ~3-7 min compile time per crate is expensive for CI.

5. **Cache tokenizer** - Move `tiktoken_rs::cl100k_base()` outside benchmark loops to get realistic token counting numbers.

6. **Historical tracking** - Use `github-action-benchmark` or `bencher.dev` to track results over time and catch regressions automatically.
