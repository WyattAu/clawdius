# Clawdius TypeScript Client

> TypeScript/JavaScript bindings for the Clawdius agentic coding engine.
> Status: PLANNED for v1.1.0 | Last updated: 2026-05-30

## Installation

```bash
npm install @clawdius/client
```

## Quickstart

```typescript
import { Client } from "@clawdius/client";

const client = new Client();
const response = await client.chat("Explain this code", { files: ["main.rs"] });
console.log(response.content);
```

## Features (Planned)

- Native TypeScript types with full API coverage
- ESM and CJS module support
- Session management with persistence
- Multi-provider LLM support
- Tool execution with sandboxing
- Streaming responses via async iterators
- VSCode extension integration (via clawdius-code)
- Browser support via WASM build (v1.2.0)

## API Reference (Planned)

### Client
| Method | Description |
|--------|-------------|
| `new Client(options?)` | Initialize client |
| `chat(prompt, options?)` | Send chat message |
| `stream(prompt, options?)` | Stream response chunks |
| `sessions.list()` | List all sessions |
| `sessions.create(title?)` | Create new session |

### Options
```typescript
interface ClientOptions {
  apiKey?: string;
  provider?: "anthropic" | "openai" | "deepseek" | "ollama";
  baseUrl?: string;
  sandbox?: boolean;
}

interface ChatOptions {
  files?: string[];
  tools?: string[];
  systemPrompt?: string;
  temperature?: number;
  maxTokens?: number;
}
```

## Implementation Notes

- Primary: HTTP client communicating with local clawdius daemon (MCP or REST)
- Secondary: WASM build for browser-based usage (v1.2.0)
- Depends on clawdius-code binary for VSCode integration
