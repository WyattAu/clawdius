//! Fuzz target for MCP protocol parsing and response formatting.
//!
//! Feeds arbitrary bytes through `parse_request` and `format_response`
//! to verify no panics on malformed input.

#![no_main]

use clawdius_core::mcp::protocol::{McpError, McpResponse};
use clawdius_mcp::{format_response, parse_request};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Parse request
        if let Ok(req) = parse_request(s) {
            // Verify it roundtrips through JSON serialization
            let json = serde_json::to_string(&req).unwrap_or_default();
            let _reparsed: Result<clawdius_core::mcp::protocol::McpRequest, _> =
                serde_json::from_str(&json);

            // Format success response to ensure no panic
            let resp = McpResponse::success(req.id, serde_json::json!({"ok": true}));
            let _ = format_response(&resp);

            // Format error response to ensure no panic
            let err = McpError::method_not_found("fuzz");
            let err_resp = McpResponse::error(req.id, err);
            let _ = format_response(&err_resp);

            // Format parse error response
            let parse_err = McpError::parse_error("bad input");
            let parse_resp = McpResponse::error(req.id, parse_err);
            let _ = format_response(&parse_resp);
        }
    }
});
