# Clawdius Acceptance Criteria Specification

**Document ID:** AC-CLAWDIUS-001  
**Version:** 1.0.0  
**Phase:** 0 (Requirements Engineering)  
**Created:** 2026-03-01  

---

## Verification Method Legend

| Method | Code | Description |
|--------|------|-------------|
| **Inspection (I)** | I | Visual review of artifacts, documents, or code |
| **Demonstration (D)** | D | Operational test without instrumentation |
| **Test (T)** | T | Instrumented verification with pass/fail criteria |
| **Analysis (A)** | A | Mathematical or logical proof |
| **Measurement (M)** | M | Quantitative performance verification |

---

## 1. Core Engine & Lifecycle Acceptance Criteria

### REQ-1.1: Deterministic State Machine
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-1.1.1 | Given a valid phase sequence, when transition requested, then FSM transitions to correct next phase | T | 100% of 12 phase transitions |
| AC-1.1.2 | Given an invalid phase sequence, when transition requested, then FSM rejects transition | T | 100% rejection of invalid transitions |
| AC-1.1.3 | All 12 phases are defined: Context Discovery → Requirements → Architecture → Implementation → Testing → Deployment → Monitoring → Maintenance → Refactoring → Documentation → Knowledge Transfer | I | All phases documented |
| AC-1.1.4 | Phase state is persisted across system restarts | T | State recovery within 5s |

### REQ-1.2: Typestate Enforcement
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-1.2.1 | Given source code attempts illegal phase transition, when compiled, then compilation fails | T | 100% compile-time rejection |
| AC-1.2.2 | All legal phase transitions compile successfully | T | 100% compile success |
| AC-1.2.3 | Typestate pattern uses zero runtime overhead | A | Zero-size types verified |
| AC-1.2.4 | Phase-specific methods only available in correct phase | T | 100% method availability correctness |

### REQ-1.3: Atomic Commit Ledger
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-1.3.1 | Every state change produces CHANGELOG.md entry | T | 100% coverage |
| AC-1.3.2 | Each entry includes cryptographic hash (SHA-256) | I | Hash present and valid |
| AC-1.3.3 | Hash correctly represents project state | T | Hash verification passes |
| AC-1.3.4 | Ledger append-only (no modification of historical entries) | T | 100% immutability |
| AC-1.3.5 | ADR entries in valid TOML format | I | Schema validation passes |

### REQ-1.4: Artifact Generation
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-1.4.1 | `.clawdius/` directory created on project init | I | Directory exists |
| AC-1.4.2 | `specs/` subdirectory created | I | Directory exists |
| AC-1.4.3 | `sops/` subdirectory created | I | Directory exists |
| AC-1.4.4 | `graph/` subdirectory created | I | Directory exists |
| AC-1.4.5 | `sentinel/` subdirectory created | I | Directory exists |
| AC-1.4.6 | Directory structure matches specification | I | 100% match |

---

## 2. Knowledge & Intelligence Acceptance Criteria

### REQ-2.1: Structural AST Indexing
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-2.1.1 | tree-sitter parses Rust files into AST | T | Parse success rate ≥99% |
| AC-2.1.2 | AST relationships stored in SQLite | I | Database contains nodes/edges |
| AC-2.1.3 | CALLS relationships correctly identified | T | ≥95% accuracy |
| AC-2.1.4 | DEFINES relationships correctly identified | T | ≥99% accuracy |
| AC-2.1.5 | 10,000-file repository indexed in <5s | M | ≤5 seconds on M2/x64 |
| AC-2.1.6 | Incremental re-indexing on file change | T | Change detected <100ms |

### REQ-2.2: Semantic Vector Indexing
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-2.2.1 | Code chunks embedded and stored in LanceDB | T | Embedding generation successful |
| AC-2.2.2 | Semantic query returns relevant results | T | ≥80% relevance in top 10 |
| AC-2.2.3 | Vector search latency <50ms for 1M vectors | M | P99 < 50ms |
| AC-2.2.4 | Theoretical papers indexed alongside code | I | Papers present in index |

### REQ-2.3: Multi-Lingual Knowledge Integration
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-2.3.1 | All 16 languages supported for retrieval | T | Each language query returns results |
| AC-2.3.2 | TQA score calculated for each translation | T | 100% TQA score coverage |
| AC-2.3.3 | Safety-critical translations meet TQA Level 5 | T | Level 5 for critical content |
| AC-2.3.4 | Language auto-detection accuracy ≥95% | T | ≥95% correct detection |

### REQ-2.4: MCP Host Support
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-2.4.1 | MCP protocol implemented per specification | T | Protocol compliance test passes |
| AC-2.4.2 | Third-party tools connectable via MCP | D | ≥3 tools demonstrated |
| AC-2.4.3 | MCP communication isolated from core | T | Isolation boundary verified |

### REQ-2.5: Provider Agnosticism
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-2.5.1 | Anthropic API callable via genai | T | Successful completion |
| AC-2.5.2 | OpenAI API callable via genai | T | Successful completion |
| AC-2.5.3 | DeepSeek API callable via genai | T | Successful completion |
| AC-2.5.4 | Ollama local inference callable via genai | T | Successful completion |
| AC-2.5.5 | Llama.cpp local inference callable via genai | T | Successful completion |
| AC-2.5.6 | Provider switching requires no code change | T | Config-only switch verified |

---

## 3. Security & Sandboxing Acceptance Criteria

### REQ-3.1: JIT Sandboxing
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-3.1.1 | C++ code executes in Tier 1 (bubblewrap) sandbox | T | Bubblewrap invocation verified |
| AC-3.1.2 | Rust code executes in Tier 1 sandbox | T | Bubblewrap invocation verified |
| AC-3.1.3 | Node.js code executes in Tier 2 (Podman) container | T | Podman invocation verified |
| AC-3.1.4 | Python code executes in Tier 2 container | T | Podman invocation verified |
| AC-3.1.5 | Malicious `rm -rf /` blocked by Sentinel | T | 100% block rate |
| AC-3.1.6 | Sandbox escape attempts logged as Level 4 errors | I | Error log verified |

### REQ-3.2: Brain Isolation
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-3.2.1 | Brain logic runs in Wasmtime sandbox | I | WASM execution verified |
| AC-3.2.2 | Brain-Host communication via versioned RPC | T | Version mismatch detected |
| AC-3.2.3 | Brain cannot access host filesystem directly | T | Direct access blocked |
| AC-3.2.4 | Brain cannot access host network directly | T | Direct access blocked |
| AC-3.2.5 | Brain memory isolated from Host | A | WASM isolation verified |

### REQ-3.3: Secret Redaction
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-3.3.1 | API keys not present in sandbox environment | T | Environment inspection clean |
| AC-3.3.2 | Credentials stored in OS keychain | I | Keychain access verified |
| AC-3.3.3 | Host acts as sole network proxy | T | Proxy-only access verified |
| AC-3.3.4 | Secrets wrapped in `Secret<T>` preventing logging | T | Debug output clean |

### REQ-3.4: Anti-RCE Validation
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-3.4.1 | settings.toml validated against safety policy | T | Validation invoked |
| AC-3.4.2 | Malicious settings.toml rejected | T | 100% rejection of malicious configs |
| AC-3.4.3 | Safety policy user-configurable | I | Policy file documented |
| AC-3.4.4 | Rejection produces Level 4 error report | I | Error format verified |

---

## 4. Methodology & Rigor Acceptance Criteria

### REQ-4.1: Active SOP Enforcement
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-4.1.1 | common.sop.md loaded on startup | I | SOP file loaded |
| AC-4.1.2 | rust.sop.md loaded when Rust detected | I | Language SOP loaded |
| AC-4.1.3 | `unwrap()` in production code flagged | T | 100% detection |
| AC-4.1.4 | `expect()` in production code flagged | T | 100% detection |
| AC-4.1.5 | SOP violations prevent code acceptance | T | Violation blocks commit |

### REQ-4.2: NTIB Identification
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-4.2.1 | Complex code blocks identified as NTIB | T | ≥90% detection rate |
| AC-4.2.2 | Execution halts on NTIB detection | T | Halt verified |
| AC-4.2.3 | Blue Paper generation required to proceed | T | Block until paper exists |
| AC-4.2.4 | NTIB criteria configurable | I | Configuration documented |

### REQ-4.3: ADR Generation
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-4.3.1 | SOP deviation triggers ADR creation | T | ADR generated |
| AC-4.3.2 | ADR in valid TOML format | I | Schema validation passes |
| AC-4.3.3 | ADR includes context, decision, consequences | I | All sections present |
| AC-4.3.4 | ADR stored in `.clawdius/specs/adrs/` | I | Location verified |

### REQ-4.4: Formal Verification Integration
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-4.4.1 | Lean 4 proof scripts generated for safety-critical code | I | .lean files created |
| AC-4.4.2 | Automated verification attempted | T | Lean compiler invoked |
| AC-4.4.3 | Verification failure produces Level 2 warning | I | Warning logged |
| AC-4.4.4 | Manual Theoretical Risk ADR required on failure | T | Block until ADR exists |

---

## 5. Domain-Specific Acceptance Criteria

### REQ-5.1: Automated Refactoring
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-5.1.1 | Cross-file call-sites identified via AST Graph | T | ≥95% identification |
| AC-5.1.2 | TS to Rust migration preserves behavior | T | Behavioral equivalence |
| AC-5.1.3 | No orphaned references after refactoring | T | Zero orphaned refs |
| AC-5.1.4 | Refactoring preview before execution | D | Preview displayed |

### REQ-5.2: High-Frequency Ingestion
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-5.2.1 | WebSocket market data ingestion functional | T | Connection established |
| AC-5.2.2 | Processing latency <1ms (sub-millisecond) | M | P99 < 1ms |
| AC-5.2.3 | Zero GC pauses during processing | M | 0µs GC pauses |
| AC-5.2.4 | Market data buffer uses HugePage mmap | I | mmap flags verified |

### REQ-5.3: Wallet Guard
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-5.3.1 | Trade signal exceeding max position size rejected | T | 100% rejection |
| AC-5.3.2 | Trade signal exceeding max drawdown rejected | T | 100% rejection |
| AC-5.3.3 | Risk check completes in <100µs | M | P99 < 100µs |
| AC-5.3.4 | Rejection logged with reason | I | Log entry verified |

### REQ-5.4: Low-Latency Notifications
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-5.4.1 | Matrix notification dispatched | T | Message received |
| AC-5.4.2 | Notification latency <100ms | M | P99 < 100ms |
| AC-5.4.3 | API keys stored in OS keychain | I | Keychain verified |
| AC-5.4.4 | WhatsApp notification dispatched | T | Message received |

---

## 6. Performance & Platform Acceptance Criteria

### REQ-6.1: Binary Footprint
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-6.1.1 | Compressed binary size <15MB | M | <15MB |
| AC-6.1.2 | Binary statically linked (musl on Linux) | I | No dynamic dependencies |
| AC-6.1.3 | Single file distribution | I | Single artifact |
| AC-6.1.4 | Strip symbols in release build | I | Binary stripped |

### REQ-6.2: Boot Latency
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-6.2.1 | Interactive TUI in <20ms | M | P99 < 20ms |
| AC-6.2.2 | First frame rendered <25ms | M | P99 < 25ms |
| AC-6.2.3 | Cold start (no cache) <50ms | M | P99 < 50ms |
| AC-6.2.4 | Warm start <15ms | M | P99 < 15ms |

### REQ-6.3: Resource Efficiency
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-6.3.1 | Idle memory <30MB RAM | M | <30MB |
| AC-6.3.2 | No memory leaks over 24h idle | M | Memory stable ±5% |
| AC-6.3.3 | CPU usage <1% when idle | M | <1% CPU |
| AC-6.3.4 | mimalloc or snmalloc allocator used | I | Allocator verified |

### REQ-6.4: Cross-Platform PAL
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-6.4.1 | Linux sandbox via bubblewrap | T | Sandbox functional |
| AC-6.4.2 | macOS sandbox via sandbox-exec | T | Sandbox functional |
| AC-6.4.3 | Windows sandbox via WSL2 | T | Sandbox functional |
| AC-6.4.4 | Linux keychain via libsecret | T | Keychain functional |
| AC-6.4.5 | macOS keychain via Keychain | T | Keychain functional |
| AC-6.4.6 | Windows keychain via Credential Manager | T | Keychain functional |

---

## 7. Interface Acceptance Criteria

### REQ-7.1: 60FPS TUI
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-7.1.1 | ratatui used for rendering | I | Dependency verified |
| AC-7.1.2 | Frame rate ≥60fps | M | ≥60fps sustained |
| AC-7.1.3 | Zero visible flicker | D | No flicker observed |
| AC-7.1.4 | Frame time variance <2ms | M | Jitter <2ms |

### REQ-7.2: Rigor Score Visualization
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-7.2.1 | Rigor Score displayed in UI | I | Score visible |
| AC-7.2.2 | Score range 0.0-1.0 | T | Range enforced |
| AC-7.2.3 | Score updates in real-time | D | Update <100ms |
| AC-7.2.4 | Score reflects SOP adherence | T | Score correlates with violations |

### REQ-7.3: Multi-Agent Swarm View
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-7.3.1 | DAG visualization of agents | D | DAG displayed |
| AC-7.3.2 | Scout agent status visible | I | Status shown |
| AC-7.3.3 | Architect agent status visible | I | Status shown |
| AC-7.3.4 | Sentinel agent status visible | I | Status shown |
| AC-7.3.5 | Engineer agent status visible | I | Status shown |

### REQ-7.4: Syntax Highlighting
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| AC-7.4.1 | syntect used for highlighting | I | Dependency verified |
| AC-7.4.2 | Rust syntax highlighted correctly | D | Visual verification |
| AC-7.4.3 | Diff syntax highlighted correctly | D | Visual verification |
| AC-7.4.4 | Highlighting latency <5ms | M | P99 < 5ms |

---

## 8. System-Level Acceptance Criteria

### SAC-1: Safety
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| SAC-1.1 | Malicious `rm -rf /` hallucinated by LLM blocked by Sentinel | T | Level 4 Error report generated |

### SAC-2: Performance
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| SAC-2.1 | 10,000-file repository parsed and AST Graph built in <5s | M | <5s on M2/x64 equivalent |

### SAC-3: Rigor
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| SAC-3.1 | Every line of code traceable to Blue Paper via TRACEABILITY_MATRIX.md | I | 100% traceability |

### SAC-4: Uptime
| Criterion ID | Acceptance Criterion | Method | Pass/Fail Threshold |
|--------------|---------------------|--------|---------------------|
| SAC-4.1 | Broker module maintains 99.9% heartbeat uptime | M | 99.9% over 24h |
| SAC-4.2 | Zero GC pauses during operation | M | 0µs GC pauses |

---

## 9. Acceptance Criteria Summary

| Requirement Category | Criteria Count | Test Methods |
|---------------------|----------------|--------------|
| Core Engine | 18 | I, T, A |
| Knowledge | 19 | T, D, M, I |
| Security | 20 | T, I, A |
| Methodology | 15 | T, I |
| Domain-Specific | 14 | T, M, I, D |
| Performance | 18 | M, I, T |
| Interface | 17 | I, D, M, T |
| System-Level | 6 | T, M, I |
| **Total** | **127** | All |

---

**Approval:** All acceptance criteria are measurable and testable. Each criterion maps to a specific requirement and has defined pass/fail thresholds.
