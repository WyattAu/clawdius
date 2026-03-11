# JSON Output Format Implementation Summary

## Changes Made

### 1. New File: `crates/clawdius-core/src/output/formatter.rs`
Created a high-level `OutputFormatter` struct with methods:
- `format_chat_response()` - Formats chat responses in text/JSON/stream-json
- `format_error()` - Formats error messages
- `format_session_list()` - Formats session listings
- `format_tool_result()` - Formats tool execution results

### 2. Modified: `crates/clawdius-core/src/output.rs`
- Added `formatter` module export
- Exported `OutputFormatter`, `SessionInfo`, and additional types

### 3. Modified: `crates/clawdius/src/cli.rs`
- Updated imports to include `OutputFormat`, `OutputFormatter`, `OutputOptions`, `SessionInfo`
- Implemented `ValueEnum` for `OutputFormat` (required for clap)
- Updated `Cli` struct to use `OutputFormat` enum instead of `String`
- Updated `handle_command()` to accept and pass `output_format` parameter
- Updated `handle_chat()` to use `OutputFormatter` for all output formats
- Updated `handle_sessions()` to use `OutputFormatter` and accept `output_format` parameter

### 4. Modified: `crates/clawdius/src/main.rs`
- Extract `output_format` from CLI args
- Pass `output_format` to `handle_command()`

## Output Format Types

### Text (default)
```
Provider: anthropic
Session: session-123

Hello, world!

Tokens: 10 input, 5 output (15 total)
Duration: 1000ms
```

### JSON
```json
{
  "content": "Hello, world!",
  "session_id": "session-123",
  "timestamp": "2026-03-06T...",
  "tool_calls": [],
  "files_changed": [],
  "usage": {
    "input": 10,
    "output": 5,
    "total": 15
  },
  "duration_ms": 1000,
  "success": true
}
```

### Stream-JSON (NDJSON)
```json
{"type":"start","session_id":"session-123","model":"claude-3-5-sonnet","timestamp":"..."}
{"type":"token","content":"Hello "}
{"type":"token","content":"world "}
{"type":"complete","usage":{"input":10,"output":5,"total":15},"duration_ms":1000}
```

## Tests Added

All tests in `crates/clawdius-core/src/output/formatter.rs`:
1. `test_format_chat_response_text` - Text format for chat
2. `test_format_chat_response_json` - JSON format for chat
3. `test_format_chat_response_stream_json` - Stream-JSON format for chat
4. `test_format_error_json` - JSON format for errors
5. `test_format_error_stream_json` - Stream-JSON format for errors
6. `test_format_session_list` - Session listing format

All 6 tests pass successfully.

## Usage Examples

```bash
# Default text output
clawdius chat "Hello"

# JSON output
clawdius chat "Hello" --output-format json

# Stream-JSON output
clawdius chat "Hello" --output-format stream-json

# List sessions in JSON
clawdius sessions --output-format json

# Short flag
clawdius chat "Hello" -f json
```

## Success Criteria Met

✓ `--output-format json` flag works
✓ Chat responses output as valid JSON
✓ Errors output as structured JSON
✓ Stream-json works for streaming responses
✓ Backward compatible (text is default)
✓ Tests added for new functionality

## Architecture

The implementation leverages existing infrastructure:
- Uses existing `OutputFormat` enum from `output/format.rs`
- Uses existing `JsonOutput` struct for JSON serialization
- Uses existing `StreamEvent` and `StreamWriter` for streaming
- Adds high-level `OutputFormatter` to provide a simple API for CLI commands

## Notes

- The `OutputFormat` enum implements clap's `ValueEnum` trait for CLI parsing
- All formatters accept a generic `Write` trait for flexibility
- Error handling preserves the original error while formatting appropriately
- The formatter respects the `quiet` flag for compact vs pretty JSON
- Metadata (provider, model, tokens) is only shown in text format when `include_metadata` is true
