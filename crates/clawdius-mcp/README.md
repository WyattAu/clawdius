# clawdius-mcp

MCP (Model Context Protocol) server for Claude Desktop, Cursor, and other MCP-compatible AI tools.

## What It Does

Clawdius exposes 12 tools that give AI coding assistants real access to your codebase:

| Tool | Description |
|------|-------------|
| `read_file` | Read file contents |
| `list_directory` | List files and directories |
| `write_file` | Create or overwrite files (with path traversal protection) |
| `edit_file` | Search-and-replace within files |
| `codebase_search` | Search indexed symbols (names, signatures, doc comments) via tree-sitter |
| `git_status` | Show working tree status |
| `git_log` | Show recent commits |
| `git_diff` | Show unstaged changes |
| `check_build` | Run `cargo check` |
| `run_tests` | Run tests with 120s timeout |
| `web_search` | Search the web via DuckDuckGo (no API key needed) |
| `generate_code` | Generate or edit code using an LLM (requires API key) |

## Setup with Claude Desktop

1. **Build the binary:**

   ```bash
   cargo build --release -p clawdius-mcp
   ```

2. **Add to Claude Desktop config** (`claude_desktop_config.json`):

   ```json
   {
     "mcpServers": {
       "clawdius": {
         "command": "/path/to/clawdius-mcp",
         "args": []
       }
     }
   }
   ```

3. **Restart Claude Desktop.** The tools will appear in the Claude Desktop interface.

## Setup with Cursor

Add to `.cursor/mcp.json` in your project:

```json
{
  "mcpServers": {
    "clawdius": {
      "command": "/path/to/clawdius-mcp",
      "args": []
    }
  }
}
```

## Optional: LLM Code Generation

The `generate_code` tool requires an LLM API key. Set environment variables:

```bash
export CLAWDIUS_PROVIDER=anthropic  # or "openai"
export ANTHROPIC_API_KEY=sk-ant-...
# or
export OPENAI_API_KEY=sk-...
```

If no API key is set, `generate_code` will return an error explaining what to configure. All other 11 tools work without any API key.

## How It Works

The server reads newline-delimited JSON from stdin and writes JSON responses to stdout (MCP stdio transport). It is stateless — no config files, no database, no network connections (except for `web_search` and `generate_code`).

The `codebase_search` tool lazy-indexes the current directory on first call using tree-sitter. It parses `.rs`, `.py`, `.js`, `.ts`, `.tsx`, `.go`, `.java`, `.c`, `.cpp`, and `.swift` files, extracting function names, class names, type signatures, and doc comments into an in-memory SQLite store. Subsequent searches query this index.
