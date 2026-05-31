# Clawdius Client Libraries

> Multi-language client SDKs for the Clawdius agentic coding engine.
> Status: PLANNED for v1.1.0 | Last updated: 2026-05-31

## SDK Strategy

Clawdius provides client SDKs that wrap the `clawdius-core` Rust library (v1.0.0-rc.2) with idiomatic interfaces for each target language. All SDKs communicate with the Clawdius engine through one of three transport layers:

| Transport | Protocol | Use Case |
|-----------|----------|----------|
| **MCP (Model Context Protocol)** | JSON-RPC 2.0 over stdio/HTTP/WS | Tool integration, IDE extension |
| **REST API** | HTTP + JSON | Session management, chat completion |
| **IPC** | Unix domain socket | Local high-performance communication |

The MCP protocol version is `2025-03-26` (see `clawdius-core/src/mcp/protocol.rs`).

## Available Clients

| Language | Package | Status | Target Version |
|----------|---------|--------|----------------|
| Rust | `clawdius-core` | Stable | v1.0.0-rc.2 |
| Python | `clawdius-client` | Planned | v1.1.0 |
| TypeScript | `@clawdius/sdk` | Planned | v1.1.0 |

## Installation Matrix

```
Python:     pip install clawdius-client
TypeScript: npm install @clawdius/sdk
Rust:       (workspace dependency, not published separately yet)
```

## Quickstart

### Python

```python
import asyncio
from clawdius import Client, LlmConfig

async def main():
    client = Client(
        config=LlmConfig.from_env("anthropic"),
    )
    response = await client.chat("Explain this Rust function")
    print(response.text)

asyncio.run(main())
```

### TypeScript

```typescript
import { Client } from "@clawdius/sdk";

const client = new Client({ provider: "anthropic" });
const response = await client.chat("Explain this Rust function");
console.log(response.text);
```

### Rust (native)

```rust
use clawdius_core::llm::{LlmConfig, create_provider, ChatMessage, ChatRole};
use clawdius_core::llm::providers::LlmClient;

#[tokio::main]
async fn main() -> clawdius_core::Result<()> {
    let config = LlmConfig::from_env("anthropic")?;
    let provider = create_provider(&config)?;
    let messages = vec![ChatMessage {
        role: ChatRole::User,
        content: "Explain Rust ownership".into(),
    }];
    let response = provider.chat(messages).await?;
    Ok(())
}
```

## Core Concepts

### LLM Providers

All SDKs support the same providers, matching `clawdius-core/src/llm.rs`:

| Provider | Config Key | Required Env |
|----------|-----------|-------------|
| Anthropic (Claude) | `anthropic` | `ANTHROPIC_API_KEY` |
| OpenAI (GPT) | `openai` | `OPENAI_API_KEY` |
| OpenRouter | `openrouter` | `OPENROUTER_API_KEY` |
| Ollama (local) | `ollama` | (none, requires local daemon) |
| Z.AI | `zai` | `ZAI_API_KEY` |

### Session Management

Sessions map directly to `clawdius-core::session::SessionStore`. Features:

- SQLite-backed persistence (`.clawdius/sessions.db`)
- Automatic context compaction when token count approaches the configured threshold
- Metadata tracking (title, creation time, project context)
- Message history with role tagging (`User`, `Assistant`, `System`, `Tool`)

### Tool Execution

Tools map to `clawdius-core::tools::Tool` with JSON Schema parameter definitions. Available tools:

- **Shell**: Sandboxed command execution (see `clawdius-core::sandbox`)
- **File**: Read/write/edit operations
- **Git**: Version control operations
- **Browser**: Web automation
- **Web Search**: Information retrieval
- **Editor**: External editor integration

### Sandbox Tiers

All SDKs respect the four-tier sandbox model from `clawdius-core::sandbox::SandboxTier`:

| Tier | Name | Isolation | Use Case |
|------|------|-----------|----------|
| 1 | `TrustedAudited` | None | Audited Rust/C++ build scripts |
| 2 | `Trusted` | Blocklist | Trusted Python/Node.js |
| 3 | `Untrusted` | OS-level (bubblewrap, gVisor) | LLM-generated code |
| 4 | `Hardened` | VM (Firecracker) + no-network | Completely untrusted code |

## API Compatibility Guarantees

| Guarantee | Scope |
|-----------|-------|
| Semver 2.0 | All published packages follow semantic versioning |
| Backward compatible minor releases | Additive API changes only |
| Deprecated before removal | 2-release deprecation window minimum |
| Transport stability | MCP protocol (`2025-03-26`) remains stable within a major version |
| Error type stability | Error variants only added, never removed |

## Versioning Policy

- **Rust (`clawdius-core`)**: Follows workspace version in `Cargo.toml` (currently `1.0.0-rc.2`)
- **Python (`clawdius-client`)**: Matches Rust major.minor, independent patch
- **TypeScript (`@clawdius/sdk`)**: Matches Rust major.minor, independent patch
- All SDKs within the same major version are wire-compatible with each other

## Design Principles

1. **Minimal dependencies**: Zero or near-zero transitive dependencies per client
2. **Type safety**: Full type coverage for all API surfaces, including error variants
3. **Async native**: First-class async/await in all languages
4. **Streaming**: All clients support response streaming (async generators / AsyncIterator)
5. **Structured errors**: Error types mirror `clawdius-core::Error` variants (Config, Llm, RateLimited, SessionNotFound, ToolExecution, etc.)

## Documentation

- [Python SDK](./python_client.md) -- Full API reference and examples
- [TypeScript SDK](./typescript_client.md) -- Full API reference and examples
