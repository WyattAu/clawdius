# 🦀 Clawdius

**The High-Assurance Engineering Engine.**  
*Powered by Rust. Governed by SOPs. Verified by Nexus.*

[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org)
[![Security: Sentinel](https://img.shields.io/badge/Security-Sentinel_JIT-blue.svg)](#-the-sentinel-jit-sandboxing)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**Clawdius** is a next-generation AI agentic engine built for developers who can't afford hallucinations and traders who can't afford latency. While other "claws" run on bloated Node.js runtimes with raw shell access, Clawdius is a native Rust binary that enforces a formal R&D lifecycle and executes code in strictly isolated, just-in-time sandboxes.

---

## 🚀 Why Clawdius?

| Feature      | Clawdius                           | Claude Code / OpenClaw             |
| :----------- | :--------------------------------- | :--------------------------------- |
| **Runtime**  | **Rust** (Zero GC, <20ms boot)     | Node.js (Heavy, Garbage Collected) |
| **Security** | **Sentinel JIT Sandboxing**        | Raw Shell / Local OS Access        |
| **Rigor**    | **Nexus Lifecycle** (Formal Specs) | Stochastic (Guess & Check)         |
| **Context**  | **Graph-RAG** (AST + Vector)       | Simple Vector / RAG                |
| **Trading**  | **Broker Mode** (Sub-ms latency)   | Not Supported / High Latency       |

---

## 🛠 Core Pillars

### 🛡️ The Sentinel (JIT Sandboxing)
Stop letting AI agents run `rm -rf /` on your machine. Clawdius analyzes your project and dynamically spawns the most restrictive environment needed:
- **Tier 1 (Systems):** Bubblewrap/sandbox-exec passthrough for high-performance C++/Rust.
- **Tier 2 (Scripts):** Rootless Podman containers for untrusted Node.js/Python code.
- **Privacy:** Your API keys and SSH secrets stay in the Host memory; they are never visible to the agent.

### 🧠 Graph-RAG Intelligence
Clawdius doesn't just "read" your files; it understands them.
- **Structural:** Uses `tree-sitter` to build a local SQLite graph of your codebase (Who calls whom? What defines what?).
- **Semantic:** LanceDB vector indexing for high-speed retrieval of documentation and intent.
- **Multi-Lingual:** Research SOTA findings across 16 languages (EN/ZH/RU/JP/etc.) with integrated TQA (Translation Quality Assurance).

### 🏗 The Nexus Lifecycle
Clawdius enforces the **Nexus R&D Lifecycle**—a 12-phase transition from Context Discovery to Knowledge Transfer.
- **Yellow Papers:** Theoretical ground truth and mathematical proofs.
- **Blue Papers:** IEEE 1016-compliant architectural specifications.
- **SOPs:** Active Standard Operating Procedures that Clawdius "signs off" on before every commit.

### 📈 The Broker (Financial Guard)
Deploy Clawdius as a 24/7 financial assistant on your server or Mac Mini.
- **Low Latency:** Zero garbage collection pauses for real-time market ingestion.
- **Wallet Guard:** A hard-coded safety interlock that rejects any trade violating your pre-defined risk parameters.
- **Bridge:** Instant reports via Matrix or WhatsApp when a signal is triggered.

---

## 📦 Installation

Clawdius is distributed as a single, static Rust binary.

```bash
# Via Cargo
cargo install clawdius

# Or via Nix
nix shell github:your-org/clawdius
```

---

## 🛠 Getting Started

Initialize Clawdius in your repository:

```bash
clawdius init
```

This creates the `.clawdius/` directory:
- `sops/`: Your project’s common and language-specific rules.
- `specs/`: Where Yellow and Blue papers are generated.
- `graph/`: The local SQLite AST and LanceDB vector store.

### Commands
- `clawd chat`: Start a high-assurance session.
- `clawd refactor`: Plan and execute a cross-language refactor (e.g., TS to Rust).
- `clawd broker`: Activate financial monitoring and trading signals.
- `clawd verify`: Run Lean 4 proofs and SOP compliance checks.

---

## 🏗 The Stack
- **Engine:** Rust (Tokio runtime)
- **Logic:** Wasmtime (Brain isolation)
- **Database:** SQLite (Structural) + LanceDB (Vector)
- **UI:** Ratatui (60FPS Terminal UI)
- **Protocols:** MCP (Model Context Protocol), Matrix, LSP

---

## ⚖️ License
Clawdius is released under the apache 2.0 License. See [LICENSE](LICENSE) for details.

---

## 🦀 Join the Swarm
Clawdius is built by and for Rustaceans. If you value low-latency, high-rigor, and deterministic engineering, we want your PRs.

> **"Clawdius: Build like an Emperor. Protect like a Sentinel."**