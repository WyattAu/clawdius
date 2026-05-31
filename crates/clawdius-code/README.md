# clawdius-code

JSON-RPC 2.0 server binary that acts as the bridge between the Clawdius VSCode extension and the LLM backend. It communicates with the extension over stdio, handling session lifecycle, chat streaming, file I/O, context management, state snapshots, and inline code completion.

## Features

- **Session management** -- create, load, save, list, and delete named sessions
- **Chat with LLM streaming** -- send prompts to the configured LLM provider and receive streamed responses
- **File operations** -- read and write files on behalf of the extension
- **Context management** -- add, remove, list, and compact context entries (file snippets, symbols)
- **State and checkpoints** -- snapshot, restore, and list editor/workspace state
- **Inline code completion** -- provide single-line completions at the cursor position

## Supported JSON-RPC Methods

| Method | Handler | Description |
| --- | --- | --- |
| `session/create` | `SessionHandler` | Create a new named session |
| `session/load` | `SessionHandler` | Load an existing session by ID |
| `session/save` | `SessionHandler` | Persist the current session to disk |
| `session/list` | `SessionHandler` | List all sessions |
| `session/delete` | `SessionHandler` | Delete a session by ID |
| `chat/send` | `ChatHandler` | Send a single message and collect the full response |
| `chat/stream` | `ChatHandler` | Send a message and stream tokens back over stdio |
| `chat/cancel` | `ChatHandler` | Cancel an in-flight streaming request |
| `file/read` | `FileHandler` | Read a file from disk |
| `file/write` | `FileHandler` | Write content to a file on disk |
| `context/add` | `ContextHandler` | Add a context entry (file, symbol, snippet) |
| `context/remove` | `ContextHandler` | Remove a context entry by ID |
| `context/list` | `ContextHandler` | List all active context entries |
| `context/compact` | `ContextHandler` | Compact context to fit token budgets |
| `state/get` | `StateHandler` | Get current editor/workspace state |
| `state/checkpoint` | `StateHandler` | Create a named state checkpoint |
| `state/restore` | `StateHandler` | Restore state from a checkpoint |
| `state/list` | `StateHandler` | List available checkpoints |
| `completion/inline` | `CompletionHandler` | Get an inline code completion at a position |

All methods follow the JSON-RPC 2.0 specification. Requests must include `"jsonrpc": "2.0"`, `"id"`, and `"method"` fields. Errors use standard codes: `-32700` (parse error), `-32600` (invalid request), `-32601` (method not found), `-32602` (invalid params), `-32603` (internal error).

## Quick Start

Use `clawdius-code` as a library to parse and format JSON-RPC messages:

```rust
use clawdius_code::{parse_request, format_response};
use clawdius_core::rpc::types::Response;

fn main() {
    let raw = r#"{"jsonrpc":"2.0","id":1,"method":"session/list"}"#;

    let request = parse_request(raw).expect("valid JSON-RPC request");
    println!("method: {}", request.method);

    let response = Response::success(request.id, serde_json::json!({"sessions": []}));
    let json = format_response(&response);
    println!("{}", json);
}
```

Add to `Cargo.toml`:

```toml
[dependencies]
clawdius-code = { version = "1.0.0-rc.2", path = "crates/clawdius-code" }
clawdius-core = { version = "1.0.0-rc.2", path = "crates/clawdius-core", features = ["rpc"] }
```

## Binary Usage

The VSCode extension spawns the `clawdius-code` binary as a child process. The binary reads newline-delimited JSON-RPC requests from stdin and writes newline-delimited JSON-RPC responses to stdout. Diagnostic output goes to stderr.

```
vscode extension  -->  stdin  -->  clawdius-code  -->  stdout  -->  vscode extension
                                     |
                                  stderr (logs)
```

On startup the binary:

1. Loads configuration via `clawdius_core::Config::load_default()`
2. Initializes an LLM client from the config (gracefully degrades if no provider is set)
3. Registers all JSON-RPC handlers
4. Enters the stdio event loop via `RpcServer::run_stdio()`

The process exits cleanly on EOF (stdin close).

## Configuration

The binary reads configuration through `clawdius_core::Config::load_default()`, which looks for config files in standard locations (project root, XDG config directory, etc.). If loading fails, a default config is used and a warning is printed to stderr.

Key config sections relevant to `clawdius-code`:

- **LLM provider** -- `llm.default_provider`, model name, and API keys. Without a provider, chat and completion handlers return errors.
- **Session storage** -- directory and format for persisted sessions.
- **Context limits** -- token budget and compaction thresholds.

## Feature Flags

| Flag | Default | Description |
| --- | --- | --- |
| `mimalloc` | enabled | Use [mimalloc](https://github.com/microsoft/mimalloc) as the global allocator for reduced allocation overhead |

Disable mimalloc when profiling or using custom allocators:

```toml
[dependencies]
clawdius-code = { version = "1.0.0-rc.2", default-features = false }
```

## Testing

**Unit tests** (in `src/lib.rs`):

```bash
cargo test -p clawdius-code
```

Covers request parsing, response formatting, error codes, edge cases, round-trip serialization, and concurrent access.

**Integration tests** (in `tests/integration.rs`):

```bash
cargo test -p clawdius-code --test integration
```

Spawns the actual `clawdius-code` binary as a subprocess, sends JSON-RPC requests over stdin, and validates stdout responses. Tests session CRUD, file I/O, context management, state checkpoints, error handling for malformed input and unknown methods, empty-line tolerance, notification-style requests, and multi-request ordering.

## License

See the top-level [LICENSE](../../LICENSE) file.
