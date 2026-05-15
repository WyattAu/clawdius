# Tools

Clawdius provides a set of built-in tools that the LLM can invoke during a session.

## Overview

Tools extend the LLM's capabilities beyond text generation. When the LLM determines it needs to execute a command, read a file, or perform an action, it invokes the appropriate tool. The tool executes (within the sandbox) and returns the result to the LLM.

## Built-in Tools

### Shell Tool

Execute shell commands with sandboxing:

```rust
pub struct ShellParams {
    pub command: String,
    pub timeout: u64,       // Default: 120000ms
    pub cwd: Option<String>,
}

pub struct ShellResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}
```

The shell tool enforces all sandbox rules: blocked commands, timeouts, output limits, and directory restrictions.

### File Tool

File system operations:

- **Read**: Read file contents
- **Write**: Write file contents
- **List**: List directory contents
- **Search**: Search files by pattern

### Git Tool

Git repository operations:

- **Status**: Get current repository status
- **Stage/Unstage**: Stage or unstage files
- **Commit**: Create commits
- **Diff**: View staged or working diffs
- **History**: Get file change history

### Browser Tool

Browser automation for testing and web interaction (requires `browser` feature):

- Navigate to URLs
- Interact with page elements
- Capture screenshots
- Extract page content

### Web Search Tool

Search the web for information to supplement the LLM's knowledge.

## Tool Definition Format

Each tool is defined with a name, description, and JSON Schema parameters:

```rust
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,  // JSON Schema
}

pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub metadata: Option<serde_json::Value>,
}
```

## Auto-Approve Mode

By default, tool executions require user confirmation. In autonomous mode, tools execute without prompting:

```bash
clawdius chat --auto-approve "refactor this module"
clawdius auto "fix failing tests" --run-tests
```

## Sandboxed Execution

All tool execution passes through the sandbox system. See [Sandboxing](./sandboxing.md) for details on how commands are restricted.

## Custom Tools via MCP

Clawdius supports the Model Context Protocol for extending available tools. See the `clawdius-mcp` crate for MCP server implementation.

## Tool Errors

When a tool fails, the error is returned to the LLM with context:

```rust
Error::ToolExecution {
    tool: "shell",
    reason: "blocked command pattern detected: rm -rf /",
}
```

The LLM can then adjust its approach and try a different action.
