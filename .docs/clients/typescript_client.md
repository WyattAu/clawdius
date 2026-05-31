# Clawdius TypeScript SDK

> TypeScript/JavaScript bindings for the Clawdius agentic coding engine.
> Status: PLANNED for v1.1.0 | Last updated: 2026-05-31

## Installation

```bash
npm install @clawdius/sdk
```

Requires Node.js 18 or later. The SDK ships TypeScript declarations inline (no separate `@types` package needed).

## Module Support

The package provides dual ESM/CJS entry points:

```json
{
  "main": "./dist/index.cjs",
  "module": "./dist/index.js",
  "types": "./dist/index.d.ts",
  "exports": {
    ".": {
      "import": { "types": "./dist/index.d.ts", "default": "./dist/index.js" },
      "require": { "types": "./dist/index.d.cts", "default": "./dist/index.cjs" }
    }
  }
}
```

```typescript
// ESM (default)
import { Client } from "@clawdius/sdk";

// CJS
const { Client } = require("@clawdius/sdk");
```

## Architecture

The TypeScript SDK is a pure JavaScript HTTP client that communicates with a running `clawdius` daemon. No native compilation required.

Transport options:
- **REST API** (default): HTTP requests to `http://localhost:8080`
- **MCP stdio**: Subprocess communication with `clawdius mcp`
- **Unix socket**: IPC at `$XDG_RUNTIME_DIR/clawdius.sock`

## Client Class

### Initialization

```typescript
import { Client, LlmConfig, RetryConfig, RetryCondition } from "@clawdius/sdk";

const client = new Client({
  config: LlmConfig.fromEnv("anthropic"),
  baseUrl: "http://localhost:8080",
  transport: "rest",
  retryConfig: new RetryConfig({
    maxRetries: 3,
    initialDelayMs: 1000,
    maxDelayMs: 30000,
    exponentialBase: 2.0,
    retryOn: [RetryCondition.RateLimit, RetryCondition.Timeout],
  }),
  timeout: 120_000,
});
```

### ClientOptions Interface

```typescript
interface ClientOptions {
  config: LlmConfig;
  baseUrl?: string;
  transport?: "rest" | "mcp" | "ipc";
  retryConfig?: RetryConfig;
  timeout?: number;
}
```

### LLM Provider Types

`LlmConfig` maps to `clawdius-core::llm::LlmConfig`. The `Provider` type literal constrains valid values.

```typescript
import { LlmConfig, type Provider } from "@clawdius/sdk";

// From environment variable
const config = LlmConfig.fromEnv("anthropic");

// Explicit construction
const config = new LlmConfig({
  provider: "anthropic" satisfies Provider,
  model: "claude-3-5-sonnet-20241022",
  apiKey: "sk-ant-...",
  maxTokens: 4096,
});

// Ollama (local, no API key)
const config = new LlmConfig({
  provider: "ollama",
  model: "llama3.2",
  baseUrl: "http://localhost:11434",
});
```

#### Provider Type

```typescript
type Provider = "anthropic" | "openai" | "openrouter" | "ollama" | "zai";
```

#### LlmConfig Interface

```typescript
class LlmConfig {
  constructor(opts: {
    provider: Provider;
    model: string;
    apiKey?: string;
    baseUrl?: string;
    maxTokens?: number;
  });

  static fromEnv(provider: Provider): LlmConfig;

  readonly provider: Provider;
  readonly model: string;
  readonly apiKey: string | undefined;
  readonly baseUrl: string | undefined;
  readonly maxTokens: number;
}
```

#### RetryConfig Interface

```typescript
class RetryConfig {
  constructor(opts: {
    maxRetries?: number;
    initialDelayMs?: number;
    maxDelayMs?: number;
    exponentialBase?: number;
    retryOn?: RetryCondition[];
  });

  readonly maxRetries: number;       // default: 3
  readonly initialDelayMs: number;   // default: 1000
  readonly maxDelayMs: number;        // default: 30000
  readonly exponentialBase: number;  // default: 2.0
  readonly retryOn: RetryCondition[]; // default: all conditions
}

enum RetryCondition {
  RateLimit = "rate_limit",
  Timeout = "timeout",
  ServerError = "server_error",
  NetworkError = "network_error",
}
```

## Chat Completion

```typescript
import { Client, LlmConfig, type ChatResponse } from "@clawdius/sdk";

const client = new Client({ config: LlmConfig.fromEnv("anthropic") });

const response: ChatResponse = await client.chat({
  prompt: "Explain Rust ownership rules",
  systemPrompt: "You are a systems programming expert.",
  temperature: 0.7,
  maxTokens: 1024,
});

console.log(response.text);
console.log(`Tokens: ${response.usage.input} in / ${response.usage.output} out`);
```

### ChatOptions Interface

```typescript
interface ChatOptions {
  prompt: string;
  systemPrompt?: string;
  files?: string[];
  tools?: string[];
  temperature?: number;
  maxTokens?: number;
  sessionId?: string;
}
```

### Response Types

```typescript
interface ChatResponse {
  text: string;
  usage: TokenUsage;
  toolCalls: ToolCall[];
  sessionId: string | null;
}

interface TokenUsage {
  input: number;
  output: number;
  cached: number;
}

interface ToolCall {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
}
```

## Code Generation

```typescript
import { Client, LlmConfig } from "@clawdius/sdk";

const client = new Client({ config: LlmConfig.fromEnv("anthropic") });

const response = await client.codegen({
  prompt: "Implement a binary search tree",
  language: "typescript",
  files: ["src/tree.ts"],
  tools: ["file", "shell"],
});

console.log(response.text);
console.log(`Files modified: ${response.filesModified}`);
```

### CodeGenOptions Interface

```typescript
interface CodeGenOptions {
  prompt: string;
  language?: string;
  files?: string[];
  tools?: string[];
  sessionId?: string;
}
```

## Streaming

The SDK provides streaming via `AsyncIterator`, compatible with `for await...of` syntax.

```typescript
import { Client, LlmConfig, type StreamChunk } from "@clawdius/sdk";

const client = new Client({ config: LlmConfig.fromEnv("anthropic") });

for await (const chunk of client.stream({
  prompt: "Write a Kafka consumer in Go",
  tools: ["file"],
})) {
  if (chunk.type === "text") {
    process.stdout.write(chunk.delta);
  } else if (chunk.type === "tool_call") {
    console.log(`\n[Tool call: ${chunk.toolName}]`);
  }
}
```

### Stream API

```typescript
stream(options: ChatOptions): AsyncIterable<StreamChunk>;
```

### StreamChunk Discriminated Union

```typescript
type StreamChunk =
  | { type: "text"; delta: string }
  | { type: "tool_call"; toolId: string; toolName: string; delta: string }
  | { type: "usage"; usage: TokenUsage };
```

## Session Management

Sessions map to `clawdius-core::session::SessionStore` and `clawdius-core::session::SessionManager`.

```typescript
import { Client, LlmConfig } from "@clawdius/sdk";

const client = new Client({ config: LlmConfig.fromEnv("anthropic") });

// Create a session
const session = await client.sessions.create({ title: "Refactoring project" });

// Chat within the session
const response = await client.chat({
  prompt: "List all functions in main.rs",
  sessionId: session.id,
});

// Retrieve message history
const messages = await client.sessions.messages(session.id);
for (const msg of messages) {
  console.log(`[${msg.role}] ${msg.content.slice(0, 80)}...`);
}

// List all sessions
const allSessions = await client.sessions.list({ limit: 20 });
for (const s of allSessions) {
  console.log(`${s.id}: ${s.title} (${s.createdAt.toISOString()})`);
}

// Export session
const data = await client.sessions.export(session.id, "json");

// Delete a session
await client.sessions.delete(session.id);
```

### Session API Reference

| Method | Signature | Returns | Description |
|--------|-----------|---------|-------------|
| `sessions.create` | `(opts?: { title?: string }) => Promise<Session>` | `Session` | Create a new session |
| `sessions.list` | `(opts?: { limit?: number; offset?: number }) => Promise<SessionMeta[]>` | `SessionMeta[]` | List sessions |
| `sessions.get` | `(sessionId: string) => Promise<Session>` | `Session` | Load a session by ID |
| `sessions.messages` | `(sessionId: string, limit?: number) => Promise<Message[]>` | `Message[]` | Get message history |
| `sessions.export` | `(sessionId: string, format?: string) => Promise<string>` | `string` | Export session data |
| `sessions.delete` | `(sessionId: string) => Promise<void>` | -- | Delete a session |

### Session and Message Types

```typescript
interface Session {
  id: string;
  title: string | null;
  createdAt: Date;
  updatedAt: Date;
}

interface SessionMeta {
  id: string;
  title: string | null;
  createdAt: Date;
  messageCount: number;
}

interface Message {
  role: "user" | "assistant" | "system" | "tool";
  content: string;
  createdAt: Date;
  toolCalls?: ToolCall[];
}
```

## Tool Registration

Tools map to `clawdius-core::tools::Tool` with JSON Schema parameter definitions.

```typescript
import { Client, LlmConfig, ToolDefinition, SandboxTier } from "@clawdius/sdk";

const client = new Client({ config: LlmConfig.fromEnv("anthropic") });

const tool = new ToolDefinition({
  name: "query_database",
  description: "Execute a read-only SQL query against the project database",
  parameters: {
    type: "object",
    properties: {
      sql: {
        type: "string",
        description: "SQL SELECT statement to execute",
      },
    },
    required: ["sql"],
  },
});

await client.tools.register(tool);

// List available tools
const available = await client.tools.list();
for (const t of available) {
  console.log(`${t.name}: ${t.description}`);
}

// Execute a tool
const result = await client.tools.execute({
  name: "read_file",
  arguments: { path: "src/main.rs" },
  sandboxTier: "untrusted",
});
console.log(result.output);
console.log(`Success: ${result.success}`);
```

### ToolResult Type

```typescript
interface ToolResult {
  success: boolean;
  output: string;
  metadata?: Record<string, unknown>;
  exitCode?: number;
}
```

### SandboxTier Type

```typescript
type SandboxTier = "trusted_audited" | "trusted" | "untrusted" | "hardened";
```

## MCP Protocol Client

Communicate with MCP servers using the built-in MCP client, matching `clawdius-core::mcp::protocol` (JSON-RPC 2.0, protocol version `2025-03-26`).

```typescript
import { McpClient } from "@clawdius/sdk/mcp";

const client = new McpClient({ command: ["clawdius", "mcp"] });

// Initialize the MCP connection
await client.initialize();

// List available tools from the MCP server
const tools = await client.listTools();
for (const tool of tools) {
  console.log(`MCP tool: ${tool.name}`);
}

// Call an MCP tool
const result = await client.callTool("read_file", { path: "README.md" });
console.log(result);

// List resources
const resources = await client.listResources();

// Read a resource
const content = await client.readResource("file:///path/to/file");

// Disconnect
await client.close();
```

### McpClient Reference

| Method | Signature | Description |
|--------|-----------|-------------|
| `initialize` | `() => Promise<void>` | Send MCP `initialize` request |
| `listTools` | `() => Promise<McpTool[]>` | List server-provided tools |
| `callTool` | `(name: string, args: Record<string, unknown>) => Promise<unknown>` | Invoke an MCP tool |
| `listResources` | `() => Promise<McpResource[]>` | List server-provided resources |
| `readResource` | `(uri: string) => Promise<string>` | Read a resource by URI |
| `close` | `() => Promise<void>` | Close the MCP connection |

### McpTool and McpResource Types

```typescript
interface McpTool {
  name: string;
  description?: string;
  inputSchema: Record<string, unknown>;
}

interface McpResource {
  uri: string;
  name: string;
  description?: string;
  mimeType?: string;
}
```

## Error Handling

All errors extend the base `ClawdiusError` class and mirror `clawdius-core::Error` variants.

```typescript
import {
  ClawdiusError,
  ConfigError,
  LlmError,
  LlmProviderError,
  RateLimitedError,
  ContextLimitError,
  ToolExecutionError,
  SessionNotFoundError,
  AuthError,
  TimeoutError,
  RetryExhaustedError,
  McpError,
  NetworkError,
} from "@clawdius/sdk/errors";
```

### Error Class Hierarchy

```
ClawdiusError (abstract base)
 +-- ConfigError               // Missing/invalid configuration
 +-- LlmError                  // General LLM failure
 +-- LlmProviderError          // Provider-specific (.provider, .message)
 +-- RateLimitedError          // HTTP 429 (.retryAfterMs)
 +-- ContextLimitError         // Token budget exceeded (.current, .limit)
 +-- ToolExecutionError        // Tool failed (.tool, .reason)
 +-- SessionNotFoundError      // Unknown session ID (.sessionId)
 +-- AuthError                 // Invalid/expired credentials
 +-- TimeoutError              // Request exceeded deadline
 +-- RetryExhaustedError       // All retry attempts failed (.attempts)
 +-- McpError                  // MCP protocol error (.code)
 +-- NetworkError              // Connection/transport failure
```

### Usage Pattern

```typescript
import { Client, LlmConfig } from "@clawdius/sdk";
import { RateLimitedError, AuthError, ClawdiusError } from "@clawdius/sdk/errors";

async function resilientChat(): Promise<void> {
  const client = new Client({ config: LlmConfig.fromEnv("anthropic") });

  try {
    const response = await client.chat({ prompt: "Hello" });
    console.log(response.text);
  } catch (err) {
    if (err instanceof AuthError) {
      console.error("API key is invalid or expired");
    } else if (err instanceof RateLimitedError) {
      const ms = err.retryAfterMs;
      console.error(`Rate limited; retry after ${ms}ms`);
      await new Promise((r) => setTimeout(r, ms));
      const response = await client.chat({ prompt: "Hello" });
      console.log(response.text);
    } else if (err instanceof ClawdiusError) {
      console.error(`Unexpected error: ${err.message}`);
    }
  }
}
```

### TypeScript Generics

The client uses generics to allow typed tool results and custom response types:

```typescript
interface QueryResult {
  rows: Array<Record<string, unknown>>;
  rowCount: number;
}

const result = await client.tools.execute<QueryResult>({
  name: "query_database",
  arguments: { sql: "SELECT * FROM users" },
  sandboxTier: "untrusted",
});

// result.output is typed as QueryResult
console.log(`Found ${result.output.rowCount} rows`);
```

## Configuration Reference

Configuration file (`clawdius.toml`) is parsed by `clawdius-core::Config`. The TypeScript client can load it directly:

```typescript
import { Client, loadConfig } from "@clawdius/sdk";

const config = await loadConfig("clawdius.toml");
const client = Client.fromConfig(config);
```

### Full Configuration Table

| Section | Key | Type | Default | Description |
|---------|-----|------|---------|-------------|
| `project` | `name` | `string` | -- | Project name |
| `project` | `rigor_level` | `string` | `"standard"` | `"low"`, `"standard"`, or `"high"` |
| `workspace` | `storage` | `string` | `"sqlite"` | Storage backend |
| `storage` | `database_path` | `string` | `".clawdius/graph/index.db"` | SQLite path |
| `storage` | `vector_path` | `string` | `".clawdius/graph/vectors.lance"` | LanceDB vector store |
| `storage` | `sessions_path` | `string` | `".clawdius/sessions.db"` | Session database |
| `llm` | `default_provider` | `string` | `"anthropic"` | Default LLM provider |
| `llm` | `max_tokens` | `number` | `4096` | Default max tokens |
| `llm.<provider>` | `model` | `string` | -- | Model identifier |
| `llm.<provider>` | `api_key_env` | `string` | -- | Env var for API key |
| `llm.<provider>` | `base_url` | `string` | -- | Override endpoint URL |
| `llm.retry` | `max_retries` | `number` | `3` | Retry attempts |
| `llm.retry` | `initial_delay_ms` | `number` | `1000` | Initial backoff |
| `llm.retry` | `max_delay_ms` | `number` | `30000` | Max backoff |
| `llm.retry` | `exponential_base` | `number` | `2.0` | Backoff multiplier |
| `session` | `compact_threshold` | `number` | `0.85` | Fraction of limit to trigger compaction |
| `session` | `keep_recent` | `number` | `4` | Messages preserved during compaction |
| `session` | `min_messages` | `number` | `10` | Minimum messages before compaction |
| `session` | `auto_save` | `boolean` | `true` | Persist after each message |
| `shell_sandbox` | `timeout_secs` | `number` | `120` | Shell command timeout |
| `shell_sandbox` | `max_output_bytes` | `number` | `1048576` | Max stdout/stderr bytes |
| `shell_sandbox` | `restrict_to_cwd` | `boolean` | `true` | Restrict file access to working directory |
| `shell_sandbox` | `blocked_commands` | `string[]` | `[]` | Command blocklist |
