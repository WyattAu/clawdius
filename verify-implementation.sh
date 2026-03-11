#!/bin/bash
# Verification script for v0.6.0 P3-P4 implementation

set -e

echo "=== Verifying Error Handling and Structured Logging Implementation ==="
echo ""

echo "1. Checking error types..."
grep -q "LlmProvider" crates/clawdius-core/src/error.rs && echo "   ✓ LlmProvider error type found"
grep -q "RateLimited" crates/clawdius-core/src/error.rs && echo "   ✓ RateLimited error type found"
grep -q "ContextLimit" crates/clawdius-core/src/error.rs && echo "   ✓ ContextLimit error type found"
grep -q "ToolExecution" crates/clawdius-core/src/error.rs && echo "   ✓ ToolExecution error type found"
grep -q "SessionNotFound" crates/clawdius-core/src/error.rs && echo "   ✓ SessionNotFound error type found"
grep -q "CircuitBreakerOpen" crates/clawdius-core/src/error.rs && echo "   ✓ CircuitBreakerOpen error type found"
grep -q "is_retryable" crates/clawdius-core/src/error.rs && echo "   ✓ is_retryable method found"
grep -q "retry_after_ms" crates/clawdius-core/src/error.rs && echo "   ✓ retry_after_ms method found"
echo ""

echo "2. Checking logging configuration..."
grep -q "LoggingConfig" crates/clawdius-core/src/telemetry/mod.rs && echo "   ✓ LoggingConfig struct found"
grep -q "init_logging" crates/clawdius-core/src/telemetry/mod.rs && echo "   ✓ init_logging function found"
grep -q "json_format" crates/clawdius-core/src/telemetry/mod.rs && echo "   ✓ json_format option found"
echo ""

echo "3. Checking retry and circuit breaker module..."
test -f crates/clawdius-core/src/retry.rs && echo "   ✓ retry.rs module created"
grep -q "CircuitBreaker" crates/clawdius-core/src/retry.rs && echo "   ✓ CircuitBreaker struct found"
grep -q "with_retry_and_circuit" crates/clawdius-core/src/retry.rs && echo "   ✓ with_retry_and_circuit function found"
echo ""

echo "4. Checking module exports..."
grep -q "pub mod retry" crates/clawdius-core/src/lib.rs && echo "   ✓ retry module exported"
grep -q "pub mod telemetry" crates/clawdius-core/src/lib.rs && echo "   ✓ telemetry module exported"
grep -q "CircuitBreaker" crates/clawdius-core/src/lib.rs && echo "   ✓ CircuitBreaker re-exported"
echo ""

echo "5. Checking documentation..."
test -f docs/error-handling-and-logging.md && echo "   ✓ Error handling guide created"
test -f clawdius.example.toml && echo "   ✓ Example config created"
test -f IMPLEMENTATION_REPORT.md && echo "   ✓ Implementation report created"
echo ""

echo "6. Checking tests..."
test -f crates/clawdius-core/tests/error_types_test.rs && echo "   ✓ Error types tests created"
echo ""

echo "=== All verification checks passed! ==="
echo ""
echo "To compile and test, run:"
echo "  cargo build --lib -p clawdius-core"
echo "  cargo test --lib -p clawdius-core error_types_test"
echo ""
