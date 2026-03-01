# Clawdius Technical Stack Specification v1.0

## 1. Core Architecture: The "Kernel-Plugin" Split
Clawdius rejects the monolithic execution model. It follows a **Host-Brain-Hands** abstraction to ensure that intelligence (LLM) is decoupled from execution (Sandbox) and governed by the Host (Rust).

### 1.1 The Host (The Kernel)
*   **Language:** Rust (2024 Edition).
*   **Runtime:** `tokio` (Multi-threaded, asynchronous I/O).
*   **State Machine:** A deterministic Finite State Machine (FSM) implemented via Rust’s Typestate pattern to enforce the **Nexus Lifecycle** (Phase -1 to Phase 12).
*   **Serialization:** `serde` for JSON/TOML; `rkyv` for zero-copy binary serialization of the knowledge graph and indices (maximizing low-latency I/O).

### 1.2 The Brain (Logic Layer)
*   **Runtime:** `wasmtime` (WebAssembly).
*   **Logic:** The core "Nexus" reasoning, SOP enforcement, and prompt-construction logic run inside a WASM sandbox. 
*   **Security:** This prevents "Brain-Leaking"—where a compromised LLM response tries to use the Host's memory to escalate privileges. The Brain communicates with the Host via a strict, versioned RPC protocol.

### 1.3 The Hands (Execution Layer)
*   **Native Execution:** `bubblewrap` (Linux) and `sandbox-exec` (macOS). Used for heavy systems tasks (compiling Rust/C++, Vulkan shaders).
*   **Containerized Execution:** `bollard` (Rust interface for Podman/Docker). Used for untrusted environments (Node.js, Python, Ruby).
*   **Isolation:** The "Sentinel" service manages JIT (Just-In-Time) mount points, ensuring the "Hands" only see the files defined in the current Blue Paper scope.

---

## 2. Intelligence & Knowledge Layer

### 2.1 Multi-LLM Orchestration
*   **Provider Interface:** `genai` (Unified crate for OpenAI, Anthropic, Gemini, Groq).
*   **Protocol:** **MCP (Model Context Protocol)**. Clawdius acts as an MCP Host, allowing it to plug into any existing tool (PostgreSQL, GitHub, Slack) natively.
*   **Local Inference:** Support for `llama-cpp-2` and `ollama` for zero-trust, offline-first operation.

### 2.2 The "Graph-RAG" Memory (Knowledge Integration)
Clawdius does not use a simple flat-file index. It uses a **Relational-Semantic Hybrid**.
*   **Structural Index (The Graph):** **SQLite (`rusqlite`)**. Stores the AST (Abstract Syntax Tree) relationships.
    *   *Nodes:* Functions, Structs, Modules, Standards, SOPs.
    *   *Edges:* `CALLS`, `DEFINES`, `IMPLEMENTS`, `COMPLIES_WITH`.
*   **Semantic Index (The Vector):** **LanceDB** (Embedded, Rust-native). Stores embeddings of code chunks and "Nexus" papers.
*   **Parsing Engine:** `tree-sitter` with specific grammars for C++, Rust, TS, and Go.

---

## 3. Communication & Interface

### 3.1 The "Clawdius-Pit" (Terminal UI)
*   **Tech:** `ratatui`.
*   **Performance:** 60fps rendering with zero-flicker streaming.
*   **Features:** Integrated syntax highlighting via `syntect`, real-time DAG visualization of the "Swarm" (active agents), and live "Rigor Score" monitoring.

### 3.2 The Bridge (Notification Gateway)
*   **Matrix/WhatsApp/Telegram:** Handled by the **Host Kernel**.
*   **Protocol:** Matrix-SDK (Rust).
*   **Security:** The API keys for these services are stored in the OS Keychain (via `keyring-rs`) and are never exposed to the LLM or the Sandbox.

---

## 4. Methodology & Rigor Tools

### 4.1 Formal Verification Support
*   **Primary Tool:** **Lean 4**.
*   **Integration:** Clawdius generates `.lean` files in `.clawdius/specs/proofs/`.
*   **Automation:** The Host attempts to run the Lean compiler to verify theorems. If absent, it logs a **Level 2 Warning** and requires a manual "Theoretical Risk" ADR.

### 4.2 Standard Operating Procedures (SOPs)
*   **Format:** Versioned Markdown files in `.clawdius/sops/`.
*   **Checkers:** Custom Rust logic that regex-scans and AST-analyzes proposed code changes against SOP rules (e.g., "No `unwrap()` in production Rust code").

### 4.3 Documentation Engine
*   **Spec Generation:** TOML for requirements; Markdown for Yellow/Blue papers.
*   **Traceability:** A custom Rust utility that cross-references the SQLite Graph with the `.specs/` folder to generate a `TRACEABILITY_MATRIX.md` on every build.

---

## 5. Cross-Platform Abstraction Matrix

| OS | Sandbox Tech | Key Storage | FS Events |
| :--- | :--- | :--- | :--- |
| **Linux** | `bwrap` (Bubblewrap) | Secret Service / Libsecret | `inotify` |
| **macOS** | `sandbox-exec` | Apple Keychain | `fsevent` |
| **Windows** | WSL2 / Hyper-V | Windows Credential Mgr | `read_directory_changes` |

---

## 6. The Execution Pipeline (The Swarm)
Clawdius utilizes an **Actor-based Swarm** for maximum concurrency:
1.  **Scout (Rust):** Parallel tree-sitter parsing and graph indexing.
2.  **Architect (WASM/LLM):** Synthesizes Yellow/Blue papers and SOP checks.
3.  **Sentinel (Rust):** Pre-flight security and sandbox mount configuration.
4.  **Engineer (Podman/Bwrap):** Atomic execution of code changes.
5.  **Auditor (LLM/LSP):** Verifies implementation against the Blue Paper via LSP/Tests.

---

## 7. Build & Distribution
*   **Static Binary:** Compiled with `musl` on Linux for zero-dependency portability.
*   **Package Manager:** Distributed via `cargo install`, Homebrew, and as a standalone Nix Flake.
*   **Size Target:** < 15MB compressed.

---

### **Final Determination**
The Clawdius stack is designed to be **opinionated.** It assumes that the developer wants the highest possible rigor and lowest possible latency. By leveraging **Rust's speed**, **WASM's isolation**, and **SQLite's structural indexing**, Clawdius provides a professional engineering environment that outperforms existing Go/Node.js based agents in every safety and performance metric.