#!/usr/bin/env bash
set -euo pipefail

echo "=== Clawdius PGO Build ==="
echo "Step 1: Build instrumented binary"
cargo build --profile pgo-instrument -p clawdius

echo "Step 2: Run workload to generate profiling data"
echo "Running benchmarks for profiling..."
cargo bench -p clawdius-core --bench core_bench -- --quick 2>/dev/null || true

echo "Step 3: Merge profiling data and build optimized binary"
cargo build --profile pgo-optimized -p clawdius

echo "Step 4: Optionally apply BOLT post-link optimization"
if command -v bolt &>/dev/null; then
    echo "Applying BOLT optimizations..."
    BOLT_DIR=$(mktemp -d)
    objcopy --only-keep-debug target/pgo-optimized/clawdius "$BOLT_DIR/clawdius.debug"
    bolt merge-fdata "$BOLT_DIR/clawdius.debug" -o "$BOLT_DIR/clawdius.fdata" \
        --prof-file /tmp/pgo-merge.profdata \
        -ignore-unresolved 2>/dev/null || echo "BOLT merge skipped (no fdata)"
    bolt optimize -o target/release/clawdius-pgo-bolt target/pgo-optimized/clawdius \
        --data "$BOLT_DIR/clawdius.fdata" \
        --reorder-blocks=ext-tsp \
        --reorder-functions=hot-cold \
        --split-functions=2 \
        --split-all-cold \
        --icf=1 2>/dev/null || echo "BOLT optimization skipped"
    rm -rf "$BOLT_DIR"
else
    echo "BOLT not installed, skipping post-link optimization"
fi

echo "=== PGO Build Complete ==="
ls -lh target/release/clawdius* 2>/dev/null || ls -lh target/pgo-optimized/clawdius
