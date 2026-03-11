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

```rust
use clawdius_core::llm::{LlmClient, Provider, Message};

// Create client
let client = LlmClient::new(Provider::OpenAI)?;

// Send message
let response = client.complete(vec![
    Message::system("You are a helpful coding assistant."),
    Message::user("Explain Rust ownership."),
]).await?;

println!("{}", response.content);
```

### Session Management

```rust
use clawdius_core::session::{Session, SessionConfig};

// Create session
let config = SessionConfig {
    provider: Provider::Anthropic,
    model: "claude-3-opus".to_string(),
    max_tokens: 4096,
};

let mut session = Session::new(config)?;

// Add messages
session.add_message(Message::user("Hello!"));

// Get completion
let response = session.complete().await?;

// Access history
for msg in session.history() {
    println!("{}: {}", msg.role, msg.content);
}
```

### Tool System

```rust
use clawdius_core::tools::{ToolRegistry, BashTool, FileReadTool};

// Create registry
let mut registry = ToolRegistry::new();

// Register tools
registry.register(BashTool::new())?;
registry.register(FileReadTool::new())?;

// Execute tool
let result = registry.execute("bash", json!({
    "command": "echo 'Hello, World!'"
})).await?;

println!("{}", result.output);
```

### Graph-RAG

```rust
use clawdius_core::graph_rag::{GraphRag, SearchResult};

// Initialize
let graph_rag = GraphRag::new("./my-project")?;

// Index project
graph_rag.index_project("./src")?;

// Semantic search
let results: Vec<SearchResult> = graph_rag
    .semantic_search("error handling", 10)
    .await?;

for result in results {
    println!("{} (score: {:.2})", result.file_path, result.score);
}

// Structural query
let callers = graph_rag.find_callers("main")?;
for caller in callers {
    println!("Called by: {} at {:?}", caller.symbol, caller.location);
}
```

### Sandboxing

```rust
use clawdius_core::sandbox::{Sandbox, SandboxTier, SandboxPolicy};

// Create sandbox policy
let policy = SandboxPolicy {
    tier: SandboxTier::Container,
    allowed_commands: vec!["python3", "node"],
    network_access: false,
    read_paths: vec!["./src"],
    write_paths: vec!["./output"],
};

// Create sandbox
let sandbox = Sandbox::new(policy)?;

// Execute command
let result = sandbox.execute("python3 script.py").await?;

println!("Exit code: {}", result.exit_code);
println!("Output: {}", result.stdout);
```

---

## Module Structure

```
clawdius-core/
├── src/
│   ├── lib.rs              # Library entry point
│   ├── llm/                # LLM integration
│   │   ├── mod.rs
│   │   ├── client.rs       # LLM client trait
│   │   ├── openai.rs       # OpenAI implementation
│   │   ├── anthropic.rs    # Anthropic implementation
│   │   ├── deepseek.rs     # DeepSeek implementation
│   │   └── ollama.rs       # Ollama implementation
│   ├── session/            # Session management
│   │   ├── mod.rs
│   │   ├── session.rs      # Session struct
│   │   └── history.rs      # Conversation history
│   ├── tools/              # Tool system
│   │   ├── mod.rs
│   │   ├── registry.rs     # Tool registry
│   │   ├── bash.rs         # Bash execution
│   │   ├── file.rs         # File operations
│   │   ├── web_search.rs   # Web search
│   │   └── ...             # Other tools
│   ├── graph_rag/          # Graph-RAG system
│   │   ├── mod.rs
│   │   ├── ast_index.rs    # SQLite AST index
│   │   ├── vector_store.rs # LanceDB vectors
│   │   └── search.rs       # Search implementation
│   ├── sandbox/            # Sandboxing
│   │   ├── mod.rs
│   │   ├── policy.rs       # Sandbox policies
│   │   ├── bubblewrap.rs   # Linux sandbox
│   │   ├── sandbox_exec.rs # macOS sandbox
│   │   └── wasmtime.rs     # WASM sandbox
│   ├── mcp/                # Model Context Protocol
│   │   ├── mod.rs
│   │   ├── server.rs       # MCP server
│   │   └── protocol.rs     # Protocol types
│   └── utils/              # Utilities
│       ├── mod.rs
│       ├── token_counter.rs
│       └── diff.rs
└── Cargo.toml
```

---

## Usage Examples

### Custom Tool Implementation

```rust
use clawdius_core::tools::{Tool, ToolResult, ToolContext};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct MyToolInput {
    query: String,
}

#[derive(Serialize)]
struct MyToolOutput {
    result: String,
}

struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str {
        "my_tool"
    }
    
    fn description(&self) -> &str {
        "A custom tool that does something useful"
    }
    
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The query to process"
                }
            },
            "required": ["query"]
        })
    }
    
    async fn execute(
        &self,
        input: serde_json::Value,
        _context: ToolContext,
    ) -> Result<ToolResult, Box<dyn std::error::Error>> {
        let input: MyToolInput = serde_json::from_value(input)?;
        
        // Do something useful
        let result = format!("Processed: {}", input.query);
        
        Ok(ToolResult {
            output: serde_json::to_value(MyToolOutput { result })?,
            metadata: Default::default(),
        })
    }
}
```

### MCP Server

```rust
use clawdius_core::mcp::{McpServer, McpConfig};

#[tokio::main]
async fn main() -> Result<()> {
    let config = McpConfig {
        port: 3000,
        host: "127.0.0.1".to_string(),
    };
    
    let server = McpServer::new(config)?;
    
    // Register tools
    server.register_tool(BashTool::new())?;
    server.register_tool(FileReadTool::new())?;
    
    // Start server
    server.start().await?;
    
    Ok(())
}
```

### Graph-RAG with Custom Indexing

```rust
use clawdius_core::graph_rag::{GraphRag, IndexConfig};

let config = IndexConfig {
    include_patterns: vec!["**/*.rs".to_string()],
    exclude_patterns: vec!["**/target/**".to_string()],
    max_file_size: 1024 * 1024, // 1MB
    embedding_model: "text-embedding-ada-002".to_string(),
};

let graph_rag = GraphRag::with_config("./project", config)?;

// Index with progress
graph_rag.index_project_with_progress("./src", |progress| {
    println!("Indexed {} / {} files", progress.current, progress.total);
})?;
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

```rust
use clawdius_core::llm::{LlmConfig, Provider};

let config = LlmConfig {
    provider: Provider::OpenAI,
    model: "gpt-4".to_string(),
    api_key: std::env::var("OPENAI_API_KEY")?,
    base_url: Some("https://api.openai.com/v1".to_string()),
    temperature: 0.7,
    max_tokens: 4096,
};

let client = LlmClient::with_config(config)?;
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

### Mock Testing

```rust
use clawdius_core::llm::{LlmClient, MockLlmClient};

#[tokio::test]
async fn test_with_mock() {
    let mut mock = MockLlmClient::new();
    
    mock.expect_complete()
        .returning(|_| Ok(Message::assistant("Mock response")));
    
    let response = mock.complete(vec![
        Message::user("Test")
    ]).await?;
    
    assert_eq!(response.content, "Mock response");
}
```

---

## Error Handling

### Error Types

```rust
use clawdius_core::{Error, Result};

// Library uses thiserror
match graph_rag.semantic_search("test", 10) {
    Ok(results) => println!("Found {} results", results.len()),
    Err(Error::DatabaseError(e)) => eprintln!("Database error: {}", e),
    Err(Error::VectorSearchError(e)) => eprintln!("Search failed: {}", e),
    Err(e) => eprintln!("Other error: {}", e),
}
```

### Application Error Handling

```rust
use anyhow::Context;

fn main() -> anyhow::Result<()> {
    let graph_rag = GraphRag::new("./project")
        .context("Failed to initialize Graph-RAG")?;
    
    let results = graph_rag
        .semantic_search("error handling", 10)
        .context("Search failed")?;
    
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
