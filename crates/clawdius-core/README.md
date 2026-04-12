# Clawdius Core

Core library for Clawdius - LLM integration, session management, tools, and sandboxing.

---

## Overview

`clawdius-core` is the heart of the Clawdius ecosystem, providing:

- **LLM Integration:** Multi-provider support (`OpenAI`, Anthropic, `DeepSeek`, Ollama)
- **Session Management:** Conversation state and history
- **Tool System:** Extensible tool framework (bash, file operations, web search, etc.)
- **Graph-RAG:** Code understanding via AST and vector indexing
- **Sandboxing:** Multi-tier execution isolation
- **MCP Protocol:** Model Context Protocol implementation

---

## Features

| Feature | Description | Dependencies Added |
|---------|-------------|-------------------|
| `default` | Standard features (none) | - |
| `vector-db` | `LanceDB` vector storage for semantic search | lancedb, arrow |
| `embeddings` | Real sentence transformer embeddings (BERT) | candle-core, candle-nn, candle-transformers, tokenizers, hf-hub |
| `hft-mode` | High-frequency trading mode (arena allocators, ring buffers) | - |
| `broker-mode` | Financial broker integration | - |
| `keyring` | OS keyring for secure credential storage | keyring |
| `crash-reporting` | Sentry crash reporting | sentry |

### Building with Features

```bash
# Minimal build (AST-only Graph-RAG, no vector DB)
cargo build -p clawdius-core

# With vector database for semantic search
cargo build -p clawdius-core --features vector-db

# With real embeddings (BERT-based)
cargo build -p clawdius-core --features embeddings

# Full Graph-RAG with both
cargo build -p clawdius-core --features vector-db,embeddings
```

---

## API Overview

### LLM Integration

```rust,no_run
use clawdius_core::llm::{LlmConfig, create_provider, ChatMessage, ChatRole};

# #[tokio::main]
# async fn main() -> clawdius_core::Result<()> {
// Create provider from environment
let config = LlmConfig::from_env("anthropic")?;
let provider = create_provider(&config)?;

// Send message
let response = provider.chat(vec![ChatMessage {
    role: ChatRole::User,
    content: "Explain Rust ownership.".to_string(),
}]).await?;

println!("{}", response);
# Ok(())
# }
```

### Session Management

```rust,no_run
use clawdius_core::session::{SessionStore, Session, Message};
use std::path::Path;

# fn main() -> clawdius_core::Result<()> {
// Create session store
let store = SessionStore::open(Path::new(".clawdius/sessions.db"))?;

// Create session
let mut session = Session::new();
session.add_message(Message::user("Hello!"));

// Save and retrieve
store.create_session(&session)?;
let loaded = store.load_session(&session.id)?;

// List sessions
let sessions = store.list_sessions()?;
for s in sessions {
    println!("{:?} - {:?}", s.id, s.title);
}
# Ok(())
# }
```

### Tool System

```rust
use clawdius_core::tools::{Tool, ToolResult};
use serde_json::json;

// Define a tool with JSON Schema parameters
let tool = Tool {
    name: "bash".to_string(),
    description: "Execute shell commands".to_string(),
    parameters: json!({
        "type": "object",
        "properties": {
            "command": { "type": "string", "description": "The command to run" }
        },
        "required": ["command"]
    }),
};

// Tool results are structured
let result = ToolResult {
    success: true,
    output: "Hello, World!".to_string(),
    metadata: Some(json!({ "exit_code": 0 })),
};
```

### Graph-RAG

```rust,no_run
use clawdius_core::graph_rag::{GraphStore, GraphRagConfig};
use std::path::Path;

# fn main() -> clawdius_core::Result<()> {
// Configure Graph-RAG
let config = GraphRagConfig {
    database_path: ".clawdius/graph/index.db".to_string(),
    vector_path: ".clawdius/graph/vectors.lance".to_string(),
};

// Open graph store and search symbols
let graph_store = GraphStore::open(Path::new(&config.database_path))?;
let symbols = graph_store.search_symbols("handle_request")?;
for symbol in symbols {
    println!("Found: {} ({:?})", symbol.name, symbol.kind);
}
# Ok(())
# }
```

### Sandboxing

```rust,no_run
use clawdius_core::sandbox::{SandboxTier, executor::SandboxExecutor, tiers::SandboxConfig};
use std::path::Path;

# fn main() -> clawdius_core::Result<()> {
// Choose appropriate tier based on code trust level
let tier = SandboxTier::Untrusted;
let config = SandboxConfig {
    tier,
    network: false,
    mounts: vec![],
};

// Execute command in sandbox
let executor = SandboxExecutor::new(tier, config)?;
let result = executor.execute("ls", &["-la"], Path::new("."))?;

println!("Status: {:?}", result.status);
println!("Output: {}", String::from_utf8_lossy(&result.stdout));
# Ok(())
# }
```

---

## Module Structure

````text
clawdius-core/
├── src/
│   ├── lib.rs              # Library entry point
│   ├── llm/                # LLM integration
│   ├── session/            # Session management
│   ├── tools/              # Tool system
│   ├── graph_rag/          # Graph-RAG system
│   ├── sandbox/            # Sandboxing
│   ├── mcp/                # Model Context Protocol
│   ├── agentic/            # Agentic system
│   ├── agents/             # Agent teams
│   ├── config.rs           # Configuration
│   ├── error/              # Error types
│   └── ...
└── Cargo.toml
````

---

## Usage Examples

### Custom Tool Implementation

```rust
use clawdius_core::tools::{Tool, ToolResult};
use serde_json::json;

// Define a custom tool with JSON Schema parameters
let tool = Tool {
    name: "my_tool".to_string(),
    description: "A custom tool that processes queries".to_string(),
    parameters: json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "The query to process"
            }
        },
        "required": ["query"]
    }),
};

// Return structured results
let result = ToolResult {
    success: true,
    output: json!({ "result": "Processed successfully" }).to_string(),
    metadata: Some(json!({ "tokens_used": 42 })),
};
```

### MCP Protocol

```rust
use clawdius_core::mcp::{McpRequest, McpResponse, McpTool, McpToolResult};
use serde_json::json;

// Create an MCP request
let request = McpRequest::new(1, "tools/list");

// Define an MCP tool
let tool = McpTool {
    name: "read_file".to_string(),
    description: "Read contents of a file".to_string(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" }
        },
        "required": ["path"]
    }),
};

// Create a tool result
let result = McpToolResult {
    content: vec![],
    is_error: false,
};
```

### Graph-RAG with Custom Indexing

```rust,no_run
use clawdius_core::graph_rag::{GraphStore, GraphRagConfig};
use std::path::Path;

# fn main() -> clawdius_core::Result<()> {
let config = GraphRagConfig {
    database_path: ".clawdius/graph/index.db".to_string(),
    vector_path: ".clawdius/graph/vectors.lance".to_string(),
};

let graph_store = GraphStore::open(Path::new(&config.database_path))?;

// Query symbol relationships
let symbols = graph_store.search_symbols("main")?;
for symbol in symbols {
    if let Some(id) = symbol.id {
        let refs = graph_store.find_symbol_refs(id)?;
        println!("{} referenced {} times", symbol.name, refs.len());
    }
}
# Ok(())
# }
```

---

## Configuration

### Environment Variables

```bash
# LLM API Keys
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
export DEEPSEEK_API_KEY="..."

# Optional: Custom endpoints
export OPENAI_API_BASE="https://api.openai.com/v1"
export OLLAMA_API_BASE="http://localhost:11434"
```

### Programmatic Configuration

```rust,no_run
use clawdius_core::llm::{LlmConfig, create_provider};

# fn main() -> clawdius_core::Result<()> {
let config = LlmConfig {
    provider: "openai".to_string(),
    model: "gpt-4".to_string(),
    api_key: std::env::var("OPENAI_API_KEY").ok(),
    base_url: Some("https://api.openai.com/v1".to_string()),
    max_tokens: 4096,
};

let provider = create_provider(&config)?;
# Ok(())
# }
```

---

## Performance

### Benchmarks

Run benchmarks with:

```bash
cargo bench -p clawdius-core
```

**Key Metrics:**

| Operation | Latency | Throughput |
|-----------|---------|------------|
| LLM request (streaming) | ~100ms first token | 50 tokens/sec |
| Graph-RAG query | 28ms | 35 queries/sec |
| AST indexing | 5ms per file | 200 files/sec |
| Vector search | 15ms | 66 searches/sec |
| Sandbox creation (Tier 2) | 150ms | - |

### Memory Usage

| Mode | Memory | Description |
|------|--------|-------------|
| Standard | ~50MB | Normal operation |
| HFT Mode | ~800MB | With arena + ring buffer |

---

## Testing

### Unit Tests

```bash
# Run all tests
cargo test -p clawdius-core

# Run specific test
cargo test -p clawdius-core test_semantic_search

# Run with verbose output
cargo test -p clawdius-core -- --nocapture
```

### Integration Tests

```bash
# Run integration tests
cargo test -p clawdius-core --test '*'

# Run with specific feature
cargo test -p clawdius-core --features hft-mode
```

### Error-Based Testing

```rust
use clawdius_core::{Error, Result};

fn test_provider_error() -> Result<()> {
    let config = clawdius_core::llm::LlmConfig::from_env("anthropic")?;
    let _provider = clawdius_core::llm::create_provider(&config)?;
    Ok(())
}

// Test error classification
let error = Error::RateLimited { retry_after_ms: 5000 };
assert!(error.is_retryable());
assert_eq!(error.retry_after_ms(), Some(5000));
```

---

## Error Handling

### Error Types

```rust
use clawdius_core::{Error, Result};

// Library uses thiserror
let error = Error::Config("missing API key".to_string());
assert!(!error.is_retryable());

let rate_error = Error::RateLimited { retry_after_ms: 5000 };
assert!(rate_error.is_retryable());
assert_eq!(rate_error.retry_after_ms(), Some(5000));

let timeout_error = Error::Timeout(std::time::Duration::from_secs(30));
assert!(timeout_error.is_retryable());
```

### Application Error Handling

```rust,no_run
use clawdius_core::{Error, Result};
use std::path::Path;

fn search_code() -> Result<()> {
    let graph_store = clawdius_core::graph_rag::GraphStore::open(
        Path::new(".clawdius/graph/index.db")
    )?;

    let symbols = graph_store.search_symbols("handle_request")?;
    println!("Found {} symbols", symbols.len());

    Ok(())
}
```

---

## Dependencies

### Key Dependencies

| Dependency | Purpose |
|------------|---------|
| `tokio` | Async runtime |
| `reqwest` | HTTP client for LLM APIs |
| `genai` | LLM abstraction layer |
| `rusqlite` | AST index storage |
| `tree-sitter` | Code parsing |
| `wasmtime` | WASM sandboxing |
| `jsonrpsee` | JSON-RPC server |

### Optional Dependencies

| Dependency | Feature | Purpose |
|------------|---------|---------|
| `lancedb` | `vector-db` | Vector database for semantic search |
| `arrow` | `vector-db` | Arrow data format for `LanceDB` |
| `candle-*` | `embeddings` | ML framework for embeddings |
| `tokenizers` | `embeddings` | `HuggingFace` tokenizers |
| `hf-hub` | `embeddings` | `HuggingFace` model hub |
| `keyring` | `keyring` | OS keyring integration |
| `sentry` | `crash-reporting` | Crash reporting |

---

## Platform Support

| Platform | Runtime | Sandbox | Notes |
|----------|---------|---------|-------|
| Linux | monoio | bubblewrap | Full support |
| macOS | tokio | sandbox-exec | Full support |
| WSL2 | tokio | bubblewrap | Full support |
| Windows | tokio | Limited | Experimental |

---

## Security

### Trust Boundaries

The library enforces strict trust boundaries:

1. **Host (Trusted):** LLM clients, session management, configuration
2. **Sandbox (Untrusted):** Tool execution, code analysis
3. **WASM (Hardened):** LLM reasoning modules

### Secret Management

- API keys are never logged
- Secrets stay in host memory
- Sandboxed code cannot access environment variables

---

## Contributing

See [CONTRIBUTING.md](../../CONTRIBUTING.md) for guidelines.

---

## License

Apache 2.0 - See [LICENSE](../../LICENSE) for details.
