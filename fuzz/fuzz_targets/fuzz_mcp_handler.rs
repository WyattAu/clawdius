//! Fuzz target for MCP request handler dispatch.
//!
//! Feeds arbitrary valid MCP requests through the handler to verify
//! no panics on unexpected method names or malformed params.

#![no_main]

use clawdius_core::mcp::{handle_mcp_request, protocol::McpRequest};
use libfuzzer_sys::fuzz_target;

fn make_request(method: &str, id: u64, params: serde_json::Value) -> McpRequest {
    McpRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: method.to_string(),
        params: if params.is_null() { None } else { Some(params) },
    }
}

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Try to parse as JSON-RPC request
        if let Ok(req) = serde_json::from_str::<McpRequest>(s) {
            let _ = handle_mcp_request(&req);
            return;
        }

        // Otherwise, construct requests with fuzz data as method name
        let method = if s.len() > 100 { &s[..100] } else { s };
        let methods = [
            method,
            "initialize",
            "ping",
            "tools/list",
            "tools/call",
            "resources/list",
            "prompts/list",
        ];

        for m in &methods {
            let req = make_request(m, 1, serde_json::json!({"data": s}));
            let _ = handle_mcp_request(&req);
        }
    }
});
