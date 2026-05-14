# PGO Pipeline Report — Clawdius

**Date:** 2026-05-14
**Toolchain:** rustc 1.97.0-nightly (ff9a9ea07 2026-05-13)

## PGO Feasibility: SUPPORTED

Nightly rustc supports both `-C profile-generate` and `-C profile-use`.

## Cargo.toml Profiles

Already configured in `Cargo.toml` (lines 208-222):

| Profile | Inherits | LTO | Codegen Units |
|---|---|---|---|
| `pgo-instrument` | release | thin | 1 |
| `pgo-optimized` | release | fat | 1 |

## Build Commands

### Step 1 — Instrumented Build

```bash
CARGO_INCREMENTAL=0 \
RUSTFLAGS="-C profile-generate=target/pgo-profdata" \
rustup run nightly cargo build --profile pgo-instrument --workspace
```

### Step 2 — Collect Profiling Data

```bash
# Run benchmarks
cargo bench -p clawdius-core

# Or run tests
cargo test --workspace

# Or run any representative workload
```

This produces `.profraw` files in `target/pgo-profdata/`.

### Step 3 — Merge & Optimize

```bash
rustup run nightly llvm-profdata merge -sparse \
    target/pgo-profdata/*.profraw \
    -o target/pgo-profdata/merged.profdata

CARGO_INCREMENTAL=0 \
RUSTFLAGS="-C profile-use=target/pgo-profdata/merged.profdata" \
rustup run nightly cargo build --profile pgo-optimized --workspace
```

## Automation Script

`scripts/pgo-build.sh` — full pipeline in one command:

```bash
# Full pipeline (instrument → workload → optimize)
./scripts/pgo-build.sh full

# Individual steps
./scripts/pgo-build.sh instrument
./scripts/pgo-build.sh workload
./scripts/pgo-build.sh optimize

# Use tests instead of benchmarks for profiling workload
PGO_WORKLOAD=test ./scripts/pgo-build.sh full
```

## Issues Encountered

| Issue | Status |
|---|---|
| Nightly toolchain not installed | Resolved — installed via `rustup toolchain install nightly` |
| Existing `scripts/pgo.sh` missing RUSTFLAGS | Replaced by `scripts/pgo-build.sh` which sets `-C profile-generate` / `-C profile-use` correctly |
| Workspace build is slow (~5+ min) | Expected — wasmtime + lancedb dependencies are heavy |
| No issues with PGO flag support | Nightly 1.97.0 supports all required flags |

## Previous Script Notes

The original `scripts/pgo.sh` had several issues:
- Did not use `rustup run nightly` — would fail on stable-only installs
- Did not set `RUSTFLAGS="-C profile-generate=..."` — profiling data would not be generated
- Did not merge `.profraw` files with `llvm-profdata`
- BOLT section referenced incorrect file paths and mixed PGO/BOLT concepts

These are all addressed in the new `scripts/pgo-build.sh`.
