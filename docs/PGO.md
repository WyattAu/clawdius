# Profile-Guided Optimization (PGO) + BOLT

## What is PGO?

Profile-Guided Optimization uses runtime profiling data from representative workloads
to inform the compiler's optimization decisions. Instead of relying on static heuristics,
the compiler knows which code paths are actually hot and optimizes them aggressively.

For Clawdius, this means better performance on common operations like LLM request
handling, session management, and tool execution.

## How to run locally

```bash
bash scripts/pgo.sh
```

This runs the full three-step PGO pipeline:

1. **Instrument** — builds `clawdius` with profiling instrumentation
2. **Profile** — runs the benchmark suite to collect real-world usage data
3. **Optimize** — rebuilds using the collected profiles for guided optimization

If BOLT is installed, a fourth step applies post-link binary optimization.

## What BOLT adds

[BOLT](https://github.com/llvm/llvm-project/tree/main/bolt) is a post-link optimizer from
LLVM that operates on the final binary. It provides additional gains on top of PGO by:

- Reordering basic blocks for better instruction cache locality
- Reordering functions to separate hot and cold code
- Splitting functions to reduce icache pressure
- Performing identical code folding (ICF)

BOLT is **optional** — the script skips it if not installed.

## Expected improvements

PGO typically yields **10-30%** improvements on hot paths. Combined with BOLT, expect
additional **5-15%** on top. The biggest gains appear in:

- LLM request/response processing
- Token counting and streaming
- Session state management
- Tool dispatch and result handling

## CI integration

A weekly PGO + BOLT build runs automatically (Sundays at 03:00 UTC) and can also be
triggered manually via workflow dispatch. The optimized binary is uploaded as a
GitHub artifact named `clawdius-pgo-bolt`.
