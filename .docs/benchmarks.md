# Benchmark Suite

Performance benchmarks for Clawdius Core using [Criterion.rs](https://bheisler.github.io/criterion.rs/book/).

## Running Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark file
cargo bench --bench core_bench
cargo bench --bench llm_benchmark
cargo bench --bench tools_benchmark
cargo bench --bench session_benchmark
cargo bench --bench hft_bench

# Run specific benchmark group
cargo bench -- "ring_buffer"

# Generate HTML report
cargo bench -- --save-baseline new
```

## Benchmark Files

### core_bench.rs

Core functionality benchmarks covering foundational operations.

| Group | Benchmark | Description |
|-------|-----------|-------------|
| `session_store` | `create_session` | Session creation in SQLite store |
| `session_store` | `load_session` | Session loading by ID |
| `session_store` | `save_message` | Message persistence |
| `session_store` | `load_session_full_100_messages` | Full session with 100 messages |
| `context_mentions` | `parse` | Mention parsing (@file:, @url:, @git:, etc.) |
| `diff_computation` | `compute` | File diff calculation (small/medium/large) |
| `json_rpc_serialization` | `request_serialize/deserialize` | JSON-RPC serialization |
| `json_rpc_serialization` | `response_serialize/deserialize` | JSON-RPC deserialization |
| `token_counting` | `count` | Token counting with tiktoken (cl100k_base) |

### llm_benchmark.rs

LLM-related operations and message handling.

| Group | Benchmark | Description |
|-------|-----------|-------------|
| `llm_message_creation` | `chat_message_create_*` | ChatMessage struct creation |
| `llm_message_serialization` | `message_serialize` | JSON serialization |
| `llm_message_serialization` | `message_deserialize` | JSON deserialization |
| `llm_message_collections` | `create_message_vec_*` | Vector of messages (10/100) |

### tools_benchmark.rs

File tool operations with varying file sizes.

| Group | Benchmark | Description |
|-------|-----------|-------------|
| `file_read` | `read` (small/medium/large) | File reading (20/2000/20000 lines) |
| `file_write` | `write` (small/medium) | File writing operations |
| `file_list` | `list_100_files` | Directory listing |
| `file_read_offset_limit` | `read_*_100_lines` | Offset/limit reads |

### session_benchmark.rs

Session management and persistence.

| Group | Benchmark | Description |
|-------|-----------|-------------|
| `session_create` | `session_new` | In-memory session creation |
| `session_create` | `session_store_create` | Persisted session creation |
| `session_persistence` | `session_load` | Load session metadata |
| `session_persistence` | `session_load_full_empty` | Load empty session with messages |
| `session_message_operations` | `message_create_*` | Message creation (user/assistant/system) |
| `session_with_messages` | `session_load_*_messages` | Load sessions with 10/100/1000 messages |
| `session_list_operations` | `list_sessions_*` | List 10/100 sessions |

### hft_bench.rs

High-frequency trading performance-critical paths.

| Group | Benchmark | Target | Description |
|-------|-----------|--------|-------------|
| `ring_buffer` | `push_single` | <100ns | Single message push |
| `ring_buffer` | `pop_single` | <100ns | Single message pop |
| `ring_buffer` | `push_pop_roundtrip` | <200ns | Push + pop cycle |
| `ring_buffer` | `burst_1000` | - | Burst of 1000 messages |
| `wallet_guard` | `validate_order` | <100µs | Order validation |
| `wallet_guard` | `validate_with_restrictions` | <100µs | With symbol restrictions |
| `hft_pipeline` | `signal_generation` | - | Strategy signal generation |
| `hft_pipeline` | `full_pipeline_signal_to_risk` | <1ms | Full signal-to-risk pipeline |
| `boot_simulation` | `ring_buffer_init` | - | Ring buffer initialization |
| `boot_simulation` | `full_hft_stack_init` | <20ms | Complete HFT stack init |
| `throughput` | `ring_buffer_msg_rate` | - | Message throughput rate |

## Performance Targets

### HFT Critical Paths

| Component | Target | Notes |
|-----------|--------|-------|
| Ring Buffer Operations | <100ns | Lock-free SPSC |
| Wallet Guard Validation | <100µs | Risk checks |
| Full HFT Pipeline | <1ms | Signal to order |
| HFT Stack Boot | <20ms | Cold start |

### General Performance

| Operation | Expected Range |
|-----------|----------------|
| Session create/load | 1-10ms (SQLite) |
| Message save | <1ms |
| Token counting (cl100k) | ~1µs/token |
| File read (medium) | <1ms |
| Diff computation (1000 lines) | 1-5ms |

## Profiling

```bash
# Run with profiling
cargo bench -- --profile-time=5

# Flamegraph (requires flamegraph crate)
cargo flamegraph --bench core_bench -- "session_store"
```

## CI Integration

Benchmarks run in CI to detect performance regressions. Baseline comparisons are stored in `.criterion/` directory.
