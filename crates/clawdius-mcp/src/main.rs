//! MCP stdio server binary for Claude Desktop interop.

use clawdius_core::mcp::{handle_mcp_request, protocol::McpRequest};
use std::io::{BufRead, BufReader, Write};

fn main() {
    let mut stdin = BufReader::new(std::io::stdin());
    let mut stdout = std::io::stdout().lock();

    loop {
        let mut line = String::new();
        match stdin.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {},
            Err(e) => {
                eprintln!("Error reading stdin: {e}");
                break;
            },
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let request: McpRequest = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to parse request: {e}");
                continue;
            },
        };

        let response = handle_mcp_request(&request);

        if !response.is_notification() {
            let json = match serde_json::to_string(&response) {
                Ok(j) => j,
                Err(e) => {
                    eprintln!("Failed to serialize response: {e}");
                    continue;
                },
            };
            if writeln!(stdout, "{json}").is_err() {
                break;
            }
        }
    }
}
