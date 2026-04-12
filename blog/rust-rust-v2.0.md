---
name: Reddit r/rust Post
about: Launch post for Clawdius v2.0.0
title: "Clawdius v2.0.0: Open-source agentic coding with Lean4 proofs and WASM sandboxing"
labels: ""
assignees: ""
---

## Post Content

Title: Clawdius v2.0.0: Open-source agentic coding with Lean4 proofs and WASM sandboxing

Hey r/rust,

Clawdius v2.0.0 is out. It's a Rust CLI for coding with LLMs — chat, generate code, run it in sandboxes. The two things that differentiate it from other coding assistants are formal verification (Lean4 proofs for core invariants) and sandboxed execution for anything the LLM generates.

Architecture

The project is a Cargo workspace with 6 crates:

- `clawdius` — CLI binary (clap-based, ~12K LOC)
- `clawdius-core` — shared library (LLM, sessions, sandboxes, tools, agent system)
- `clawdius-code` — VSCode extension helper binary
- `clawdius-mcp` — MCP stdio server for Claude Desktop interop
- `clawdius-server` — JSON-RPC / LSP / GraphQL server
- `clawdius-webview` — Leptos WASM frontend

LLM support covers 3 working providers (Anthropic, OpenAI, Ollama) plus 2 stubs (DeepSeek, OpenRouter). Protocol stack: JSON-RPC, LSP, MCP, DAP, GraphQL, REST. No Node.js runtime anywhere in the pipeline.

Sandboxing

Generated code doesn't execute as raw shell commands. It runs through one of 7 sandbox backends. Three are functional today:

- **Container** (Docker/Podman) — full process isolation, the default when Docker is available
- **Bubblewrap** (Linux) — namespace-based isolation using kernel features
- **sandbox-exec** (macOS) — macOS native sandbox profile execution

Four more exist but are stubs or experimental: WASM (wasmtime runtime, stub), Filtered (command whitelist), gVisor (planned), Firecracker (experimental, not functional). The executor auto-selects the most restrictive backend available on the current platform.

Formal Verification

142 Lean4 theorems across 11 proof files. They cover:

- Ring buffer lock-free invariants (push/pop index safety, empty-not-full, occupancy bounds)
- Wallet guard risk check completeness (position limits, margin, drawdown)
- Sandbox capability unforgeability and attenuation-only derivation
- FSM state transition monotonicity and deadlock freedom
- Plugin lifecycle properties (state machine correctness)
- Container isolation guarantees

All 142 theorems are proven — zero `sorry`. One axiom remains:

```lean
axiom postulate_signature_unforgeable (t1 t2 : CapabilityToken) :
    t1.signature ≠ t2.signature → t1.id ≠ t2.id ∨ t1.resource ≠ t2.resource
```

This is a cryptographic assumption: that Ed25519 signatures on capability tokens resist forgery. It's unprovable without assuming something about the signature scheme, and it's the same assumption every signature-based security system makes. Down from 42 axioms at the start of the project.

The CI enforces proof integrity on every PR: all `.lean` files must compile, sorry count must be 0, axiom count must be ≤ 2.

What Works

- CLI chat with multi-provider LLM support and streaming output
- Agent-mode code generation with real LLM pipelines and task decomposition
- MCP server (`clawdius-mcp`) for Claude Desktop integration
- 4 IDE plugins: VSCode (1,561 LOC TypeScript), JetBrains (2,453 LOC Kotlin), Neovim (Lua), Emacs (Elisp)
- Graph-RAG context with tree-sitter parsing (Rust, Python, JS, TS, Go)
- Session persistence with SQLite, auto-compaction for long conversations
- `clawdius setup` interactive onboarding wizard
- `clawdius git commit/diff/status` with LLM-generated conventional commits
- Context-window management with tiktoken budgeting

What Doesn't Work Yet

- **Full autonomous coding.** Agent mode generates code but doesn't do Aider-style apply-test-retry loops. You get the generated code, then apply it yourself.
- **IDE-native inline completions.** The completion module exists (LRU cache, FIM templates, language detection) but isn't wired to the IDE plugins yet.
- **Plugin marketplace.** The backend exists (7 REST endpoints, in-memory registry, 20 tests) but there's no frontend UI.
- **gVisor and Firecracker sandboxes.** Backends are defined but not production-ready. Firecracker explicitly marks itself as experimental.

CI and Quality

1,956 tests pass across the workspace (3 skipped on CI: lean binary not installed, headless Chrome available). Clippy pedantic + nursery + deny on unwrap_used, expect_used, panic, todo is configured. Compiler warnings are zero. The release pipeline builds signed binaries for 4 platforms with SBOM generation.

**Known debt:** ~1,200 `unwrap()` calls remain in production code across 109 files. The clippy deny-on-unwrap configuration was intended to enforce zero unwraps but was added after most of the codebase was written, so existing calls weren't fixed incrementally. This is tracked for gradual remediation.

Install

```bash
cargo install clawdius
clawdius setup
clawdius chat
```

With a local model, nothing leaves your machine:

```bash
clawdius chat --provider ollama --model llama3
```

GitHub: https://github.com/WyattAu/clawdius

---

**Note to poster:** Copy the content above the horizontal line. The Lean4 code block and architecture details are the main differentiators for r/rust — lead with those if the discussion goes technical.
