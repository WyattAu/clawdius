# Clawdius MoSCoW Priority Matrix

**Document ID:** MOSCOW-CLAWDIUS-001  
**Version:** 1.0.0  
**Phase:** 0 (Requirements Engineering)  
**Created:** 2026-03-01  

---

## 1. MoSCoW Classification Legend

| Priority | Definition | Impact if Excluded |
|----------|------------|-------------------|
| **MUST** | Non-negotiable requirement; system cannot function without it | System failure |
| **SHOULD** | High priority; should be included if possible | Significant degradation |
| **COULD** | Desirable; include if resources permit | Minor degradation |
| **WON'T** | Acknowledged but not in current scope | Future consideration |

---

## 2. Requirements Priority Matrix

### 2.1 MUST Requirements (14 total)

| ID | Requirement | Rationale | Dependencies |
|----|-------------|-----------|--------------|
| REQ-1.1 | Deterministic State Machine | Core lifecycle enforcement | None |
| REQ-1.2 | Typestate Enforcement | Compile-time safety | REQ-1.1 |
| REQ-1.3 | Atomic Commit Ledger | Audit trail | None |
| REQ-1.4 | Artifact Generation | Project structure | None |
| REQ-2.1 | Structural AST Indexing | Code understanding | None |
| REQ-2.2 | Semantic Vector Indexing | Knowledge retrieval | REQ-2.1 |
| REQ-2.5 | Provider Agnosticism | Vendor independence | None |
| REQ-3.1 | JIT Sandboxing | Security foundation | None |
| REQ-3.2 | Brain Isolation | LLM containment | None |
| REQ-3.3 | Secret Redaction | Credential security | None |
| REQ-3.4 | Anti-RCE Validation | Supply chain security | None |
| REQ-4.1 | Active SOP Enforcement | Quality assurance | None |
| REQ-4.2 | NTIB Identification | Architecture review | None |
| REQ-5.3 | Wallet Guard (Broker) | Risk management | Broker mode |

### 2.2 SHOULD Requirements (14 total)

| ID | Requirement | Rationale | Dependencies |
|----|-------------|-----------|--------------|
| REQ-2.3 | Multi-Lingual Integration | Global research access | REQ-2.2 |
| REQ-2.4 | MCP Host Support | Extensibility | None |
| REQ-4.3 | ADR Generation | Decision documentation | REQ-1.3 |
| REQ-4.4 | Formal Verification | Safety proofs | Lean 4 |
| REQ-5.1 | Automated Refactoring | Migration support | REQ-2.1 |
| REQ-5.2 | High-Frequency Ingestion | HFT capability | Broker mode |
| REQ-5.4 | Low-Latency Notifications | Alert delivery | Broker mode |
| REQ-6.1 | Binary Footprint | Deployment simplicity | None |
| REQ-6.2 | Boot Latency | User experience | None |
| REQ-6.3 | Resource Efficiency | Coexistence | None |
| REQ-6.4 | Cross-Platform PAL | Platform support | None |
| REQ-7.1 | 60FPS TUI | UI responsiveness | None |
| REQ-7.2 | Rigor Score Visualization | Quality visibility | REQ-4.1 |
| REQ-7.4 | Syntax Highlighting | Code readability | None |

### 2.3 COULD Requirements (1 total)

| ID | Requirement | Rationale | Dependencies |
|----|-------------|-----------|--------------|
| REQ-7.3 | Multi-Agent Swarm View | Debugging visibility | REQ-7.1 |

### 2.4 WON'T Requirements (Current Release)

| ID | Requirement | Reason | Future Target |
|----|-------------|--------|---------------|
| N/A | Web Dashboard | Terminal-first approach | v2.0 |
| N/A | Mobile Companion App | Desktop focus | v3.0 |
| N/A | Cloud Sync | Local-first design | v2.5 |
| N/A | Multi-Project Workspace | Single project focus | v2.0 |

---

## 3. Category-wise Priority Distribution

### 3.1 Core Engine & Lifecycle

| ID | Requirement | Priority | Phase Target |
|----|-------------|----------|--------------|
| REQ-1.1 | Deterministic State Machine | MUST | Phase 1 |
| REQ-1.2 | Typestate Enforcement | MUST | Phase 1 |
| REQ-1.3 | Atomic Commit Ledger | MUST | Phase 1 |
| REQ-1.4 | Artifact Generation | MUST | Phase 1 |

### 3.2 Knowledge & Intelligence

| ID | Requirement | Priority | Phase Target |
|----|-------------|----------|--------------|
| REQ-2.1 | Structural AST Indexing | MUST | Phase 2 |
| REQ-2.2 | Semantic Vector Indexing | MUST | Phase 2 |
| REQ-2.3 | Multi-Lingual Integration | SHOULD | Phase 4 |
| REQ-2.4 | MCP Host Support | SHOULD | Phase 3 |
| REQ-2.5 | Provider Agnosticism | MUST | Phase 2 |

### 3.3 Security & Sandboxing

| ID | Requirement | Priority | Phase Target |
|----|-------------|----------|--------------|
| REQ-3.1 | JIT Sandboxing | MUST | Phase 3 |
| REQ-3.2 | Brain Isolation | MUST | Phase 3 |
| REQ-3.3 | Secret Redaction | MUST | Phase 3 |
| REQ-3.4 | Anti-RCE Validation | MUST | Phase 3 |

### 3.4 Methodology & Rigor

| ID | Requirement | Priority | Phase Target |
|----|-------------|----------|--------------|
| REQ-4.1 | Active SOP Enforcement | MUST | Phase 2 |
| REQ-4.2 | NTIB Identification | MUST | Phase 2 |
| REQ-4.3 | ADR Generation | SHOULD | Phase 2 |
| REQ-4.4 | Formal Verification | SHOULD | Phase 4 |

### 3.5 Domain-Specific

| ID | Requirement | Priority | Phase Target |
|----|-------------|----------|--------------|
| REQ-5.1 | Automated Refactoring | SHOULD | Phase 5 |
| REQ-5.2 | High-Frequency Ingestion | SHOULD | Phase 6 |
| REQ-5.3 | Wallet Guard | MUST (Broker) | Phase 6 |
| REQ-5.4 | Low-Latency Notifications | SHOULD | Phase 6 |

### 3.6 Performance & Platform

| ID | Requirement | Priority | Phase Target |
|----|-------------|----------|--------------|
| REQ-6.1 | Binary Footprint | SHOULD | Phase 7 |
| REQ-6.2 | Boot Latency | SHOULD | Phase 7 |
| REQ-6.3 | Resource Efficiency | SHOULD | Phase 7 |
| REQ-6.4 | Cross-Platform PAL | SHOULD | Phase 5 |

### 3.7 Interface

| ID | Requirement | Priority | Phase Target |
|----|-------------|----------|--------------|
| REQ-7.1 | 60FPS TUI | SHOULD | Phase 5 |
| REQ-7.2 | Rigor Score Visualization | SHOULD | Phase 5 |
| REQ-7.3 | Multi-Agent Swarm View | COULD | Phase 8 |
| REQ-7.4 | Syntax Highlighting | SHOULD | Phase 5 |

---

## 4. Implementation Phase Mapping

```
Phase 1 (Foundation):     MUST x4  [REQ-1.1, REQ-1.2, REQ-1.3, REQ-1.4]
Phase 2 (Intelligence):   MUST x3, SHOULD x2, MUST x1
Phase 3 (Security):       MUST x4
Phase 4 (Rigor):          SHOULD x2
Phase 5 (Interface):      SHOULD x4, COULD x1
Phase 6 (Broker):         MUST x1, SHOULD x2
Phase 7 (Performance):    SHOULD x4
Phase 8 (Polish):         COULD x1
```

---

## 5. Priority Conflict Resolution

| Conflict | Higher Priority | Rationale |
|----------|-----------------|-----------|
| REQ-5.2 vs REQ-6.1 | REQ-5.2 (latency) | HFT requires performance over binary size |
| REQ-7.3 vs REQ-6.2 | REQ-6.2 (boot) | Startup time affects all users |
| REQ-2.3 vs REQ-2.4 | REQ-2.4 (MCP) | MCP enables more integrations |

---

## 6. Risk-based Priority Adjustment

| Risk Factor | Priority Adjustment | Affected Requirements |
|-------------|---------------------|----------------------|
| Security vulnerability | +1 priority level | REQ-3.x |
| Performance regression | +1 priority level | REQ-5.2, REQ-6.x |
| Compliance requirement | MUST assignment | REQ-1.3, REQ-5.3 |

---

## 7. Summary Statistics

| Priority | Count | Percentage |
|----------|-------|------------|
| MUST | 14 | 48.3% |
| SHOULD | 14 | 48.3% |
| COULD | 1 | 3.4% |
| WON'T | 0 | 0% |
| **Total** | **29** | **100%** |

---

## 8. Approval Gates

| Gate | Required MUST Completion | Required SHOULD Completion |
|------|-------------------------|---------------------------|
| Alpha | 50% (7/14) | 25% (3/14) |
| Beta | 100% (14/14) | 75% (10/14) |
| Release | 100% (14/14) | 100% (14/14) |

---

**Approval:** MoSCoW priorities established based on stakeholder input, technical dependencies, and risk assessment. All MUST requirements are traceable to core system functionality.
