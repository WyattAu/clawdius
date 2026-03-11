# Clawdius v0.2.0 Release Notes

## Summary
Brief summary of this release - critical fixes and improvements.

## Critical Fixes
1. **Fixed crash.rs compilation** - Removed obsolete `register_panic_handler()` calls from sentry integration
2. **Fixed Rust edition** - Changed from "2024" to "2021" (2024 edition is not stable)
3. **Pinned chrono version** - Fixed compatibility with arrow-arith by pinning chrono to 0.4.39
4. **Updated dependencies** - Resolved HIGH severity CVE (RUSTSEC-2026-0037) by updating quinn-proto

## Security Improvements
- **HIGH CVE Fixed**: quinn-proto 0.11.13 → 0.11.14 (DoS vulnerability, CVSS 8.7)
- Remaining: 2 LOW severity CVEs (transitive from lancedb)

## Build Verification
- All crates compile successfully
- Release binary: 59MB (optimized)
- Test results: 162+ tests passing (timeline, context, telemetry, broker, graph_rag modules)
- CLI smoke tests: PASSING

## Known Issues
- 2 LOW severity CVEs (transitive, cannot fix directly)
- 16 unmaintained dependencies (need replacement planning)
- 54 compiler warnings (mostly unused imports/variables)

## Breaking Changes
- None

## Upgrade Instructions
1. Update with `cargo install clawdius`
2. Or download binary from releases

## Next Steps (v0.3.0)
- Replace unmaintained dependencies
- Reduce compiler warnings
- Add more integration tests
- Performance benchmarks
