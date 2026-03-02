# Clawdius Formal Requirements Specification (SRS) v2.0

**Document ID:** SRS-CLAWDIUS-002  
**Version:** 2.0.0  
**Phase:** 0 (Requirements Engineering)  
**Status:** APPROVED  
**Created:** 2026-03-01  
**Standard:** EARS (Easy Approach to Requirements Syntax)  

---

## 1. Core Engine & Lifecycle Requirements (Nexus FSM)

### REQ-1.1: Deterministic State Machine
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-1.1 |
| **Category** | Ubiquitous |
| **EARS Statement** | The system shall implement the 12-phase Nexus R&D lifecycle (Context Discovery through Knowledge Transfer) as a hard-coded Finite State Machine (FSM). |
| **Rationale** | Deterministic lifecycle enforcement prevents ad-hoc development processes and ensures reproducible engineering outcomes. |
| **Verification** | Test, Inspection |
| **Priority** | MUST |
| **Trace To** | DA-CLAWDIUS-001 §2.2 |

### REQ-1.2: Typestate Enforcement
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-1.2 |
| **Category** | State-Driven |
| **EARS Statement** | While in any given lifecycle phase, the system shall use the Typestate pattern to prevent illegal phase transitions (e.g., implementation cannot start before Architecture sign-off). |
| **Rationale** | Compile-time phase transition enforcement eliminates an entire class of runtime errors related to process violations. |
| **Verification** | Analysis, Test |
| **Priority** | MUST |
| **Trace To** | DA-CLAWDIUS-001 §2.2, rust_sop.md §1.2 |

### REQ-1.3: Atomic Commit Ledger
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-1.3 |
| **Category** | Event-Driven |
| **EARS Statement** | When any state change, architectural decision (ADR), or file modification occurs, the system shall log the event in `CHANGELOG.md` with a cryptographic hash of the project state. |
| **Rationale** | Immutable audit trails enable forensic analysis, compliance verification, and reproducibility. |
| **Verification** | Inspection, Test |
| **Priority** | MUST |
| **Trace To** | DA-CLAWDIUS-001 §2.2 |

### REQ-1.4: Artifact Generation
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-1.4 |
| **Category** | Event-Driven |
| **EARS Statement** | When the system initializes a project, the system shall automatically create the `.clawdius/` directory structure including specialized folders for `specs/`, `sops/`, `graph/`, and `sentinel/`. |
| **Rationale** | Standardized directory structure ensures consistent artifact location across all Clawdius projects. |
| **Verification** | Inspection |
| **Priority** | MUST |
| **Trace To** | basic_spec.md §4.3 |

---

## 2. Knowledge & Intelligence Requirements (Graph-RAG)

### REQ-2.1: Structural AST Indexing
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-2.1 |
| **Category** | Ubiquitous |
| **EARS Statement** | The system shall use `tree-sitter` to parse code into an Abstract Syntax Tree (AST) and store relationships in a local SQLite database. |
| **Rationale** | Structural code understanding enables precise refactoring, impact analysis, and cross-reference generation. |
| **Verification** | Test, Demonstration |
| **Priority** | MUST |
| **Trace To** | basic_spec.md §2.2 |

### REQ-2.2: Semantic Vector Indexing
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-2.2 |
| **Category** | Ubiquitous |
| **EARS Statement** | The system shall use `LanceDB` to store vector embeddings of codebase "chunks" and theoretical papers for semantic retrieval. |
| **Rationale** | Semantic search enables natural language queries against codebases and research materials. |
| **Verification** | Test, Demonstration |
| **Priority** | MUST |
| **Trace To** | basic_spec.md §2.2, DA-CLAWDIUS-001 §6.2 |

### REQ-2.3: Multi-Lingual Knowledge Integration
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-2.3 |
| **Category** | Optional |
| **EARS Statement** | Where multi-lingual research support is enabled, the system shall support retrieval and synthesis of technical data across 16 languages (EN/ZH/RU/JP/etc.) with a mandatory Translation Quality Assurance (TQA) score. |
| **Rationale** | SOTA research is published globally; multi-lingual support maximizes knowledge access. |
| **Verification** | Test |
| **Priority** | SHOULD |
| **Trace To** | DA-CLAWDIUS-001 §5 |

### REQ-2.4: MCP Host Support
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-2.4 |
| **Category** | Ubiquitous |
| **EARS Statement** | The system shall implement the Model Context Protocol (MCP) to allow Clawdius to utilize third-party tools (databases, search engines) without native code integration. |
| **Rationale** | MCP enables extensibility without compromising the core security model. |
| **Verification** | Test, Demonstration |
| **Priority** | SHOULD |
| **Trace To** | basic_spec.md §2.1 |

### REQ-2.5: Provider Agnosticism
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-2.5 |
| **Category** | Ubiquitous |
| **EARS Statement** | The system shall support Anthropic, OpenAI, DeepSeek, and local inference (Ollama/Llama.cpp) via the `genai` unified interface. |
| **Rationale** | Provider agnosticism prevents vendor lock-in and enables cost/risk optimization. |
| **Verification** | Test |
| **Priority** | MUST |
| **Trace To** | basic_spec.md §2.1 |

---

## 3. Security & Sandboxing Requirements (Sentinel)

### REQ-3.1: JIT Sandboxing
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-3.1 |
| **Category** | Event-Driven |
| **EARS Statement** | When the system requires code execution, the system shall analyze the required toolchain and dynamically spawn the most restrictive sandbox possible (Tier 1: Native Passthrough for C++/Rust/Vulkan; Tier 2: Containerized for Node.js/Python). |
| **Rationale** | JIT sandboxing provides defense-in-depth against supply chain attacks and LLM hallucinations. |
| **Verification** | Test, Analysis |
| **Priority** | MUST |
| **Trace To** | DA-CLAWDIUS-001 §2.3 (DR-002), basic_spec.md §1.3 |

### REQ-3.2: Brain Isolation
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-3.2 |
| **Category** | Ubiquitous |
| **EARS Statement** | The system shall run LLM reasoning logic (The Brain) inside a WebAssembly (Wasmtime) sandbox, communicating with the Host Kernel via a strictly versioned RPC. |
| **Rationale** | WASM isolation prevents "Brain-Leaking" where compromised LLM responses escalate privileges. |
| **Verification** | Test, Analysis |
| **Priority** | MUST |
| **Trace To** | basic_spec.md §1.2, DA-CLAWDIUS-001 §6.2 |

### REQ-3.3: Secret Redaction
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-3.3 |
| **Category** | Unwanted Behavior |
| **EARS Statement** | The system shall not inject API keys and financial credentials into the sandbox environment. The Host Kernel shall act as the only authorized network proxy for sensitive requests. |
| **Rationale** | Credential isolation prevents exfiltration via compromised dependencies or malicious LLM outputs. |
| **Verification** | Test, Inspection |
| **Priority** | MUST |
| **Trace To** | basic_spec.md §3.2, rust_sop.md §2.2 |

### REQ-3.4: Anti-RCE Validation
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-3.4 |
| **Category** | Event-Driven |
| **EARS Statement** | When loading any `.clawdius/settings.toml` file, the system shall validate the file against a global user-defined safety policy before execution to prevent repository-based remote code execution. |
| **Rationale** | Repository-based RCE is a critical attack vector in AI agent systems. |
| **Verification** | Test, Analysis |
| **Priority** | MUST |
| **Trace To** | DA-CLAWDIUS-001 §2.3 (DR-002) |

---

## 4. Methodology & Rigor Requirements (SOPs)

### REQ-4.1: Active SOP Enforcement
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-4.1 |
| **Category** | Ubiquitous |
| **EARS Statement** | The system shall ingest `common.sop.md` and language-specific SOPs (`rust.sop.md`) as "Immutable Constraints" for every generated code block. |
| **Rationale** | Automated SOP enforcement ensures consistent code quality without manual review overhead. |
| **Verification** | Test, Inspection |
| **Priority** | MUST |
| **Trace To** | basic_spec.md §4.2, rust_sop.md |

### REQ-4.2: NTIB Identification
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-4.2 |
| **Category** | Event-Driven |
| **EARS Statement** | When the system identifies a Non-Trivial Implementation Block (NTIB), the system shall halt execution until a Blue Paper (Architecture) is generated. |
| **Rationale** | Complex implementations require explicit architectural review to prevent technical debt. |
| **Verification** | Test, Demonstration |
| **Priority** | MUST |
| **Trace To** | DA-CLAWDIUS-001 §2.2 |

### REQ-4.3: ADR Generation
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-4.3 |
| **Category** | Event-Driven |
| **EARS Statement** | When any deviation from the SOP or change in project architecture occurs, the system shall create an Architecture Decision Record (ADR) in TOML format. |
| **Rationale** | ADRs provide audit trails for architectural decisions and enable retrospective analysis. |
| **Verification** | Inspection |
| **Priority** | SHOULD |
| **Trace To** | DA-CLAWDIUS-001 §2.2 |

### REQ-4.4: Formal Verification Integration
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-4.4 |
| **Category** | Optional |
| **EARS Statement** | Where safety-critical logic is identified, the system shall generate Lean 4 proof scripts and attempt automated verification. |
| **Rationale** | Formal verification provides mathematical guarantees for critical code paths. |
| **Verification** | Test, Analysis |
| **Priority** | SHOULD |
| **Trace To** | basic_spec.md §4.1 |

---

## 5. Domain-Specific Requirements (Coder & Broker)

### REQ-5.1: Automated Refactoring
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-5.1 |
| **Category** | Ubiquitous |
| **EARS Statement** | The system shall be capable of cross-file refactoring (e.g., TS to Rust) by utilizing the AST Graph to identify all affected call-sites. |
| **Rationale** | AST-aware refactoring prevents incomplete migrations and runtime errors. |
| **Verification** | Test, Demonstration |
| **Priority** | SHOULD |
| **Trace To** | basic_spec.md §2.2 |

### REQ-5.2: High-Frequency Ingestion
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-5.2 |
| **Category** | Optional |
| **EARS Statement** | Where Broker mode is active, the system shall support WebSocket ingestion of market data with sub-millisecond processing latency. |
| **Rationale** | HFT applications require deterministic low-latency data processing. |
| **Verification** | Test, Analysis |
| **Priority** | SHOULD |
| **Trace To** | DA-CLAWDIUS-001 §3.3 (HC-001) |

### REQ-5.3: Wallet Guard
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-5.3 |
| **Category** | Optional |
| **EARS Statement** | Where Broker mode is active, the system shall implement a "Hard Interlock" that rejects any trade signal violating pre-defined risk parameters (max position size, max daily drawdown). |
| **Rationale** | Hard risk limits prevent catastrophic losses from algorithmic errors. |
| **Verification** | Test, Analysis |
| **Priority** | MUST (Broker) |
| **Trace To** | DA-CLAWDIUS-001 §3.2 (SEC Rule 15c3-5) |

### REQ-5.4: Low-Latency Notifications
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-5.4 |
| **Category** | Optional |
| **EARS Statement** | Where Broker mode is active, trading signals and reports shall be dispatched to Matrix/WhatsApp via the Rust Host within 100ms of signal generation. |
| **Rationale** | Timely notifications enable human oversight of automated trading. |
| **Verification** | Test, Measurement |
| **Priority** | SHOULD |
| **Trace To** | basic_spec.md §3.2 |

---

## 6. Performance & Platform Requirements

### REQ-6.1: Binary Footprint
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-6.1 |
| **Category** | Ubiquitous |
| **EARS Statement** | The compiled Clawdius binary shall be less than 15MB (compressed) and distributed as a single static file. |
| **Rationale** | Minimal binary size reduces deployment complexity and attack surface. |
| **Verification** | Inspection, Measurement |
| **Priority** | SHOULD |
| **Trace To** | basic_spec.md §7 |

### REQ-6.2: Boot Latency
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-6.2 |
| **Category** | Event-Driven |
| **EARS Statement** | When the system is executed, the system shall achieve an interactive TUI state in less than 20ms from execution. |
| **Rationale** | Fast startup enables seamless integration into developer workflows. |
| **Verification** | Measurement |
| **Priority** | SHOULD |
| **Trace To** | basic_spec.md §7 |

### REQ-6.3: Resource Efficiency
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-6.3 |
| **Category** | State-Driven |
| **EARS Statement** | While in idle state, the system shall maintain peak memory usage of less than 30MB RAM. |
| **Rationale** | Low resource usage enables coexistence with heavy development workloads. |
| **Verification** | Measurement |
| **Priority** | SHOULD |
| **Trace To** | DA-CLAWDIUS-001 §6.2 |

### REQ-6.4: Cross-Platform PAL
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-6.4 |
| **Category** | Ubiquitous |
| **EARS Statement** | The system shall implement a Platform Abstraction Layer (PAL) to provide native sandboxing and credential storage on Linux, macOS, and Windows (via WSL2). |
| **Rationale** | Cross-platform support maximizes developer accessibility. |
| **Verification** | Test |
| **Priority** | SHOULD |
| **Trace To** | basic_spec.md §5 |

---

## 7. Interface Requirements (Clawdius-Pit TUI)

### REQ-7.1: 60FPS TUI
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-7.1 |
| **Category** | State-Driven |
| **EARS Statement** | While the terminal interface is active, the system shall utilize `ratatui` for high-performance, flicker-free rendering at 60 frames per second. |
| **Rationale** | Smooth UI rendering improves user experience and perceived responsiveness. |
| **Verification** | Demonstration, Measurement |
| **Priority** | SHOULD |
| **Trace To** | basic_spec.md §3.1 |

### REQ-7.2: Rigor Score Visualization
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-7.2 |
| **Category** | State-Driven |
| **EARS Statement** | While the terminal interface is active, the UI shall display a real-time "Rigor Score" (0.0 - 1.0) indicating how strictly the current session adheres to SOPs and Nexus Lifecycle. |
| **Rationale** | Visible quality metrics encourage adherence to engineering standards. |
| **Verification** | Demonstration |
| **Priority** | SHOULD |
| **Trace To** | basic_spec.md §3.1 |

### REQ-7.3: Multi-Agent Swarm View
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-7.3 |
| **Category** | State-Driven |
| **EARS Statement** | While the terminal interface is active, the UI shall provide a visual DAG (Directed Acyclic Graph) showing the status of parallel actors (Scout, Architect, Sentinel, Engineer). |
| **Rationale** | Agent visualization enables debugging and optimization of parallel workflows. |
| **Verification** | Demonstration |
| **Priority** | COULD |
| **Trace To** | basic_spec.md §6 |

### REQ-7.4: Syntax Highlighting
| Attribute | Value |
|-----------|-------|
| **ID** | REQ-7.4 |
| **Category** | Ubiquitous |
| **EARS Statement** | The system shall render all code blocks and diffs with native terminal syntax highlighting via `syntect`. |
| **Rationale** | Syntax highlighting improves code readability and reduces errors. |
| **Verification** | Demonstration |
| **Priority** | SHOULD |
| **Trace To** | basic_spec.md §3.1 |

---

## 8. Requirements Summary

| Category | Count | MUST | SHOULD | COULD |
|----------|-------|------|--------|-------|
| Core Engine | 4 | 4 | 0 | 0 |
| Knowledge | 5 | 3 | 2 | 0 |
| Security | 4 | 4 | 0 | 0 |
| Methodology | 4 | 2 | 2 | 0 |
| Domain-Specific | 4 | 1 | 3 | 0 |
| Performance | 4 | 0 | 4 | 0 |
| Interface | 4 | 0 | 3 | 1 |
| **Total** | **29** | **14** | **14** | **1** |

---

## 9. EARS Category Distribution

| EARS Category | Count | Percentage |
|---------------|-------|------------|
| Ubiquitous | 14 | 48.3% |
| State-Driven | 5 | 17.2% |
| Event-Driven | 7 | 24.1% |
| Optional | 6 | 20.7% |
| Unwanted Behavior | 1 | 3.4% |

---

**Approval:** This requirements specification establishes the formal foundation for Clawdius development. All subsequent phases must trace back to requirements defined herein.

**Next Phase:** 1 (Architecture - Yellow Paper Generation)
