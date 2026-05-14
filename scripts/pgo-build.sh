#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOLCHAIN="nightly"
PROFDATA_DIR="$REPO_ROOT/target/pgo-profdata"
WORKLOAD="${PGO_WORKLOAD:-bench}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info()  { echo -e "${GREEN}[pgo]${NC} $*"; }
warn()  { echo -e "${YELLOW}[pgo]${NC} $*"; }
error() { echo -e "${RED}[pgo]${NC} $*" >&2; }

check_nightly() {
    if ! rustup toolchain list | grep -q "$TOOLCHAIN"; then
        warn "Nightly toolchain not found, installing..."
        rustup toolchain install "$TOOLCHAIN"
    fi
    local ver
    ver=$(rustup run "$TOOLCHAIN" rustc --version)
    info "Using $ver"
}

check_pgo_support() {
    if rustup run "$TOOLCHAIN" rustc -C help 2>&1 | grep -q "profile-generate"; then
        info "PGO supported by current nightly toolchain"
        return 0
    else
        error "PGO NOT supported: -C profile-generate not available"
        return 1
    fi
}

step_instrument() {
    info "Step 1/3: Building instrumented binary..."
    rm -rf "$PROFDATA_DIR"
    mkdir -p "$PROFDATA_DIR"
    CARGO_INCREMENTAL=0 \
    RUSTFLAGS="-C profile-generate=$PROFDATA_DIR" \
    rustup run "$TOOLCHAIN" \
        cargo build --profile pgo-instrument --workspace
    info "Instrumented build complete"
}

step_workload() {
    info "Step 2/3: Running workload to collect profiling data..."
    case "$WORKLOAD" in
        bench)
            cargo bench -p clawdius-core 2>/dev/null || warn "Benchmarks failed or not found, profiling data may be incomplete"
            ;;
        test)
            cargo test --workspace 2>/dev/null || warn "Tests failed, profiling data may be incomplete"
            ;;
        *)
            warn "Unknown workload '$WORKLOAD'. Use PGO_WORKLOAD=bench|test"
            ;;
    esac
    local profdata_count
    profdata_count=$(find "$PROFDATA_DIR" -name "*.profraw" | wc -l)
    info "Collected $profdata_count .profraw files"
}

step_optimize() {
    info "Step 3/3: Building PGO-optimized binary..."
    local merged="$PROFDATA_DIR/merged.profdata"
    rustup run "$TOOLCHAIN" llvm-profdata merge -sparse "$PROFDATA_DIR"/*.profraw -o "$merged"
    CARGO_INCREMENTAL=0 \
    RUSTFLAGS="-C profile-use=$merged" \
    rustup run "$TOOLCHAIN" \
        cargo build --profile pgo-optimized --workspace
    info "PGO-optimized build complete"
}

usage() {
    cat <<'EOF'
Usage: pgo-build.sh [step]

Steps:
  instrument   Build with PGO instrumentation only
  workload     Run benchmark workload (default) to collect profiling data
  optimize     Merge profiles and build optimized binary
  full         Run all three steps (default)

Environment:
  PGO_WORKLOAD  Workload type: bench (default) or test
  PGO_PACKAGE   Specific package to build (default: --workspace)

Examples:
  ./scripts/pgo-build.sh full
  PGO_WORKLOAD=test ./scripts/pgo-build.sh full
  ./scripts/pgo-build.sh instrument
EOF
}

main() {
    cd "$REPO_ROOT"
    local step="${1:-full}"
    case "$step" in
        instrument|workload|optimize|full) ;;
        -h|--help|help) usage; exit 0 ;;
        *) error "Unknown step: $step"; usage; exit 1 ;;
    esac

    check_nightly
    check_pgo_support

    case "$step" in
        full)
            step_instrument
            step_workload
            step_optimize
            ;;
        instrument) step_instrument ;;
        workload)   step_workload   ;;
        optimize)   step_optimize   ;;
    esac
}

main "$@"
