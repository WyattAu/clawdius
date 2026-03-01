# Clawdius: Full System Requirements Specification (SRS) v1.0

## 1. Core Engine & Lifecycle Requirements (The Nexus FSM)
*   **1.1 Deterministic State Machine:** The system **SHALL** implement the 12-phase Nexus R&D lifecycle (Context Discovery through Knowledge Transfer) as a hard-coded Finite State Machine (FSM).
*   **1.2 Typestate Enforcement:** The Rust implementation **SHALL** use the Typestate pattern to prevent illegal phase transitions (e.g., implementation cannot start before Architecture sign-off).
*   **1.3 Atomic Commit Ledger:** Every state change, architectural decision (ADR), and file modification **SHALL** be logged in a `CHANGELOG.md` with a cryptographic hash of the project state.
*   **1.4 Artifact Generation:** The system **SHALL** automatically initialize the `.clawdius/` directory structure, including specialized folders for `specs/`, `sops/`, `graph/`, and `sentinel/`.

## 2. Knowledge & Intelligence Requirements (Graph-RAG)
*   **2.1 Structural AST Indexing:** The system **SHALL** use `tree-sitter` to parse code into an Abstract Syntax Tree (AST) and store relationships in a local SQLite database.
*   **2.2 Semantic Vector Indexing:** The system **SHALL** use `LanceDB` to store vector embeddings of codebase "chunks" and theoretical papers for semantic retrieval.
*   **2.3 Multi-Lingual Knowledge Integration:** The system **SHALL** support the retrieval and synthesis of technical data across 16 languages (EN/ZH/RU/JP, etc.) with a mandatory Translation Quality Assurance (TQA) score.
*   **2.4 MCP Host Support:** The system **SHALL** implement the Model Context Protocol (MCP) to allow Clawdius to utilize third-party tools (databases, search engines) without native code integration.
*   **2.5 Provider Agnosticism:** The system **SHALL** support Anthropic, OpenAI, DeepSeek, and local inference (Ollama/Llama.cpp) via the `genai` unified interface.

## 3. Security & Sandboxing Requirements (The Sentinel)
*   **3.1 Just-In-Time (JIT) Sandboxing:** The system **SHALL** analyze the required toolchain and dynamically spawn the most restrictive sandbox possible:
    *   **Tier 1:** Native Passthrough (Bubblewrap/sandbox-exec) for C++/Rust/Vulkan.
    *   **Tier 2:** Containerized (Podman) for Node.js/Python/Untrusted scripts.
*   **3.2 Brain Isolation:** The LLM reasoning logic (The Brain) **SHALL** run inside a WebAssembly (Wasmtime) sandbox, communicating with the Host Kernel via a strictly versioned RPC.
*   **3.3 Secret Redaction:** API keys and financial credentials **SHALL NOT** be injected into the sandbox environment. The Host Kernel **SHALL** act as the only authorized network proxy for sensitive requests.
*   **3.4 Anti-RCE Validation:** The system **SHALL** validate all `.clawdius/settings.toml` files against a global user-defined safety policy before execution to prevent repository-based remote code execution.

## 4. Methodology & Rigor Requirements (SOPs)
*   **4.1 Active SOP Enforcement:** The system **SHALL** ingest `common.sop.md` and language-specific SOPs (`rust.sop.md`) as "Immutable Constraints" for every generated code block.
*   **4.2 NTIB Identification:** The system **SHALL** automatically flag Non-Trivial Implementation Blocks (NTIB) and halt execution until a Blue Paper (Architecture) is generated.
*   **4.3 ADR Generation:** Every deviation from the SOP or change in project architecture **SHALL** trigger the creation of an Architecture Decision Record (ADR) in TOML format.
*   **4.4 Formal Verification Integration:** For safety-critical logic, the system **SHALL** generate Lean 4 proof scripts and attempt automated verification.

## 5. Domain-Specific Requirements (Coder & Broker)
*   **5.1 Coder: Automated Refactoring:** The system **SHALL** be capable of cross-file refactoring (e.g., TS to Rust) by utilizing the AST Graph to identify all affected call-sites.
*   **5.2 Broker: High-Frequency Ingestion:** The system **SHALL** support WebSocket ingestion of market data with sub-millisecond processing latency.
*   **5.3 Broker: Wallet Guard:** The system **SHALL** implement a "Hard Interlock" that rejects any trade signal that violates pre-defined risk parameters (e.g., max position size, max daily drawdown).
*   **5.4 Broker: Low-Latency Notifications:** Trading signals and reports **SHALL** be dispatched to Matrix/WhatsApp via the Rust Host within 100ms of signal generation.

## 6. Performance & Platform Requirements
*   **6.1 Binary Footprint:** The compiled Clawdius binary **SHALL** be `< 15MB` (compressed) and distributed as a single static file.
*   **6.2 Boot Latency:** The system **SHALL** achieve an interactive TUI state in `< 20ms` from execution.
*   **6.3 Resource Efficiency:** The system **SHALL** maintain a peak idle memory usage of `< 30MB` RAM.
*   **6.4 Cross-Platform PAL:** The system **SHALL** implement a Platform Abstraction Layer (PAL) to provide native sandboxing and credential storage on Linux, macOS, and Windows (via WSL2).

## 7. Interface Requirements (The Clawdius-Pit)
*   **7.1 60FPS TUI:** The terminal interface **SHALL** utilize `ratatui` for high-performance, flicker-free rendering.
*   **7.2 Rigor Score Visualization:** The UI **SHALL** display a real-time "Rigor Score" (0.0 - 1.0) indicating how strictly the current session adheres to the SOPs and Nexus Lifecycle.
*   **7.3 Multi-Agent "Swarm" View:** The UI **SHALL** provide a visual DAG (Directed Acyclic Graph) showing the status of parallel actors (Scout, Architect, Sentinel, Engineer).
*   **7.4 Syntax Highlighting:** All code blocks and diffs **SHALL** be rendered with native terminal syntax highlighting via `syntect`.

---

# Acceptance Criteria (The "Definition of Done")
1.  **Safety:** A malicious `rm -rf /` hallucinated by the LLM is blocked by the Sentinel with a Level 4 Error report.
2.  **Performance:** Clawdius parses a 10,000-file repository and builds an AST Graph in under 5 seconds (on M2/x64 equivalent).
3.  **Rigor:** Every line of code in the final implementation can be traced back to a Blue Paper through the `TRACEABILITY_MATRIX.md`.
4.  **Uptime:** The Broker module on a Mac Mini maintains a 99.9% heartbeat uptime with zero Garbage Collection pauses.