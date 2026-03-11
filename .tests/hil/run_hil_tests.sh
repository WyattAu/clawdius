#!/bin/bash
#
# Hardware-in-the-Loop (HIL) Test Runner
#
# Usage:
#   run_hil_tests.sh [OPTIONS]
#
# Options:
#   --mock              Run in mock mode (no real hardware)
#   --target TARGET     Specify hardware target (raspberry_pi, jetson_nano, etc.)
#   --category CAT      Run specific test category (unit, integration, stress, safety)
#   --verbose           Enable verbose output
#   --dry-run           Show what would be run without executing
#   --help              Show this help message

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
RESULTS_DIR="$SCRIPT_DIR/results"

# Default options
MOCK_MODE=false
TARGET=""
CATEGORY=""
VERBOSE=false
DRY_RUN=false

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $*"
}

log_success() {
    echo -e "${GREEN}[PASS]${NC} $*"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $*"
}

log_error() {
    echo -e "${RED}[FAIL]${NC} $*"
}

show_help() {
    cat << EOF
Hardware-in-the-Loop (HIL) Test Runner

Usage: $(basename "$0") [OPTIONS]

Options:
  --mock              Run in mock mode (no real hardware) - default for CI
  --target TARGET     Specify hardware target:
                      - raspberry_pi
                      - jetson_nano
                      - stm32
                      - esp32
  --category CAT      Run specific test category:
                      - unit        (hil_unit_*)
                      - integration (hil_integration_*)
                      - stress      (hil_stress_*)
                      - safety      (hil_safety_*)
  --verbose           Enable verbose output
  --dry-run           Show what would be run without executing
  --help              Show this help message

Examples:
  # Run all tests in mock mode (CI)
  $(basename "$0") --mock

  # Run unit tests only
  $(basename "$0") --mock --category unit

  # Run on real hardware
  $(basename "$0") --target raspberry_pi

EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --mock)
                MOCK_MODE=true
                shift
                ;;
            --target)
                TARGET="$2"
                shift 2
                ;;
            --category)
                CATEGORY="$2"
                shift 2
                ;;
            --verbose)
                VERBOSE=true
                shift
                ;;
            --dry-run)
                DRY_RUN=true
                shift
                ;;
            --help)
                show_help
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done
}

check_prerequisites() {
    log_info "Checking prerequisites..."
    
    # Check for Rust
    if ! command -v cargo &> /dev/null; then
        log_error "Cargo not found. Please install Rust."
        exit 1
    fi
    
    # Check for config file
    if [[ ! -f "$SCRIPT_DIR/hil_config.toml" ]]; then
        log_error "Configuration file not found: $SCRIPT_DIR/hil_config.toml"
        exit 1
    fi
    
    log_success "Prerequisites satisfied"
}

setup_environment() {
    log_info "Setting up test environment..."
    
    # Create results directory
    mkdir -p "$RESULTS_DIR"
    
    # Set environment variables
    export HIL_MODE=$([[ "$MOCK_MODE" == true ]] && echo "mock" || echo "hardware")
    export HIL_TARGET="${TARGET:-none}"
    export HIL_CONFIG="$SCRIPT_DIR/hil_config.toml"
    export HIL_RESULTS="$RESULTS_DIR"
    
    if [[ "$VERBOSE" == true ]]; then
        export RUST_LOG=debug
    else
        export RUST_LOG=info
    fi
    
    log_success "Environment configured"
    log_info "  Mode: $HIL_MODE"
    log_info "  Target: $HIL_TARGET"
    log_info "  Config: $HIL_CONFIG"
}

build_tests() {
    log_info "Building HIL tests..."
    
    if [[ "$DRY_RUN" == true ]]; then
        log_info "[DRY-RUN] Would build tests"
        return
    fi
    
    cd "$PROJECT_ROOT"
    
    # Build the mock hardware module
    if ! cargo build --tests 2>&1 | while read -r line; do
        if [[ "$VERBOSE" == true ]]; then
            echo "$line"
        fi
    done; then
        log_error "Build failed"
        exit 1
    fi
    
    log_success "Build complete"
}

run_mock_tests() {
    log_info "Running HIL tests in mock mode..."
    
    local test_filter=""
    case "$CATEGORY" in
        unit)
            test_filter="hil_unit"
            ;;
        integration)
            test_filter="hil_integration"
            ;;
        stress)
            test_filter="hil_stress"
            ;;
        safety)
            test_filter="hil_safety"
            ;;
        *)
            test_filter="hil"
            ;;
    esac
    
    if [[ "$DRY_RUN" == true ]]; then
        log_info "[DRY-RUN] Would run tests matching: $test_filter"
        return
    fi
    
    cd "$PROJECT_ROOT"
    
    local cargo_args=(
        test
        --no-fail-fast
        "$test_filter"
        --test-threads=4
        --report-time
        --color=always
        --
        --test-threads=4
        format=junit
    )
    
    if [[ "$VERBOSE" == true ]]; then
        cargo_args+=(--nocapture)
    fi
    
    # Run tests and capture output
    local start_time
    start_time=$(date +%s)
    
    if cargo "${cargo_args[@]}" 2>&1 | tee "$RESULTS_DIR/test_output.log"; then
        local end_time
        end_time=$(date +%s)
        local duration=$((end_time - start_time))
        log_success "All tests passed in ${duration}s"
        return 0
    else
        local end_time
        end_time=$(date +%s)
        local duration=$((end_time - start_time))
        log_error "Some tests failed after ${duration}s"
        return 1
    fi
}

run_hardware_tests() {
    log_info "Running HIL tests on hardware target: $TARGET"
    
    if [[ -z "$TARGET" ]]; then
        log_error "No hardware target specified. Use --target option."
        exit 1
    fi
    
    # Check if target is enabled in config
    if ! grep -q "^\[hardware.targets.$TARGET\]" "$SCRIPT_DIR/hil_config.toml"; then
        log_error "Unknown target: $TARGET"
        exit 1
    fi
    
    # Check target connectivity (placeholder for real implementation)
    log_info "Checking connectivity to $TARGET..."
    log_warn "Hardware testing not yet implemented"
    log_info "Falling back to mock mode for target simulation"
    
    MOCK_MODE=true
    run_mock_tests
}

generate_report() {
    log_info "Generating test report..."
    
    if [[ "$DRY_RUN" == true ]]; then
        log_info "[DRY-RUN] Would generate report in $RESULTS_DIR"
        return
    fi
    
    # Generate summary
    local report_file="$RESULTS_DIR/summary.txt"
    {
        echo "HIL Test Report"
        echo "==============="
        echo "Generated: $(date -Iseconds)"
        echo "Mode: $([[ "$MOCK_MODE" == true ]] && echo "Mock" || echo "Hardware ($TARGET)")"
        echo ""
        if [[ -f "$RESULTS_DIR/test_output.log" ]]; then
            echo "Test Output:"
            echo "---"
            tail -100 "$RESULTS_DIR/test_output.log"
        fi
    } > "$report_file"
    
    log_success "Report generated: $report_file"
}

cleanup() {
    log_info "Cleaning up..."
    # Add cleanup logic here if needed
}

main() {
    parse_args "$@"
    
    log_info "Starting HIL Test Runner"
    log_info "========================"
    
    check_prerequisites
    setup_environment
    build_tests
    
    local exit_code=0
    
    if [[ "$MOCK_MODE" == true ]]; then
        run_mock_tests || exit_code=$?
    else
        run_hardware_tests || exit_code=$?
    fi
    
    generate_report
    cleanup
    
    if [[ $exit_code -eq 0 ]]; then
        log_success "HIL tests completed successfully"
    else
        log_error "HIL tests completed with failures"
    fi
    
    exit $exit_code
}

# Run main
main "$@"
