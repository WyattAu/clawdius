# Clawdius Python Client

> Python bindings for the Clawdius agentic coding engine.
> Status: PLANNED for v1.1.0 | Last updated: 2026-05-31

## Installation

```bash
pip install clawdius-client
```

Requires Python 3.10 or later. The client uses `asyncio` exclusively; synchronous wrappers are not provided.

## Architecture

The Python client is a pure-Python HTTP/stdio client that communicates with a running `clawdius` daemon. It does not require PyO3 bindings or Rust compilation at install time.

Transport options:
- **REST API** (default): HTTP requests to `http://localhost:8080`
- **MCP stdio**: Subprocess communication with `clawdius mcp`
- **Unix socket**: IPC at `$XDG_RUNTIME_DIR/clawdius.sock`

The client re-exports all core types from `clawdius_core` as Python dataclasses.

## Client Class

### Initialization

```python
from clawdius import Client, LlmConfig, RetryConfig

client = Client(
    config=LlmConfig.from_env("anthropic"),
    base_url="http://localhost:8080",
    transport="rest",
    retry_config=RetryConfig(
        max_retries=3,
        initial_delay_ms=1000,
        max_delay_ms=30000,
        exponential_base=2.0,
        retry_on=[RetryCondition.RATE_LIMIT, RetryCondition.TIMEOUT],
    ),
)
```

### Constructor Reference

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `config` | `LlmConfig` | -- (required) | LLM provider configuration |
| `base_url` | `str` | `"http://localhost:8080"` | Daemon URL |
| `transport` | `Literal["rest", "mcp", "ipc"]` | `"rest"` | Transport protocol |
| `retry_config` | `RetryConfig \| None` | `None` | Retry behavior (uses defaults if omitted) |
| `timeout` | `float` | `120.0` | Request timeout in seconds |

### LLM Provider Configuration

`LlmConfig` maps to `clawdius-core::llm::LlmConfig`.

```python
from clawdius import LlmConfig

# From environment variable
config = LlmConfig.from_env("anthropic")

# Explicit construction
config = LlmConfig(
    provider="anthropic",
    model="claude-3-5-sonnet-20241022",
    api_key="sk-ant-...",
    max_tokens=4096,
)

# Ollama (local, no API key)
config = LlmConfig(
    provider="ollama",
    model="llama3.2",
    base_url="http://localhost:11434",
)
```

#### LlmConfig Fields

| Field | Type | Description |
|-------|------|-------------|
| `provider` | `str` | One of: `"anthropic"`, `"openai"`, `"openrouter"`, `"ollama"`, `"zai"` |
| `model` | `str` | Model identifier |
| `api_key` | `str \| None` | API key (or `None` for local providers) |
| `base_url` | `str \| None` | Override provider endpoint URL |
| `max_tokens` | `int` | Maximum tokens per response (default: 4096) |

#### RetryConfig Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_retries` | `int` | `3` | Maximum retry attempts |
| `initial_delay_ms` | `int` | `1000` | Initial backoff delay |
| `max_delay_ms` | `int` | `30000` | Maximum backoff delay |
| `exponential_base` | `float` | `2.0` | Backoff multiplier |
| `retry_on` | `list[RetryCondition]` | `[RATE_LIMIT, TIMEOUT, SERVER_ERROR, NETWORK_ERROR]` | Conditions that trigger retry |

## Chat Completion

```python
import asyncio
from clawdius import Client, LlmConfig, ChatMessage, ChatRole

async def main():
    client = Client(config=LlmConfig.from_env("anthropic"))

    response = await client.chat(
        prompt="Explain Rust ownership rules",
        system_prompt="You are a systems programming expert.",
        temperature=0.7,
        max_tokens=1024,
    )
    print(response.text)
    print(f"Tokens: {response.usage.input} in / {response.usage.output} out")

asyncio.run(main())
```

### ChatOptions Reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `prompt` | `str` | -- (required) | User message text |
| `system_prompt` | `str \| None` | `None` | System prompt override |
| `files` | `list[str] \| None` | `None` | File paths to include in context |
| `tools` | `list[str] \| None` | `None` | Tool names to enable |
| `temperature` | `float \| None` | `None` | Sampling temperature |
| `max_tokens` | `int \| None` | `None` | Override per-request token limit |
| `session_id` | `str \| None` | `None` | Bind to existing session |

### Response Type

```python
@dataclasses.dataclass
class ChatResponse:
    text: str
    usage: TokenUsage
    tool_calls: list[ToolCall]
    session_id: str | None

@dataclasses.dataclass
class TokenUsage:
    input: int
    output: int
    cached: int

@dataclasses.dataclass
class ToolCall:
    id: str
    name: str
    arguments: dict[str, Any]
```

## Code Generation

```python
import asyncio
from clawdius import Client, LlmConfig

async def generate():
    client = Client(config=LlmConfig.from_env("anthropic"))

    response = await client.codegen(
        prompt="Implement a binary search tree in Python",
        language="python",
        files=["src/tree.rs"],
        tools=["file", "shell"],
    )
    print(response.text)
    print(f"Files modified: {response.files_modified}")

asyncio.run(generate())
```

## Streaming

```python
import asyncio
from clawdius import Client, LlmConfig

async def stream_chat():
    client = Client(config=LlmConfig.from_env("anthropic"))

    async for chunk in client.stream(
        prompt="Write a Kafka consumer in Go",
        tools=["file"],
    ):
        if chunk.is_text:
            print(chunk.delta, end="", flush=True)
        elif chunk.is_tool_call:
            print(f"\n[Tool call: {chunk.tool_name}]")

asyncio.run(stream_chat())
```

### StreamChunk Type

```python
@dataclasses.dataclass
class StreamChunk:
    delta: str
    is_text: bool
    is_tool_call: bool
    tool_name: str | None = None
    tool_id: str | None = None
    usage: TokenUsage | None = None
```

## Session Management

Sessions map to `clawdius-core::session::SessionStore` and `clawdius-core::session::SessionManager`.

```python
import asyncio
from clawdius import Client, LlmConfig

async def session_demo():
    client = Client(config=LlmConfig.from_env("anthropic"))

    # Create a session
    session = await client.sessions.create(title="Refactoring project")

    # Chat within the session
    response = await client.chat(
        prompt="List all functions in main.rs",
        session_id=session.id,
    )

    # Retrieve message history
    messages = await client.sessions.messages(session.id)
    for msg in messages:
        print(f"[{msg.role}] {msg.content[:80]}...")

    # List all sessions
    all_sessions = await client.sessions.list(limit=20)
    for s in all_sessions:
        print(f"{s.id}: {s.title} ({s.created_at})")

    # Export session
    data = await client.sessions.export(session.id, format="json")
    with open("session_export.json", "w") as f:
        f.write(data)

    # Delete a session
    await client.sessions.delete(session.id)

asyncio.run(session_demo())
```

### Session API Reference

| Method | Signature | Returns | Description |
|--------|-----------|---------|-------------|
| `sessions.create` | `(title: str \| None = None) -> Session` | `Session` | Create a new session |
| `sessions.list` | `(limit: int = 50, offset: int = 0) -> list[SessionMeta]` | `list[SessionMeta]` | List sessions |
| `sessions.get` | `(session_id: str) -> Session` | `Session` | Load a session by ID |
| `sessions.messages` | `(session_id: str, limit: int = 100) -> list[Message]` | `list[Message]` | Get message history |
| `sessions.export` | `(session_id: str, format: str = "json") -> str` | `str` | Export session data |
| `sessions.delete` | `(session_id: str) -> None` | -- | Delete a session |

### Session and Message Types

```python
@dataclasses.dataclass
class Session:
    id: str
    title: str | None
    created_at: datetime
    updated_at: datetime

@dataclasses.dataclass
class SessionMeta:
    id: str
    title: str | None
    created_at: datetime
    message_count: int

@dataclasses.dataclass
class Message:
    role: str  # "user", "assistant", "system", "tool"
    content: str
    created_at: datetime
    tool_calls: list[ToolCall] | None = None
```

## Tool Registration

Tools map to `clawdius-core::tools::Tool` with JSON Schema parameter definitions.

```python
from clawdius import Client, LlmConfig, ToolDefinition
import json

client = Client(config=LlmConfig.from_env("anthropic"))

# Register a custom tool
tool = ToolDefinition(
    name="query_database",
    description="Execute a read-only SQL query against the project database",
    parameters=json.loads(json.dumps({
        "type": "object",
        "properties": {
            "sql": {
                "type": "string",
                "description": "SQL SELECT statement to execute",
            }
        },
        "required": ["sql"],
    })),
    handler=None,  # Server-side tool; client just declares availability
)

await client.tools.register(tool)

# List available tools
available = await client.tools.list()
for t in available:
    print(f"{t.name}: {t.description}")

# Execute a tool
result = await client.tools.execute(
    name="read_file",
    arguments={"path": "src/main.rs"},
    sandbox_tier="untrusted",
)
print(result.output)
print(f"Success: {result.success}")
```

### ToolResult Type

```python
@dataclasses.dataclass
class ToolResult:
    success: bool
    output: str
    metadata: dict[str, Any] | None = None
    exit_code: int | None = None
```

### Sandbox Tier for Tool Execution

```python
from clawdius import SandboxTier

SandboxTier.TRUSTED_AUDITED  # Tier 1: no isolation
SandboxTier.TRUSTED           # Tier 2: blocklist only
SandboxTier.UNTRUSTED         # Tier 3: OS-level sandbox
SandboxTier.HARDENED          # Tier 4: VM, no network
```

## MCP Server Client

Communicate with MCP servers directly via the built-in MCP client.

```python
import asyncio
from clawdius.mcp import McpClient

async def mcp_demo():
    client = McpClient(command=["clawdius", "mcp"])

    # Initialize the MCP connection
    await client.initialize()

    # List available tools from the MCP server
    tools = await client.list_tools()
    for tool in tools:
        print(f"MCP tool: {tool.name}")

    # Call an MCP tool
    result = await client.call_tool("read_file", {"path": "README.md"})
    print(result)

    # Disconnect
    await client.close()

asyncio.run(mcp_demo())
```

### McpClient Reference

| Method | Signature | Description |
|--------|-----------|-------------|
| `initialize` | `() -> None` | Send MCP `initialize` request |
| `list_tools` | `() -> list[McpTool]` | List server-provided tools |
| `call_tool` | `(name: str, arguments: dict) -> Any` | Invoke an MCP tool |
| `list_resources` | `() -> list[McpResource]` | List server-provided resources |
| `read_resource` | `(uri: str) -> str` | Read a resource by URI |
| `close` | `() -> None` | Close the MCP connection |

## Error Handling

All errors inherit from `ClawdiusError` and mirror `clawdius-core::Error` variants.

```python
from clawdius.errors import (
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
)
```

### Exception Hierarchy

```
ClawdiusError
 +-- ConfigError              # Missing/invalid configuration
 +-- LlmError                 # General LLM failure
 +-- LlmProviderError         # Provider-specific failure (.provider, .message)
 +-- RateLimitedError         # HTTP 429 (.retry_after_ms)
 +-- ContextLimitError        # Token budget exceeded (.current, .limit)
 +-- ToolExecutionError       # Tool failed (.tool, .reason)
 +-- SessionNotFoundError     # Unknown session ID (.session_id)
 +-- AuthError                # Invalid/expired credentials
 +-- TimeoutError             # Request exceeded deadline
 +-- RetryExhaustedError      # All retry attempts failed (.attempts)
 +-- McpError                 # MCP protocol error (.code)
 +-- NetworkError             # Connection/transport failure
```

### Usage Pattern

```python
import asyncio
from clawdius import Client, LlmConfig
from clawdius.errors import RateLimitedError, AuthError, ClawdiusError

async def resilient_chat():
    client = Client(config=LlmConfig.from_env("anthropic"))

    try:
        response = await client.chat("Hello")
    except AuthError:
        print("API key is invalid or expired")
    except RateLimitedError as e:
        print(f"Rate limited; retry after {e.retry_after_ms}ms")
        await asyncio.sleep(e.retry_after_ms / 1000)
        response = await client.chat("Hello")
    except ClawdiusError as e:
        print(f"Unexpected error: {e}")

asyncio.run(resilient_chat())
```

## Type Hints

The client is fully typed. All public APIs use Python type hints. Runtime type checking is available via `typing.TYPE_CHECKING`.

```python
from clawdius import Client, LlmConfig, ChatResponse, Session, Message, TokenUsage
from clawdius.errors import ClawdiusError

# All types are available for static analysis
def process(response: ChatResponse) -> tuple[str, int]:
    return (response.text, response.usage.total())
```

## Configuration Reference

Configuration file (`clawdius.toml` or `.clawdius/config.toml`) is parsed by `clawdius-core::Config`. The Python client can load it directly:

```python
from clawdius import Client, load_config

config = load_config("clawdius.toml")
client = Client.from_config(config)
```

### Full Configuration Table

| Section | Key | Type | Default | Description |
|---------|-----|------|---------|-------------|
| `project` | `name` | `str` | -- | Project name |
| `project` | `rigor_level` | `str` | `"standard"` | One of: `"low"`, `"standard"`, `"high"` |
| `workspace` | `storage` | `str` | `"sqlite"` | Storage backend |
| `storage` | `database_path` | `str` | `".clawdius/graph/index.db"` | SQLite path |
| `storage` | `vector_path` | `str` | `".clawdius/graph/vectors.lance"` | LanceDB vector store |
| `storage` | `sessions_path` | `str` | `".clawdius/sessions.db"` | Session database |
| `llm` | `default_provider` | `str` | `"anthropic"` | Default LLM provider |
| `llm` | `max_tokens` | `int` | `4096` | Default max tokens |
| `llm.<provider>` | `model` | `str` | -- | Model identifier |
| `llm.<provider>` | `api_key_env` | `str` | -- | Env var for API key |
| `llm.<provider>` | `base_url` | `str` | -- | Override endpoint URL |
| `llm.retry` | `max_retries` | `int` | `3` | Retry attempts |
| `llm.retry` | `initial_delay_ms` | `int` | `1000` | Initial backoff |
| `llm.retry` | `max_delay_ms` | `int` | `30000` | Max backoff |
| `llm.retry` | `exponential_base` | `float` | `2.0` | Backoff multiplier |
| `session` | `compact_threshold` | `float` | `0.85` | Fraction of limit to trigger compaction |
| `session` | `keep_recent` | `int` | `4` | Messages preserved during compaction |
| `session` | `min_messages` | `int` | `10` | Minimum messages before compaction |
| `session` | `auto_save` | `bool` | `true` | Persist after each message |
| `shell_sandbox` | `timeout_secs` | `int` | `120` | Shell command timeout |
| `shell_sandbox` | `max_output_bytes` | `int` | `1048576` | Max stdout/stderr bytes |
| `shell_sandbox` | `restrict_to_cwd` | `bool` | `true` | Restrict file access to working directory |
| `shell_sandbox` | `blocked_commands` | `list[str]` | `[]` | Command blocklist |
