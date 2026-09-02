#!/usr/bin/env bash
# count_tests.sh — Count test functions across the workspace.
#
# Usage:
#   ./scripts/count_tests.sh          # Print total count
#   ./scripts/count_tests.sh --json   # Print per-crate breakdown as JSON

CRATES_DIR="crates"
TESTS_DIR="tests"

count_all() {
    local dir="$1"
    local attr="$2"
    if [ -d "$dir" ]; then
        grep -roh "$attr" "$dir" --include='*.rs' 2>/dev/null | wc -l || echo 0
    else
        echo 0
    fi
}

TEST_COUNT=$(count_all "$CRATES_DIR" '#\[test\]')
TOKIO_COUNT=$(count_all "$CRATES_DIR" '#\[tokio::test\]')
RSTEST_COUNT=$(count_all "$CRATES_DIR" '#\[rstest\]')

WP_TEST=0; WP_TOKIO=0; WP_RSTEST=0
if [ -d "$TESTS_DIR" ]; then
    WP_TEST=$(count_all "$TESTS_DIR" '#\[test\]')
    WP_TOKIO=$(count_all "$TESTS_DIR" '#\[tokio::test\]')
    WP_RSTEST=$(count_all "$TESTS_DIR" '#\[rstest\]')
fi

TOTAL=$((TEST_COUNT + TOKIO_COUNT + WP_TEST + WP_TOKIO))

if [ "${1:-}" = "--json" ]; then
    cat <<EOF
{
  "crates": { "test": $TEST_COUNT, "tokio_test": $TOKIO_COUNT, "rstest": $RSTEST_COUNT },
  "workspace_tests": { "test": $WP_TEST, "tokio_test": $WP_TOKIO, "rstest": $WP_RSTEST },
  "total_explicit": $TOTAL,
  "total_with_rstest_approx": $((TOTAL + RSTEST_COUNT + WP_RSTEST))
}
EOF
else
    echo "Test function count (static analysis):"
    echo "  #[test]:        $((TEST_COUNT + WP_TEST))"
    echo "  #[tokio::test]: $((TOKIO_COUNT + WP_TOKIO))"
    echo "  #[rstest]:      $((RSTEST_COUNT + WP_RSTEST)) (functions, not parameterized cases)"
    echo "  ─────────────────────────────────"
    echo "  Total (explicit): $TOTAL"
    echo "  Total (approx):   $((TOTAL + RSTEST_COUNT + WP_RSTEST))+ (including rstest expansions)"
fi
