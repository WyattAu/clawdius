# JSON-RPC Protocol

Clawdius uses JSON-RPC for communication between editor extensions and the core engine.

## Overview

The `clawdius-code` binary acts as a JSON-RPC server, communicating with editor extensions over stdio. The protocol follows the JSON-RPC 2.0 specification.

## Transport

JSON-RPC messages are exchanged over standard input/output:

```
Editor Extension (TypeScript)
    │ stdin/stdout
    ▼
clawdius-code Binary
    │ Rust API calls
    ▼
clawdius-core Library
```

## Request Format

```json
{
  "jsonrpc": "2.0",
  "method": "semantic_search",
  "params": {
    "query": "error handling",
    "limit": 10
  },
  "id": 1
}
```

## Response Format

### Success

```json
{
  "jsonrpc": "2.0",
  "result": {
    "matches": [...]
  },
  "id": 1
}
```

### Error

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32600,
    "message": "Invalid params"
  },
  "id": 1
}
```

## Standard Methods

| Method | Description | Params |
|--------|-------------|--------|
| `semantic_search` | Search codebase by query | `query`, `limit` |
| `get_context` | Get context for a file | `path`, `line`, `column` |
| `chat` | Send a chat message | `message`, `session_id`, `mode` |
| `get_session` | Get session details | `session_id` |
| `list_sessions` | List all sessions | - |
| `create_checkpoint` | Create a checkpoint | `description` |
| `get_completion` | Get code completion | `file`, `line`, `column`, `language` |
| `git_status` | Get git status | - |
| `git_diff` | Get git diff | `staged` |

## JSON-RPC Server (Gateway)

The `clawdius-gateway` crate provides an HTTP-based JSON-RPC server using `jsonrpsee`:

```bash
clawdius server --host 0.0.0.0 --port 8080
```

This exposes the same methods over HTTP/WebSocket for remote access.

## Library

The `jsonrpsee` crate provides both server and client implementations:

```toml
[workspace.dependencies]
jsonrpsee = { version = "0.24", features = ["server", "client", "ws-client"] }
```

## Error Codes

| Code | Meaning |
|------|---------|
| -32700 | Parse error |
| -32600 | Invalid request |
| -32601 | Method not found |
| -32602 | Invalid params |
| -32603 | Internal error |

## VSCode Extension Commands

The VSCode extension communicates via JSON-RPC:

| Command | Description |
|---------|-------------|
| `Clawdius: Ask a question` | Open chat input |
| `Clawdius: Chat with selection` | Chat about selected code |
| `Clawdius: Add file to context` | Add file to conversation |
| `Clawdius: Create checkpoint` | Save current state |
| `Clawdius: Open chat view` | Open sidebar chat |
