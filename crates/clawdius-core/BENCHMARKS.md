# Clawdius Core Benchmark Suite

This directory contains comprehensive benchmarks for measuring performance of critical paths in Clawdius Core.

## Benchmark Files

### 1. `llm_benchmark.rs`
Benchmarks for LLM provider performance (without actual API calls):
- Chat message creation (simple, system, long messages)
- Message serialization/deserialization
- Message collection creation (10, 100 messages)

### 2. `tools_benchmark.rs`
Benchmarks for tool execution performance:
- File read operations (small 1KB, medium 100KB, large 1MB)
- File write operations (small, medium)
- Directory listing (100 files)
- File read with offset/limit (first, middle, last 100 lines)

### 3. `session_benchmark.rs`
Benchmarks for session operations:
- Session creation and persistence
- Message operations (user, assistant, system, long)
- Session loading (10, 100, 1000 messages)
- Session listing operations

### 4. `core_bench.rs` (existing)
Additional benchmarks for:
- Session store operations
- Context mentions parsing
- Diff computation
- JSON-RPC serialization
- Token counting

## Running Benchmarks

### Run all benchmarks:
```bash
cargo bench
```

### Run specific benchmark:
```bash
cargo bench --bench llm_benchmark
cargo bench --bench tools_benchmark
cargo bench --bench session_benchmark
```

### Compile without running:
```bash
cargo bench --no-run
```

## Test Fixtures

Test fixtures are generated dynamically using `tempfile::TempDir` to avoid committing large files to git. The `benches/fixtures/` directory contains:
- `README.md` - Documentation
- `generate.sh` - Optional script for manual fixture generation

## Statistics

- **Total benchmark files:** 4
- **Total benchmark functions:** 45+
- **Total lines of code:** 727
- **No actual API calls:** All LLM operations are mocked
- **No git-tracked fixtures:** All test data generated on-demand

## Implementation Notes

1. **No actual API calls**: All benchmarks use inline data or mocked operations
2. **Temporary files**: File operations use `tempfile::TempDir` for automatic cleanup
3. **Criterion framework**: Using Criterion 0.5 for accurate statistical analysis
4. **Throughput tracking**: Benchmarks include throughput metrics where applicable

## Success Criteria

✅ Cargo.toml configured with benchmark targets  
✅ At least 5 benchmark functions (45+ implemented)  
✅ Three separate benchmark files created  
✅ Test fixtures infrastructure in place  
✅ No actual API calls in benchmarks  
✅ No large test files committed to git  
✅ All benchmarks compile successfully  

## Next Steps

- Run `cargo bench` to execute all benchmarks
- Analyze results to identify performance bottlenecks
- Use results to guide optimization efforts
