# Clawdius Competitive Comparison Matrix

> Comprehensive feature comparison across 22 AI coding agents.
> Data verified against public repositories, documentation, and pricing pages as of 2026-06-01.
> Clawdius version: v1.0.0-rc.2.

---

## 1. Product Overview

| # | Product | License | Primary Language | Repository | Stars (approx.) | Self-Hosted | First Release |
|---|---------|---------|-----------------|------------|----------------:|:-----------:|:------------:|
| 1 | **Clawdius** | Apache 2.0 | Rust | github.com/WyattAu/clawdius | ~500 | Yes | 2025 |
| 2 | Claude Code | Proprietary | TypeScript (Node.js) | Closed source | N/A | No | 2025-02 |
| 3 | Cursor | Proprietary | TypeScript (Electron) | Closed source | N/A | No | 2023-03 |
| 4 | Aider | Apache 2.0 | Python | github.com/paul-gauthier/aider | ~35K | Yes | 2023-05 |
| 5 | Cline | Apache 2.0 | TypeScript | github.com/cline/cline | ~35K | Yes | 2024 |
| 6 | Devin | Proprietary | Unknown | Closed source | N/A | No (API) | 2024-03 |
| 7 | OpenHands | MIT | Python | github.com/All-Hands-AI/OpenHands | ~45K | Yes | 2024 |
| 8 | SWE-agent | MIT | Python | github.com/princeton-nlp/SWE-agent | ~25K | Yes | 2024-02 |
| 9 | GitHub Copilot | Proprietary | TypeScript | Closed source | N/A | No | 2021-10 |
| 10 | Goose | Apache 2.0 | Go | github.com/block/goose | ~10K | Yes | 2024-12 |
| 11 | Augment Code | Proprietary | TypeScript | Closed source | N/A | No | 2024 |
| 12 | Windsurf (Codeium) | Proprietary | TypeScript (Electron) | Closed source | N/A | No | 2024-09 |
| 13 | Continue | Apache 2.0 | TypeScript | github.com/continuedev/continue | ~20K | Yes | 2023-06 |
| 14 | Zed Editor | AGPL 3.0 / Proprietary | Rust | github.com/zed-industries/zed | ~45K | Yes (AGPL) | 2024-03 |
| 15 | Replit Agent | Proprietary | TypeScript | Closed source | N/A | No | 2024-06 |
| 16 | Shell Agent | Apache 2.0 | Rust | github.com/coder/shell-agent | ~2K | Yes | 2025 |
| 17 | Kilo Code | Proprietary | TypeScript | Closed source | N/A | No | 2025 |
| 18 | Claw Code | MIT | Rust | github.com/ultraworkers/claw-code | ~200 | Yes | 2025 |
| 19 | OpenClaw | MIT | TypeScript (Node.js) | github.com/openclaw/openclaw | ~50 | Yes | 2025 |
| 20 | Amp | Proprietary | Rust | amp.rs | ~15K | No | 2024-10 |
| 21 | Trae (ByteDance) | Proprietary | TypeScript (Electron) | Closed source | N/A | No | 2025-04 |
| 22 | PearAI | Proprietary | TypeScript (Electron) | github.com/trypear/pearai | ~15K | Partial | 2024-08 |

---

## 2. Core Capabilities

| Feature | Clawdius | Claude Code | Cursor | Aider | Cline | Devin | OpenHands | SWE-agent | Copilot | Goose | Augment | Windsurf | Continue | Zed | Replit | Shell Agent | Kilo Code | Claw Code | OpenClaw | Amp | Trae | PearAI |
|---------|:--------:|:-----------:|:------:|:-----:|:-----:|:-----:|:---------:|:---------:|:-------:|:-----:|:-------:|:--------:|:-------:|:---:|:------:|:-----------:|:---------:|:---------:|:--------:|:---:|:----:|:------:|
| Agentic coding (multi-step) | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | No | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Terminal/CLI mode | Yes | Yes | No | Yes | Yes | No | Yes | Yes | No | Yes | No | No | No | No | No | Yes | Yes | Yes | Yes | No | No | No |
| IDE integration | VSCode | VSCode/JetBrains | Native | Term only | VSCode/JetBrains | Cloud IDE | VSCode/JetBrains | CLI | VSCode/JetBrains/Neovim | VSCode/JetBrains/Neovim | VSCode/JetBrains | Native | VSCode/JetBrains | Native | Cloud IDE | Term only | VSCode | Term only | VSCode | Term only | Native | VSCode |
| TUI (terminal UI) | Yes (ratatui) | Yes | No | No | No | No | No | No | No | Yes | No | No | No | No | No | No | No | Yes (rustyline) | No | No | No | No |
| Headless/CI mode | Yes | Yes | No | Yes | Yes | API | Yes | Yes | No | Yes | No | No | No | No | No | Yes | No | No | No | No | No | No |
| Streaming responses | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Session persistence | SQLite | File | File | .aider* | File | Cloud | File | File | Cloud | File | Cloud | Cloud | File | File | Cloud | No | File | JSONL | File | File | Cloud | File |
| Context window management | Auto-compact | Auto | No | Manual | Manual | Auto | Auto | No | Auto | Yes | Yes | Yes | No | No | Auto | No | Auto | Yes | No | No | No | No |
| File watching / drift detection | Yes | No | No | No | No | No | No | No | No | No | No | No | No | No | No | No | No | No | No | No | No | No |

---

## 3. LLM Provider Support

| Provider | Clawdius | Claude Code | Cursor | Aider | Cline | Devin | OpenHands | SWE-agent | Goose | Augment | Windsurf | Continue | Zed | Replit | Shell Agent | Claw Code | OpenClaw | Amp | PearAI |
|----------|:--------:|:-----------:|:------:|:-----:|:-----:|:-----:|:---------:|:---------:|:-----:|:-------:|:--------:|:-------:|:---:|:------:|:-----------:|:---------:|:--------:|:---:|:------:|
| Anthropic Claude | Yes | Yes (only) | Yes | Yes | Yes | Yes | Yes | Config | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Config | Yes | Yes | Yes | Yes |
| OpenAI GPT | Yes | No | Yes | Yes | Yes | Yes | Yes | Config | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Google Gemini | Yes | No | Partial | Yes | Yes | Yes | Yes | Config | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Config | Yes | No | Yes | Yes |
| xAI (Grok) | Yes | No | No | No | No | No | Config | Config | Config | Config | No | No | No | No | No | Yes | No | No | No | No |
| Mistral | Yes | No | No | Yes | Yes | No | Yes | Config | Config | Config | No | Yes | No | No | No | No | No | No | No | No |
| DeepSeek | Yes | No | No | Yes | Yes | No | Yes | Config | Config | Config | No | Yes | No | Yes | No | No | Yes | No | No | No |
| OpenRouter | Yes | No | Partial | Yes | Yes | No | Yes | Config | Yes | Config | No | Yes | No | No | No | Yes | No | Yes | No |
| Ollama (local) | Yes | No | Partial | Yes | Yes | No | Yes | Config | Yes | Yes | No | Yes | Yes | No | No | Yes | No | No | No | No |
| Z.AI | Yes | No | No | No | No | No | No | No | No | No | No | No | No | No | No | No | No | No | No | No |
| Custom/OpenAI-compat | Yes | No | Partial | Yes | Yes | No | Yes | Config | Yes | Config | No | Yes | No | No | No | Yes | No | No | No | Yes |
| Amazon Bedrock | Config | No | Yes | Yes | Yes | No | Yes | No | Config | Config | No | Yes | No | No | No | No | No | No | No | No |
| Azure OpenAI | Config | No | Yes | Yes | Yes | No | Yes | No | Config | Config | No | Yes | No | No | No | No | No | No | No | No |
| Google Vertex AI | Config | No | Yes | Yes | Yes | No | Yes | No | Config | Config | No | Yes | No | No | No | No | No | No | No | No |
| DashScope (Alibaba) | No | No | No | No | No | No | Config | No | Config | No | No | No | No | No | No | Yes | No | No | No | No |
| vLLM / llama.cpp | Config | No | No | Yes | Yes | No | Yes | No | Yes | Config | No | Yes | No | No | No | Config | No | No | No | No |
| **Provider count** | **9+** | **1** | **~6** | **~8** | **~8** | **1** | **~8** | **~4** | **~6** | **~6** | **~3** | **~5** | **~5** | **~4** | **~3** | **~8** | **3** | **4** | **2** | **~5** |

---

## 4. Security and Sandboxing

| Feature | Clawdius | Claude Code | Cursor | Aider | Cline | Devin | OpenHands | SWE-agent | Copilot | Goose | Augment | Windsurf | Continue | Zed | Replit | Shell Agent | Claw Code | OpenClaw | Amp |
|---------|:--------:|:-----------:|:------:|:-----:|:-----:|:-----:|:---------:|:---------:|:-------:|:-----:|:-------:|:--------:|:-------:|:---:|:------:|:-----------:|:---------:|:--------:|:---:|
| Sandboxed execution | 5 backends | None | Partial | None | None | Cloud VM | Docker | Docker/E2B | N/A | Docker | Cloud | Cloud | None | N/A | Container | None | Container detect | None | None |
| WASM sandbox | Yes (Wasmtime) | No | No | No | No | No | No | No | N/A | No | No | No | No | N/A | No | No | No | No | No |
| Bubblewrap sandbox | Yes | No | No | No | No | No | No | No | N/A | No | No | No | No | N/A | No | No | No | No | No |
| gVisor (planned) | Planned | No | No | No | No | No | No | No | N/A | No | No | No | No | N/A | No | No | No | No | No |
| Firecracker (planned) | Planned | No | No | No | No | No | No | No | N/A | No | No | No | No | N/A | No | No | No | No | No |
| Network isolation | Yes | No | No | No | No | Cloud | Yes | Yes | N/A | Yes | Cloud | Cloud | No | N/A | Container | No | No | No | No |
| Command filtering | Yes | No | No | No | No | Yes | No | No | N/A | No | No | No | No | N/A | No | Heuristic | No | No |
| Permission prompts | Yes | Yes | Yes | No | Yes | N/A | No | No | N/A | Yes | Yes | Yes | No | N/A | N/A | Yes | 3 modes | No | No |
| OS keyring storage | Yes | No | No | No | No | N/A | No | No | N/A | No | N/A | N/A | No | N/A | N/A | No | No | No | No |
| Secret redaction | Yes | Yes | No | No | No | N/A | No | No | N/A | No | Yes | No | No | N/A | N/A | Yes | Yes | No | No |
| Encryption at rest | AES-256-GCM | No | No | No | No | N/A | No | No | N/A | No | N/A | N/A | No | N/A | N/A | No | No | No | No |
| Path traversal guard | Canonical path | No | Partial | No | No | N/A | No | No | N/A | No | N/A | N/A | No | N/A | N/A | Partial | No | No | No |
| CVE tracking (deny.toml) | Yes (6 tracked) | N/A | N/A | No | No | N/A | No | No | N/A | No | N/A | N/A | No | N/A | N/A | No | No | N/A | N/A |
| Formal verification | 319 Lean4 thm | None | None | None | None | None | None | None | None | None | None | None | None | None | None | None | None | None | None |

---

## 5. Enterprise and Compliance

| Feature | Clawdius | Claude Code | Cursor | Aider | Cline | Devin | OpenHands | Goose | Windsurf | Augment | Zed |
|---------|:--------:|:-----------:|:------:|:-----:|:-----:|:-----:|:---------:|:-----:|:--------:|:-------:|:---:|
| SSO (SAML 2.0) | Yes | Ent tier | Ent tier | No | No | Ent | No | No | Ent tier | Ent tier | No |
| SSO (OIDC) | Yes | Ent tier | Ent tier | No | No | Ent | No | No | Ent tier | Ent tier | No |
| Okta integration | Yes | Partial | Partial | No | No | Yes | No | No | No | No | No |
| Azure AD | Yes | Partial | Partial | No | No | Yes | No | No | No | No | No |
| GitHub SSO | Yes | Yes | Yes | No | No | Yes | No | No | No | No | No |
| Audit logging | 5 backends | Basic | Basic | No | No | Yes | No | No | Basic | Basic | No |
| SOC 2 template | Yes | No | No | No | No | Yes | No | No | No | No | No |
| HIPAA template | Yes | No | No | No | No | Yes | No | No | No | No | No |
| GDPR template | Yes | No | No | No | No | Yes | No | No | No | No | No |
| Team permissions | 23 permissions | Basic | Basic | No | No | Yes | No | No | Basic | Basic | No |
| Multi-tenant | Yes | No | No | No | No | Yes | No | No | No | No | No |
| Rate limiting | Per-user, per-platform | No | No | No | No | Yes | No | No | No | No | No |

---

## 6. Code Intelligence and Context

| Feature | Clawdius | Claude Code | Cursor | Aider | Cline | Devin | OpenHands | SWE-agent | Goose | Augment | Windsurf | Continue | Zed | Replit | Shell Agent | Claw Code |
|---------|:--------:|:-----------:|:------:|:-----:|:-----:|:-----:|:---------:|:---------:|:-----:|:-------:|:--------:|:-------:|:---:|:------:|:-----------:|:---------:|
| Codebase indexing | Yes (Graph-RAG) | Basic | Yes | Git only | No | Custom | No | No | Yes | Yes | Yes | No | Yes | Yes | No | No |
| Tree-sitter parsing | 5 langs | No | No | No | No | No | No | No | No | No | No | No | Yes | No | No | No |
| Vector search | LanceDB | No | Yes | No | No | Custom | No | No | No | Yes | No | No | No | No | No | No |
| Symbol extraction | Yes | No | Partial | No | No | No | No | No | No | Yes | No | No | Yes | No | No | No |
| Semantic code graph | Yes | No | No | No | No | No | No | No | No | Yes | No | No | No | No | No | No |
| LSP integration | No | No | Yes | No | Yes | No | No | No | Yes | No | Yes | Yes | Native | Yes | No | Partial |
| Multi-repo context | Yes | No | Yes | No | No | Yes | Yes | Yes | No | Yes | No | No | No | No | No | No |
| Multi-lingual research | 16 langs | No | No | No | No | No | No | No | No | No | No | No | No | No | No | No |

---

## 7. Extensibility

| Feature | Clawdius | Claude Code | Cursor | Aider | Cline | Devin | OpenHands | SWE-agent | Goose | Augment | Windsurf | Continue | Zed | Replit | Shell Agent | Claw Code | OpenClaw |
|---------|:--------:|:-----------:|:------:|:-----:|:-----:|:-----:|:---------:|:---------:|:-----:|:-------:|:--------:|:-------:|:---:|:------:|:-----------:|:---------:|:--------:|
| MCP (Model Context Protocol) | Server + Client | Server | No | No | Client | No | No | No | Client | No | No | No | No | No | No | Partial | No |
| Plugin system | WASM + 26 hooks | Custom cmds | VSCode ext | No | VSCode ext | No | Python | No | Yes | No | No | Yes | Extensions | No | No | Registry | No |
| Custom tools | Yes | Yes | Yes | Yes | Yes | No | Yes | No | Yes | Yes | Yes | Yes | Yes | Yes | No | Yes | No |
| Feature flags | 15+ | No | No | No | No | N/A | No | No | No | No | No | No | No | No | No | No | No |
| WASM plugin runtime | Yes (Wasmtime) | No | No | No | No | No | No | No | No | No | No | No | No | No | No | No | No |

---

## 8. Messaging and Gateway

| Platform | Clawdius | Claude Code | Cursor | Aider | Cline | Devin | OpenHands | Goose | Windsurf | Claw Code |
|----------|:--------:|:-----------:|:------:|:-----:|:-----:|:-----:|:---------:|:-----:|:--------:|:---------:|
| Telegram | Yes | No | No | No | No | No | No | No | No | No |
| Discord | Yes | No | No | No | No | No | No | No | No | No |
| Slack | Yes | No | No | No | No | No | No | No | No | No |
| Matrix | Yes | No | No | No | No | No | No | No | No | No |
| Signal | Yes | No | No | No | No | No | No | No | No | No |
| Microsoft Teams | Yes | No | No | No | No | No | No | No | No | No |
| WhatsApp | Yes | No | No | No | No | No | No | No | No | No |
| Rocket.Chat | Yes | No | No | No | No | No | No | No | No | No |
| Webhook/HTTP | Yes | No | No | No | No | API | No | No | No | No |
| IRC | No | No | No | No | No | No | No | No | No | No |
| Email | No | No | No | No | No | No | No | No | No | No |
| **Total adapters** | **9** | **0** | **0** | **0** | **0** | **1** | **0** | **0** | **0** | **0** |

---

## 9. Testing and Verification

| Metric | Clawdius | Claude Code | Cursor | Aider | OpenHands | SWE-agent | Goose | Shell Agent | Claw Code |
|--------|:--------:|:-----------:|:------:|:-----:|:---------:|:---------:|:-----:|:-----------:|:---------:|
| Total tests | 2,565 | N/A (closed) | N/A (closed) | ~1,200 | ~3,500 | ~800 | ~1,500 | ~200 | ~1,100 |
| Property-based tests | 27 (proptest) | Unknown | Unknown | No | No | No | No | No | No |
| Fuzz testing | 5 targets (AFL++) | No | No | No | No | No | No | No | No |
| CI test matrix | Linux, macOS, Win | N/A | N/A | Linux, macOS | Linux | Linux | Linux | Linux | Linux, Win |
| Lean4 formal proofs | 319 thm / 23 files | None | None | None | None | None | None | None | None |
| Code coverage tracked | ~63% | N/A | N/A | No | No | No | No | No | No |
| PGO-optimized builds | Yes | N/A | N/A | No | No | No | No | No | No |
| SBOM generation | CycloneDX | N/A | N/A | No | No | No | No | No | No |

---

## 10. Performance

| Metric | Clawdius | Claude Code | Cursor | Aider | Cline | Goose | Shell Agent | Claw Code | Amp |
|--------|:--------:|:-----------:|:------:|:-----:|:-----:|:-----:|:-----------:|:---------:|:---:|
| Cold boot time | <20ms | ~500ms | ~2s | ~300ms | ~200ms | ~400ms | ~30ms | ~50ms | ~15ms |
| Idle memory | ~100MB | ~200MB | ~500MB | ~150MB | ~250MB | ~180MB | ~40MB | ~50MB | ~30MB |
| Binary size (release) | 25MB + 15MB | N/A | ~300MB | N/A | N/A | ~40MB | ~8MB | ~12MB | ~6MB |
| Runtime | Rust (native) | Node.js | Electron | Python | Electron | Go | Rust (native) | Rust (native) | Rust (native) |
| Cross-platform | Linux, macOS, Win | Linux, macOS, Win | Linux, macOS, Win | Linux, macOS, Win | Linux, macOS, Win | Linux, macOS, Win | Linux | Linux, Win | Linux, macOS |

---

## 11. Distribution

| Channel | Clawdius | Claude Code | Cursor | Aider | Cline | Goose | Amp | Shell Agent | Claw Code |
|----------|:--------:|:-----------:|:------:|:-----:|:-----:|:-----:|:---:|:-----------:|:---------:|
| crates.io | Planned | N/A | N/A | PyPI | npm (VSCode) | npm | N/A | N/A | N/A |
| Homebrew | Formula exists | npm global | DMG/AppImage | pipx | VSCode Marketplace | npm | N/A | N/A | N/A |
| Docker | GHCR | No | No | Yes | No | Yes | N/A | N/A | Yes |
| Nix flake | Yes | No | No | No | No | No | N/A | N/A | No |
| AUR | PKGBUILD | No | AUR (unofficial) | AUR | No | AUR | N/A | N/A | No |
| VSCode Marketplace | Planned (code) | No | Native | No | Native | Extension | Extension | No | No |
| npm | No | npm global | N/A | N/A | VSCode ext | npm | N/A | N/A | N/A |
| pip | No | N/A | N/A | Yes | N/A | N/A | N/A | N/A | N/A |

---

## 12. Pricing

| Plan | Clawdius | Claude Code | Cursor | Aider | Cline | Devin | OpenHands | Goose | Windsurf | Augment | Zed | Amp |
|------|----------|-------------|--------|-------|-------|-------|-----------|-------|----------|---------|-----|-----|
| Free tier | Free (OSS) | No | Limited | Free | Free | No | Free | Free | Limited | No | Free | Free |
| Pro/Individual | N/A (bring API key) | $20/mo or $100/mo | $20/mo | N/A (bring API key) | N/A | $500/mo | N/A (bring API key) | N/A (bring API key) | $15/mo | N/A | N/A | N/A |
| Team | N/A (self-host) | $30/user/mo | $40/user/mo | N/A | N/A | Custom | N/A | N/A | Custom | Custom | Custom | N/A |
| Enterprise | Custom (self-host) | Volume-based | Volume-based | N/A | N/A | Custom | N/A | N/A | Custom | Custom | Custom | Custom |
| API cost model | User pays provider | Included | Included | User pays provider | User pays provider | Included | User pays provider | User pays provider | Included | Included | Included | User pays provider |

**Note:** Clawdius charges no SaaS fee. Users pay their LLM provider directly (Anthropic, OpenAI, DeepSeek, etc.). Self-hosted enterprise deployments incur no per-seat licensing.

---

## 13. Unique Differentiators

### Clawdius

- Only agent with formal mathematical verification (319 Lean4 theorems, 23 proof files)
- Only agent with 5 sandbox backends (WASM, Filtered, Bubblewrap, Container, Sandbox-exec) and 2 planned (gVisor, Firecracker)
- Only agent with 9 messaging platform adapters (Telegram, Discord, Slack, Matrix, Signal, Teams, WhatsApp, Rocket.Chat, Webhook)
- Only agent with compliance templates (SOC 2, HIPAA, GDPR)
- WASM-based plugin system with 26 hook types
- Graph-RAG code intelligence with Tree-sitter parsing (10 languages)
- LSP server (tower-lsp) for IDE integration
- Rust-native with PGO-optimized builds
- AES-256-GCM encryption at rest

### Claude Code

- Tightest Anthropic integration with exclusive model access
- Strongest single-provider agentic coding (Claude 3.5/4 family)
- Built-in macOS integration, git workflow awareness

### Cursor

- Best-in-class IDE-native experience
- Real-time multi-file editing with cursor prediction
- Largest user base among agentic IDEs

### Aider

- Most mature open-source CLI agent
- Broadest language/model compatibility for Python-based tooling
- Git-integrated workflow (auto-commits per edit)

### Devin

- Only fully autonomous cloud-based agent (no local installation)
- Full development environment with browser, terminal, editor in sandboxed VM
- Designed for delegation (task description in, PR out)

### Zed Editor

- Rust-native with sub-15ms boot, GPU-accelerated rendering
- Collaborative editing (CRDT-based)
- Built-in AI assistant with LSP-native integration

### Shell Agent

- Smallest binary (~8MB) among Rust-native agents
- Designed specifically for secure terminal automation

### Claw Code

- Broadest LLM provider support (xAI, DashScope, Ollama, OpenRouter, vLLM, llama.cpp)
- Model-specific parameter adaptation and prompt caching
- Autonomous multi-agent coordination (lanes, policies, workers)

---

## 14. Decision Matrix

| Use Case | Best Fit | Runner-Up |
|----------|----------|-----------|
| Enterprise security-first deployment | Clawdius | Devin |
| Formal verification required | Clawdius | (no alternative) |
| Multi-platform messaging bot | Clawdius | (no alternative) |
| Solo developer, quick CLI coding | Aider | Claude Code |
| IDE-native workflow | Cursor | Windsurf |
| Free/open-source, broad model support | Aider | Continue |
| Fully autonomous (no local install) | Devin | Replit Agent |
| Collaborative editing with AI | Zed | Cursor |
| Rust-native, minimal footprint | Amp | Shell Agent |
| Multi-agent orchestration | Claw Code | OpenHands |
| VSCode extension flexibility | Continue | Cline |
| Research / SWE-bench evaluation | SWE-agent | OpenHands |

---

## 15. Competitive Positioning Summary

Clawdius occupies a unique position at the intersection of **formal verification**, **enterprise security**, and **multi-platform integration**. No other agent in this matrix offers Lean4 mathematical proofs, five sandbox backends, and nine messaging adapters simultaneously.

The nearest competitors by dimension:

| Dimension | Clawdius | Nearest Competitor |
|-----------|----------|-------------------|
| Formal verification | 319 Lean4 theorems | None |
| Sandboxing | 5 backends | OpenHands (Docker), SWE-agent (Docker/E2B) |
| Messaging | 9 adapters | None (all others are CLI/IDE-only) |
| Enterprise SSO | SAML + OIDC + Okta + Azure AD | Devin (enterprise tier) |
| LLM providers | 9+ | Aider (~8), Claw Code (~8), OpenHands (~8) |
| Code intelligence | Graph-RAG + Tree-sitter + LSP | Cursor, Augment, Zed |
| Plugin system | WASM + 26 hooks | Goose, Continue |
| Compliance templates | SOC 2 + HIPAA + GDPR | Devin (SOC 2) |
| Cold boot | <20ms | Amp (~15ms), Shell Agent (~30ms) |

---

## 16. Methodology

- Data collected from public GitHub repositories, official documentation, and pricing pages
- Star counts are approximate and reflect values observed on 2026-06-01
- "Config" entries indicate the feature exists but requires user configuration (not first-class)
- "N/A" indicates the information is not publicly available (closed-source products)
- "Planned" indicates the feature is documented in a public roadmap
- Competitors were selected based on: GitHub trending (AI coding agents category), market presence (VC-funded or >1K stars), and community discussion frequency

---

*Last updated: 2026-06-11 | Clawdius v1.0.0*
