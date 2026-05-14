# Memory Profile Report

**Date:** 2026-05-14
**Binary:** clawdius (debug profile)
**Tool:** valgrind massif
**Workload:** `clawdius --help`
**Dockerfile:** `Dockerfile.profile`

## Docker Build Status

| Item | Result |
|------|--------|
| Build | **SUCCESS** |
| Image | `clawdius-profile` |
| Build time | ~7 min (cold), ~0s (cached) |
| Approach | Full Rust debug build in `rust:1.93-bookworm` + valgrind |

## Peak Memory Measurements

| Metric | Value |
|--------|-------|
| Peak heap | 1,736 bytes |
| Peak heap overhead | 32 bytes |
| Total heap peak | 1,768 bytes (~1.7 KiB) |

## Allocation Breakdown (Peak Snapshot #5)

All peak allocations originate from Rust's runtime stack overflow guard initialization:

- **1,024 bytes** — `_IO_file_doallocate` → stdio buffer for `pthread_getattr_np`
- **472 bytes** — `__fopen_internal` → file handle for stack info
- **240 bytes** — `getdelim` → line reading for `/proc/self/maps`

Call chain: `main` → `std::rt::lang_start` → `init` → `install_main_guard` → `stack_start_aligned` → `get_stack_start`

## Binary Sizes

| Profile | Size |
|---------|------|
| Release (stripped) | 25 MiB |
| Debug (with debuginfo) | 470 MiB |

## Recommendations

1. **Startup heap budget:** ~2 KiB — extremely lean. No concerns.
2. **Estimated full RSS at startup:** ~5-10 MiB (code pages, stack, libc, mmap). valgrind only measures heap.
3. **Production profiling:** Re-run with a real workload (`clawdius chat "hello"`) to measure steady-state memory during LLM calls, session management, and tool execution.
4. **Detailed traces:** Use `valgrind --tool=massif --detailed-freq=1 --depth=50` for allocation-site granularity on complex workloads.
5. **Release profiling:** Consider `[profile.release] debug = 1` for line-level massif traces without full debug build overhead.
6. **Leak check:** Run `valgrind --tool=memcheck --leak-check=full` to detect memory leaks after a full session.

## Files

- Profiling Dockerfile: `Dockerfile.profile`
- Profiling script: `scripts/profile-memory.sh`
- This report: `.reports/memory_profile.md`
