# Benchmark Fixtures

This directory contains test fixtures for benchmarking critical paths in Clawdius.

## Files

The following test files are used by the benchmark suite:

- `small.txt` (1KB) - Small file for basic read/write benchmarks
- `medium.txt` (100KB) - Medium file for realistic workload benchmarks  
- `large.txt` (1MB) - Large file for stress testing

## Generation

These files are generated on-demand by the benchmark suite using `tempfile::TempDir`.
They are NOT committed to git to avoid bloating the repository.

## Usage

Benchmarks automatically create temporary test files as needed. No manual setup required.

To run benchmarks:

```bash
cargo bench
```

To compile benchmarks without running:

```bash
cargo bench --no-run
```
