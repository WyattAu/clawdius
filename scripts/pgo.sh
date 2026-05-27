#!/usr/bin/env bash
set -euo pipefail

echo "=== Clawdius PGO Build ==="

# Step 1: Build instrumented binary
echo "Step 1: Build instrumented binary"
cargo build --profile pgo-instrument -p clawdius

# Step 2: Run workload to generate profiling data
echo "Step 2: Run workload to generate profiling data"
BINARY="./target/pgo-instrument/clawdius"

# Exercise CLI hot paths
"$BINARY" --help > /dev/null 2>&1 || true

# Run benchmarks for profiling (best-effort)
cargo bench -p clawdius-core --bench core_bench -- --quick 2>/dev/null || true

# Step 3: Merge profiling data
echo "Step 3: Merge profiling data"
PROFDATA="/tmp/pgo-merged.profdata"
# Try rustup-bundled llvm-profdata first, then system llvm-profdata
PROFDATA_BIN="$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-profdata"
if [ ! -x "$PROFDATA_BIN" ]; then
    PROFDATA_BIN="llvm-profdata"
fi

# Collect raw profile data from instrumented runs
if ls default_*.profraw 1>/dev/null 2>&1; then
    "$PROFDATA_BIN" merge -o "$PROFDATA" default_*.profraw
    echo "Profile data merged: $PROFDATA"
else
    echo "WARNING: No .profraw files found. PGO build will proceed without profile data."
    touch "$PROFDATA"
fi

# Step 4: Build optimized binary using profile data
echo "Step 4: Build PGO-optimized binary"
LLVM_PROFDATA_FILE="$PROFDATA" cargo build --profile pgo-optimized -p clawdius

echo "=== PGO Build Complete ==="
ls -lh target/pgo-optimized/clawdius 2>/dev/null || echo "No optimized binary found"

# Step 5: Optionally apply BOLT post-link optimization
if command -v llvm-bolt &>/dev/null; then
    echo "Step 5: Applying BOLT optimizations..."
    cp target/pgo-optimized/clawdius target/pgo-optimized/clawdius.pre-bolt
    llvm-bolt target/pgo-optimized/clawdius.pre-bolt \
        -o target/pgo-optimized/clawdius \
        --reorder-blocks=ext-tsp \
        --reorder-functions=hfsort \
        --split-functions=2 \
        --split-all-cold \
        --icf=1 \
        --use-gnu-stack 2>/dev/null || echo "BOLT optimization skipped"
    ls -lh target/pgo-optimized/clawdius
else
    echo "Step 5: llvm-bolt not found, skipping post-link optimization"
fi

echo "=== Done ==="
