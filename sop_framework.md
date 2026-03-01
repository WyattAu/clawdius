# Clawdius: The SOP Framework Specification

## 1. The SOP Hierarchy
Clawdius will manage SOPs in a tiered structure to ensure that project-specific needs don't override global high-assurance principles.

### A. The "Common" SOP (`common.sop.md`)
*   **Mandate:** Universal high-rigor principles regardless of language.
*   **Core Principles:**
    *   **Clean Hands Protocol:** The LLM cannot write to the filesystem without a validated instruction schema.
    *   **Deterministic State:** All state transitions must be logged in the `CHANGELOG.md` or a state-machine ledger.
    *   **Error Taxonomy:** Mandatory use of the 10-level error classification.
    *   **Traceability:** Every NTIB (Non-Trivial Implementation Block) must trace back to a Yellow or Blue Paper.
    *   **Low Overhead:** Prohibition of "bloat" libraries (e.g., preferring `serde` over heavy reflection).

### B. The "Per-Language" SOPs (`lang/[lang].sop.md`)
*   **Rust SOP:** Zero-cost abstractions, strict ownership/borrowing patterns, `tokio` for async, mandatory `deny(unsafe_code)` unless justified by an ADR.
*   **C++ SOP:** RAII enforcement, Vulkan/CUDA-specific memory alignment, strict avoidance of undefined behavior (UB), use of `std::expected` for error handling.
*   **TypeScript SOP:** Functional purity, strict typing, avoidance of `any`, focus on structural typing patterns that minimize V8 de-optimizations.

---

## 2. Integrated Requirement: The "SOP Review" Loop

To satisfy your requirement that these are "viewed and edited during every project," Clawdius will implement an **SOP-Check quality gate** at the end of every Phase.

### The "Clawdius Workflow" with SOPs:
1.  **Phase Initiation:** Clawdius loads the `common.sop.md` and the relevant `lang.sop.md` into its **Long-Term Context (LTC)**.
2.  **Intent Check:** Before generating a Blue Paper, Clawdius must output a "Compliance Checklist" derived from the SOP.
    *   *Example:* "I am designing the 'Broker' module. Per `common.sop.md`, I am implementing a 'Wallet Guard' to prevent stochastic hallucination of trades."
3.  **The Feedback Loop (Editing):** If the LLM discovers a "New Best Practice" (e.g., a more efficient SIMD pattern for Rust 1.85+), it doesn't just use it; it issues a **SOP-Update Proposal**.
    *   *Prompt:* "I have found a lower-latency RPC pattern than our current SOP. Should I update `rust.sop.md` for this project?"
4.  **Final Sign-off:** No code is committed to the "Hands" (the execution environment) unless the **Sentinel** verifies that the implementation matches the SOP constraints.

---

## 3. Formal Requirement List (Updated for Clawdius)

### R1: Deterministic Knowledge Integration
*   **Requirement:** Clawdius must maintain a local `.clawdius/knowledge_graph/` that maps multi-lingual research findings (EN/ZH/RU/JP/DE) to specific implementation nodes.
*   **Standard:** Every trade signal or code refactor must have a "Source of Truth" hash.

### R2: Active SOP Enforcement
*   **Requirement:** Clawdius must verify that every PR or code change complies with the `common.sop.md`.
*   **Metric:** A "Rigor Score" (0.0 - 1.0) must be calculated for every module based on SOP compliance.

### R3: Low-Latency Execution (The Rust Promise)
*   **Requirement:** The Clawdius binary must utilize a non-blocking `tokio` runtime to manage "The Swarm" (parallel scout/architect/engineer actors).
*   **Constraint:** Zero-GC environment. All heavy data parsing (AST/Market Streams) must be handled by zero-copy deserialization (`serde` + `rkyv`).

### R4: JIT Isolation (The Sentinel)
*   **Requirement:** Clawdius must dynamically switch between **Bubblewrap** (for high-performance C++/Rust toolchains) and **Podman** (for untrusted scripting environments).
*   **Constraint:** Environment variables containing secrets must be kept in the Rust Host memory and never passed to the sandbox.

### R5: The "Yellow/Blue" Document Lifecycle
*   **Requirement:** For any algorithm with a **Complexity Score >= 5**, Clawdius MUST generate a Yellow Paper (Theory) and a Blue Paper (IEEE 1016 Architecture) before implementation.

---

## 4. The SOP Directory Structure in `.clawdius/`

To ensure these are easily accessible and editable, Clawdius will initialize the following in every repo:

```bash
.clawdius/
├── sops/
│   ├── common.sop.md       # Universal Nexus/Clawdius Rules
│   ├── rust.sop.md         # Determinism & Performance in Rust
│   ├── cpp.sop.md          # Low-latency & Hardware optimization
│   └── finance.sop.md      # Risk management & Broker protocols
├── specs/
│   ├── YP-[Domain].md      # Yellow Papers (The "Why")
│   └── BP-[Module].md      # Blue Papers (The "How")
├── graph/                  # SQLite/LanceDB Index
└── sentinel/               # Sandbox logs and capability whitelist
```

## 5. Marketing Positioning for "Clawdius"
You are no longer selling an AI assistant. You are selling a **"Deterministic Engineering Engine."**

> **"Clawdius: High-Assurance AI for those who can't afford hallucinations. 
> Powered by Rust. Governed by SOPs. Verified by Nexus."**

By forcing the user (and the AI) to engage with the **SOPs** every time, you solve the "Lazy Developer" problem. Clawdius ensures that every project—whether it's a C++ game engine or a high-frequency trading bot—is built with the same level of architectural rigor as a flight-control system.

Should we begin detailing the **initial Common SOP** to be baked into the `clawdius init` command?