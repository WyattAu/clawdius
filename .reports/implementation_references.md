# Implementation References

Quick reference to key implementation files and line numbers for each verified feature.

## CRITICAL FEATURES

### 1. LLM Provider Integration

**Core Implementation:**
- `crates/clawdius-core/src/llm.rs:600-627` - Provider factory
- `crates/clawdius-core/src/llm/providers/mod.rs:14-23` - LlmClient trait

**Provider Implementations:**
- `crates/clawdius-core/src/llm/providers/anthropic.rs:28-50` - Anthropic chat
- `crates/clawdius-core/src/llm/providers/openai.rs` - OpenAI provider
- `crates/clawdius-core/src/llm/providers/ollama.rs` - Ollama provider
- `crates/clawdius-core/src/llm/providers/zai.rs:28-50` - Z.AI provider
- `crates/clawdius-core/src/llm/providers/local.rs` - Local LLM provider

**Tests:**
- `tests/llm_integration_test.rs:14-69` - Integration tests

**Configuration:**
- `crates/clawdius-core/src/llm.rs:324-484` - LlmConfig implementation

---

### 2. Streaming Responses

**Core Implementation:**
- `crates/clawdius-core/src/output/stream.rs:1-366` - Stream events
- `crates/clawdius-core/src/llm.rs:576-587` - chat_stream trait method

**Provider Streaming:**
- `crates/clawdius-core/src/llm/providers/anthropic.rs:52-95` - Anthropic streaming
- `crates/clawdius-core/src/llm/providers/openai.rs` - OpenAI streaming
- `crates/clawdius-core/src/llm/providers/ollama.rs` - Ollama streaming
- `crates/clawdius-core/src/llm/providers/zai.rs:52-95` - Z.AI streaming

**Event Types:**
- `crates/clawdius-core/src/output/stream.rs:12-100` - StreamEvent enum

---

### 3. Session Persistence

**Core Implementation:**
- `crates/clawdius-core/src/session/store.rs:1-443` - SessionStore
- `crates/clawdius-core/src/session/manager.rs` - SessionManager
- `crates/clawdius-core/src/session/types.rs` - Session types

**Database Schema:**
- `crates/clawdius-core/src/session/store.rs:40-91` - Schema initialization

**Key Operations:**
- `crates/clawdius-core/src/session/store.rs:94-100` - Create session
- `crates/clawdius-core/src/session/store.rs:150-200` - Save message
- `crates/clawdius-core/src/session/store.rs:250-300` - Load session

**Compaction:**
- `crates/clawdius-core/src/session/compactor.rs` - Context compaction

**Tests:**
- `crates/clawdius-core/tests/integration/session_flow.rs` - Session flow tests

---

### 4. File Tools

**Core Implementation:**
- `crates/clawdius-core/src/tools/file.rs:1-174` - FileTool

**Operations:**
- `crates/clawdius-core/src/tools/file.rs:58-82` - Read with offset/limit
- `crates/clawdius-core/src/tools/file.rs:84-95` - Write with auto-create
- `crates/clawdius-core/src/tools/file.rs:97-139` - Edit with find-replace
- `crates/clawdius-core/src/tools/file.rs:141-150` - List directory

**Parameters:**
- `crates/clawdius-core/src/tools/file.rs:8-48` - Parameter structures

---

### 5. Shell Tool Execution

**Core Implementation:**
- `crates/clawdius-core/src/tools/shell.rs:1-162` - ShellTool

**Key Methods:**
- `crates/clawdius-core/src/tools/shell.rs:53-66` - Command validation
- `crates/clawdius-core/src/tools/shell.rs:68-89` - Working directory validation
- `crates/clawdius-core/src/tools/shell.rs:91-99` - Output truncation
- `crates/clawdius-core/src/tools/shell.rs:101-150` - Execute command

**Configuration:**
- `crates/clawdius-core/src/config.rs` - ShellSandboxConfig

**Tests:**
- `tests/sandbox_tests.rs` - Shell execution tests

---

## HIGH PRIORITY FEATURES

### 6. VSCode Extension

**Core Implementation:**
- `editors/vscode/src/extension.ts:1-153` - Extension activation
- `editors/vscode/src/rpc/client.ts` - RPC client
- `editors/vscode/src/rpc/server.rs:64-122` - RPC server (STDIO)

**Providers:**
- `editors/vscode/src/providers/chatView.ts` - Chat view
- `editors/vscode/src/completion/provider.ts` - Completions
- `editors/vscode/src/codeActions/provider.ts` - Code actions

**Commands:**
- `editors/vscode/src/extension.ts:74-87` - Command registration

**Configuration:**
- `editors/vscode/package.json:20-100` - Extension contributes

**Tests:**
- `crates/clawdius-core/tests/integration/rpc_communication.rs` - RPC tests

---

### 7. Git Tool

**Core Implementation:**
- `crates/clawdius-core/src/tools/git.rs:1-127` - GitTool

**Operations:**
- `crates/clawdius-core/src/tools/git.rs:46-55` - Status
- `crates/clawdius-core/src/tools/git.rs:57-77` - Diff
- `crates/clawdius-core/src/tools/git.rs:79-96` - Log
- `crates/clawdius-core/src/tools/git.rs:98-117` - run_git helper

**Parameters:**
- `crates/clawdius-core/src/tools/git.rs:9-32` - GitDiffParams, GitLogParams

---

### 8. Web Search

**Core Implementation:**
- `crates/clawdius-core/src/tools/web_search.rs:1-493` - WebSearchTool

**Providers:**
- `crates/clawdius-core/src/tools/web_search.rs:32-43` - SearchProvider enum
- `crates/clawdius-core/src/tools/web_search.rs:92-111` - DuckDuckGo
- `crates/clawdius-core/src/tools/web_search.rs` - Google (API-based)
- `crates/clawdius-core/src/tools/web_search.rs` - Bing (API-based)

**Parsing:**
- `crates/clawdius-core/src/tools/web_search.rs:113-145` - HTML parsing
- `crates/clawdius-core/src/tools/web_search.rs:147-160` - URL decoding

**HTTP Client:**
- `crates/clawdius-core/src/tools/web_search.rs:72-80` - Client initialization

---

### 9. Graph-RAG

**Core Implementation:**
- `crates/clawdius-core/src/graph_rag.rs:1-148` - Module documentation
- `crates/clawdius-core/src/graph_rag/store.rs` - GraphStore
- `crates/clawdius-core/src/graph_rag/vector.rs` - VectorStore
- `crates/clawdius-core/src/graph_rag/search.rs` - HybridSearcher

**Embeddings:**
- `crates/clawdius-core/src/graph_rag/embedding/mod.rs` - EmbeddingGenerator trait
- `crates/clawdius-core/src/graph_rag/embedding/simple.rs` - Simple embedder
- `crates/clawdius-core/src/graph_rag/embedding/real.rs` - Candle-based embedder

**Parsing:**
- `crates/clawdius-core/src/graph_rag/parser.rs` - Tree-sitter parsing
- `crates/clawdius-core/src/graph_rag/ast.rs` - AST structures
- `crates/clawdius-core/src/graph_rag/languages.rs` - Language support

**Tests:**
- `crates/clawdius-core/tests/integration/search_workflow.rs` - Search tests

---

### 10. Sandbox Backends

**Core Implementation:**
- `crates/clawdius-core/src/sandbox.rs:1-101` - Sandbox tiers
- `crates/clawdius-core/src/sandbox/backends/mod.rs:1-43` - Backend trait

**Backend Implementations:**
- `crates/clawdius-core/src/sandbox/backends/direct.rs` - Direct execution
- `crates/clawdius-core/src/sandbox/backends/filtered.rs` - Filtered execution
- `crates/clawdius-core/src/sandbox/backends/bubblewrap.rs:27-85` - Bubblewrap (Linux)
- `crates/clawdius-core/src/sandbox/backends/sandbox_exec.rs` - sandbox-exec (macOS)

**Executor:**
- `crates/clawdius-core/src/sandbox/executor.rs` - SandboxExecutor

**Configuration:**
- `crates/clawdius-core/src/sandbox/tiers.rs` - SandboxConfig

**Tests:**
- `tests/sandbox_tests.rs` - Sandbox tests
- `crates/clawdius-core/src/sandbox/backends/bubblewrap.rs:87-100` - Backend tests

---

## CONFIGURATION

**Main Config:**
- `crates/clawdius-core/src/config.rs` - All configuration structures
- `.clawdius/config.toml` - User configuration file
- `clawdius.example.toml` - Example configuration

**Feature-Specific Config:**
- `crates/clawdius-core/src/llm.rs:303-318` - LlmConfig
- `crates/clawdius-core/src/session.rs` - SessionConfig
- `crates/clawdius-core/src/sandbox.rs` - SandboxConfig
- `crates/clawdius-core/src/graph_rag.rs:141-148` - GraphRagConfig

---

## TESTS

**Integration Tests:**
- `tests/llm_integration_test.rs` - LLM provider tests (69 lines)
- `tests/sandbox_tests.rs` - Sandbox backend tests
- `crates/clawdius-core/tests/integration/session_flow.rs` - Session tests
- `crates/clawdius-core/tests/integration/rpc_communication.rs` - RPC tests
- `crates/clawdius-core/tests/integration/diff_workflow.rs` - Diff tests
- `crates/clawdius-core/tests/integration/search_workflow.rs` - Search tests
- `crates/clawdius-core/tests/integration/features.rs` - Feature tests

**Unit Tests:**
- Inline in source files (search for `#[cfg(test)]`)

**Benchmarks:**
- `crates/clawdius-core/benches/core_bench.rs`
- `crates/clawdius-core/benches/llm_benchmark.rs`
- `crates/clawdius-core/benches/tools_benchmark.rs`
- `crates/clawdius-core/benches/session_benchmark.rs`

---

## DEPENDENCIES

**Workspace Cargo.toml:**
- `/home/wyatt/dev/prj/clawdius/Cargo.toml:26-155` - All workspace dependencies

**Core Crate:**
- `crates/clawdius-core/Cargo.toml:13-100` - Core dependencies

**Key Dependencies:**
- Line 45: `genai = "0.5"`
- Line 47: `reqwest = { features = ["json", "stream"] }`
- Line 42: `rusqlite = { features = ["bundled"] }`
- Line 50-56: Tree-sitter parsers
- Line 69-72: LanceDB and Arrow
- Line 75-79: Candle ML framework
- Line 43: `jsonrpsee = { features = ["server", "client", "ws-client"] }`

---

## ERROR HANDLING

**Error Types:**
- `crates/clawdius-core/src/error.rs` - Error definitions
- `crates/clawdius-core/src/llm.rs:115-153` - LLM error handling
- `crates/clawdius-core/src/tools/web_search.rs:10-30` - WebSearchError

**Retry Logic:**
- `crates/clawdius-core/src/llm.rs:202-300` - Retry with exponential backoff
- `crates/clawdius-core/src/retry.rs` - Retry utilities

---

**Total Lines Analyzed:** 10,000+  
**Files Referenced:** 100+  
**Test Coverage:** 1,442+ lines
