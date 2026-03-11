# Clawdius Documentation

Welcome to the official documentation for **Clawdius** — the high-assurance AI coding assistant.

## What is Clawdius?

Clawdius is a next-generation AI agentic engine built for developers who:

- **Can't afford hallucinations** — Formal verification with 104 Lean4 theorems
- **Can't afford latency** — Native Rust, <20ms boot time, zero GC pauses
- **Can't compromise on security** — 7-tier sandboxing with capability tokens

### Key Features

| Feature | Description |
|---------|-------------|
| 🛡️ **Multi-Tier Sandboxing** | 7 backends: WASM, Bubblewrap, Container, gVisor, Firecracker, etc. |
| 🧮 **Formal Verification** | 104 Lean4 theorems proving core algorithms |
| 🚀 **Native Performance** | <20ms cold start, ~100MB memory, zero GC |
| 🏢 **Enterprise Ready** | SSO, audit logging, compliance templates |
| 🔌 **Plugin System** | WASM-based plugins with marketplace |
| 🧠 **Graph-RAG** | Structural + semantic code understanding |

## Quick Links

- **New to Clawdius?** Start with the [Installation Guide](./getting-started/installation.md)
- **Have questions?** Check [GitHub Discussions](https://github.com/clawdius/clawdius/discussions)
- **Found a bug?** Open an [Issue](https://github.com/clawdius/clawdius/issues)
- **Want to contribute?** Read [CONTRIBUTING.md](https://github.com/clawdius/clawdius/blob/main/CONTRIBUTING.md)

## Why Choose Clawdius?

### vs Claude Code / Cursor

| Aspect | Clawdius | Others |
|--------|----------|--------|
| Runtime | Rust (Native) | Node.js/Electron |
| Boot Time | <20ms | 500ms-2s |
| Sandboxing | 7 backends | None/Limited |
| Formal Proofs | 104 theorems | None |
| Enterprise | Full SSO/Audit | Limited |

See [Comparison](./COMPARISON.md) for detailed feature comparison.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Clawdius                              │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────────────┐ │
│  │   CLI   │  │   TUI   │  │  VSCode │  │  JSON-RPC API   │ │
│  └────┬────┘  └────┬────┘  └────┬────┘  └────────┬────────┘ │
│       │            │            │                 │          │
│       └────────────┴────────────┴─────────────────┘          │
│                           │                                   │
│  ┌────────────────────────┴────────────────────────────────┐ │
│  │                    clawdius-core                         │ │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌───────────────┐  │ │
│  │  │   LLM   │ │ Session │ │  Tools  │ │ Graph-RAG     │  │ │
│  │  │  Layer  │ │ Manager │ │ Engine  │ │ (SQLite+Lance)│  │ │
│  │  └─────────┘ └─────────┘ └─────────┘ └───────────────┘  │ │
│  │  ┌─────────────────────────────────────────────────────┐│ │
│  │  │              Sentinel Sandbox Layer                  ││ │
│  │  │  WASM │ Filtered │ Bubblewrap │ Container │ gVisor  ││ │
│  │  └─────────────────────────────────────────────────────┘│ │
│  └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Getting Help

- 📖 **Documentation:** You're reading it!
- 💬 **Discord:** [discord.gg/clawdius](https://discord.gg/clawdius)
- 🐛 **Issues:** [GitHub Issues](https://github.com/clawdius/clawdius/issues)
- ❓ **Q&A:** [GitHub Discussions](https://github.com/clawdius/clawdius/discussions)

## License

Clawdius is released under the [Apache 2.0 License](https://github.com/clawdius/clawdius/blob/main/LICENSE).

---

> **"Clawdius: Build like an Emperor. Protect like a Sentinel."**
