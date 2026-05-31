# Clawdius Client Libraries

> Multi-language client SDKs for the Clawdius agentic coding engine.
> Status: PLANNED for v1.1.0 | Last updated: 2026-05-30

## Available Clients

| Language | Package | Status | Target |
|----------|---------|--------|--------|
| Rust | `clawdius-core` | Stable | v1.0.0 |
| Python | `clawdius` | Planned | v1.1.0 |
| TypeScript | `@clawdius/client` | Planned | v1.1.0 |

## Architecture

All client libraries communicate with the Clawdius engine through the same interfaces:
- **MCP Protocol**: Model Context Protocol for tool integration
- **REST API**: HTTP endpoints for session management and chat
- **IPC**: Unix socket for local high-performance communication

## Design Principles

1. **Minimal dependencies**: Each client should have zero or near-zero transitive dependencies
2. **Type safety**: Full type coverage for all API surfaces
3. **Async native**: First-class async/await support in all languages
4. **Streaming**: All clients support response streaming
5. **Error handling**: Structured error types with actionable messages
