# Phase C - Polish Progress

## Summary

**Phase C (Polish) is in progress.** We've made significant progress on LSP integration.

## Completed Tasks

### 1. LSP Client Bug Fix ✅

**Problem:** The LSP reader was incorrectly reading content as lines instead of exact byte counts.

**Root Cause:** After parsing headers, the reader called `lines.next_line()` to read the content. However, LSP content is NOT newline-terminated - it's exactly `content_length` bytes.

**Fix:** Changed the reader to use `read_exact()` to read the precise number of bytes after the headers.

**File:** `crates/clawdius-core/src/lsp/client.rs`

### 2. LSP Timeout Increase ✅

**Problem:** rust-analyzer was timing out at the default 10 second timeout.

**Fix:** Increased default timeout to 30 seconds and added `with_timeout_ms()` method for customization.

### 3. CLI Option Conflict Fix ✅

**Problem:** Both `model` and `mode` options in the `chat` command were trying to use `-m`.

**Fix:** Changed `mode` to use `-M` (capital M).

## Test Results

### LSP Start Command
```
$ ./target/debug/clawdius lsp start rust-analyzer --root file:///home/wyatt/dev/prj/clawdius
✅ LSP server started: rust-analyzer
   Root: file:///home/wyatt/dev/prj/clawdius

   Capabilities:
```

The LSP server now starts successfully and connects to rust-analyzer.

## Remaining Tasks

### 1. Integration Tests for Generate Command
- [ ] Create mock LLM client for testing
- [ ] Add tests for single-pass mode
- [ ] Add tests for iterative mode
- [ ] Add tests for agent mode
- [ ] Add tests for dry-run mode

### 2. Progress Indicators
- [ ] Add spinner for LSP connection
- [ ] Add progress bar for generation steps
- [ ] Add status messages for long operations

### 3. Error Message Polish
- [ ] Improve LSP connection error messages
- [ ] Add suggestions for common errors
- [ ] Add color-coded error levels

### 4. LSP Capabilities Display
- [ ] Fix capabilities parsing to show all available features
- [ ] Add more detailed capability information

## Commits

1. `33b1ab0` - fix(v2.0.0): fix LSP reader and CLI conflicts

## Files Changed

- `crates/clawdius-core/src/lsp/client.rs` - Fixed reader, increased timeout
- `crates/clawdius/src/cli.rs` - Fixed -m conflict, cleaned up orphaned code
