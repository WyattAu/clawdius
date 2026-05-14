# Clawdius Cold Start Profile

**Binary:** `clawdius 1.0.0-rc.1`  
**Profile:** `--release` (strip, opt-level 3)  
**Measured:** 2026-05-14  
**Tool:** hyperfine 1.20.0 (`--shell=none`, warmup included)  
**Note:** A `pgo-instrument` build was running concurrently during measurement, which likely introduced the observed statistical outliers. Production numbers may be slightly lower.

## Binary Size

| Metric | Value |
|--------|-------|
| File size | 24.9 MiB (26,173,832 bytes) |
| `.text` section | 25.6 MiB (25,566,563 bytes) |
| `.data` section | 586 KiB (600,320 bytes) |
| `.bss` section | 43 KiB (44,304 bytes) |
| Format | ELF 64-bit PIE, x86-64, dynamically linked, stripped |

## Startup Time (`--version`)

| Statistic | Value |
|-----------|-------|
| Mean | **4.7 ms** |
| Std dev | 2.5 ms |
| Min | 2.0 ms |
| Max | 12.8 ms |
| Runs | 50 (warmup 10) |

## Startup Time (`--help`)

| Statistic | Value |
|-----------|-------|
| Mean | **16.3 ms** |
| Std dev | 18.5 ms |
| Min | 3.5 ms |
| Max | 75.9 ms |
| Runs | 20 (warmup 5) |

The `--help` path is slower due to clap's help rendering (23 subcommands).

## Memory Usage (at `--version` exit)

| Metric | Value |
|--------|-------|
| VmPeak | 25,764 kB (25.2 MiB) |
| VmRSS (at exit) | 24 kB |

VmPeak reflects the virtual address space reserved during process lifetime (lazy mmap of the 25 MiB binary). RSS at exit shows the actual resident pages still mapped -- effectively just the stack page at exit since the binary pages were unmapped after completion.

## Target Comparison

| Metric | Target | Measured | Status |
|--------|--------|----------|--------|
| Cold start (< 500 ms) | 500 ms | **4.7 ms** | PASS (100x under) |
| First-token (< 2 s) | 2,000 ms | N/A (CLI tool) | N/A |

## Observations

1. **Startup is extremely fast** -- 4.7 ms mean is near the floor of process exec overhead on Linux. The binary is largely I/O-bound at this speed (page faults from loading the 25 MiB .text section).
2. **Binary size is 25 MiB** due to wasmtime (Cranelift JIT, Wasm runtime) being a heavy dependency. This is the dominant cost in VmPeak. The `.text` section alone is 25.6 MiB.
3. **First-token latency is not directly measurable** for this CLI tool. The `chat` subcommand's first-token time depends on the configured LLM backend, not the binary itself. To measure first-token latency, a benchmark against a live LLM endpoint would be needed.
4. **Concurrent build noise** -- the PGO-instrument build running during measurement introduced outliers. Re-run on a quiet system for tighter confidence intervals.
