# STRIDE Threat Model: Clawdius High-Assurance Engineering Engine

**Document ID:** TM-CLAWDIUS-001  
**Version:** 1.0.0  
**Phase:** 3 (Security Engineering - Red Phase)  
**Status:** APPROVED  
**Created:** 2026-03-01  
**Classification:** STRIDE Threat Model

---

## 1. Executive Summary

This document presents a comprehensive STRIDE threat model for Clawdius, analyzing all components across the six threat categories: Spoofing, Tampering, Repudiation, Information Disclosure, Denial of Service, and Elevation of Privilege.

### 1.1 Threat Summary

| Category | Critical | High | Medium | Low | Total |
|----------|----------|------|--------|-----|-------|
| Spoofing | 1 | 2 | 1 | 0 | 4 |
| Tampering | 2 | 3 | 2 | 0 | 7 |
| Repudiation | 0 | 1 | 2 | 0 | 3 |
| Information Disclosure | 2 | 3 | 2 | 0 | 7 |
| Denial of Service | 0 | 2 | 3 | 1 | 6 |
| Elevation of Privilege | 3 | 2 | 1 | 0 | 6 |
| **Total** | **8** | **13** | **11** | **1** | **33** |

### 1.2 Critical Threats Requiring Immediate Mitigation

| Threat ID | Component | Description |
|-----------|-----------|-------------|
| THREAT-001 | Sentinel | Sandbox escape via bubblewrap misconfiguration |
| THREAT-002 | Sentinel | API key exfiltration via compromised dependency |
| THREAT-003 | Brain | Brain-Leaking attack via malicious LLM response |
| THREAT-004 | Sentinel | settings.toml RCE via command injection |
| THREAT-005 | Host Kernel | Privilege escalation via HAL exploitation |
| THREAT-006 | Graph-RAG | SQL injection via AST query |
| THREAT-007 | HFT Broker | Wallet Guard bypass via race condition |
| THREAT-008 | Host Kernel | MCP tool abuse for unauthorized access |

---

## 2. Component: Host Kernel (COMP-HOST-001)

### 2.1 Trust Boundary

```
┌─────────────────────────────────────────────────────────┐
│                    TRUST BOUNDARY                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
│  │   Host      │  │  Sentinel   │  │   Brain     │     │
│  │   Kernel    │  │  Sandbox    │  │   WASM      │     │
│  │  (Trusted)  │  │ (Semi-Trust)│  │ (Untrusted) │     │
│  └─────────────┘  └─────────────┘  └─────────────┘     │
└─────────────────────────────────────────────────────────┘
                          │
                    ┌─────▼─────┐
                    │  External │
                    │  World    │
                    │(Untrusted)│
                    └───────────┘
```

### 2.2 STRIDE Analysis

| Threat ID | Category | Description | Likelihood | Impact | Risk | Mitigation |
|-----------|----------|-------------|------------|--------|------|------------|
| THREAT-005 | E | HAL exploitation for privilege escalation | Medium | Critical | **Critical** | ADR-HOST-003: HAL trait isolation, platform-specific validation |
| THREAT-008 | E | MCP tool abuse for unauthorized filesystem access | Medium | Critical | **Critical** | Capability-based MCP tool whitelist, sandbox all MCP calls |
| THREAT-009 | T | monoio runtime manipulation | Low | High | Medium | Runtime integrity checks, signed configuration |
| THREAT-010 | I | Component state leakage via logs | Medium | High | High | Structured logging with PII redaction (secrecy crate) |
| THREAT-011 | D | Resource exhaustion via component spawn loop | Medium | Medium | Medium | Rate limiting per component type, max instances cap |
| THREAT-012 | R | Missing audit trail for state transitions | Low | Medium | Medium | REQ-1.3: Cryptographic hash logging to CHANGELOG.md |
| THREAT-013 | S | Component identity spoofing | Low | High | Medium | Component ID signing, runtime verification |

---

## 3. Component: Nexus FSM (COMP-NEXUS-001)

### 3.1 Trust Boundary

```
┌─────────────────────────────────────────────────────────┐
│                  FSM Trust Boundary                      │
│                                                          │
│   Phase N ──────► Phase N+1 ──────► Phase N+2           │
│     │               │                  │                │
│     ▼               ▼                  ▼                │
│  Quality        Quality            Quality              │
│  Gates          Gates              Gates                │
│                                                          │
│  Typestate: Compile-time enforcement                    │
└─────────────────────────────────────────────────────────┘
```

### 3.2 STRIDE Analysis

| Threat ID | Category | Description | Likelihood | Impact | Risk | Mitigation |
|-----------|----------|-------------|------------|--------|------|------------|
| THREAT-014 | T | Artifact tampering during phase transition | Low | High | Medium | Cryptographic hash verification, ADR immutability |
| THREAT-015 | R | Phase transition without audit record | Low | Medium | Low | REQ-1.3: Mandatory CHANGELOG.md entries |
| THREAT-016 | D | Quality gate infinite loop | Low | Medium | Low | Timeout on gate checks, max retry limit |
| THREAT-017 | T | Typestate bypass via unsafe code | Very Low | Critical | High | clippy::unwrap_used denied, cargo-vet for unsafe |

---

## 4. Component: Sentinel Sandbox (COMP-SENTINEL-001)

### 4.1 Trust Boundary

```
┌──────────────────────────────────────────────────────────────┐
│                    Sentinel Trust Boundary                    │
│                                                               │
│  ┌─────────────┐   ┌─────────────┐   ┌─────────────┐        │
│  │   Tier 1    │   │   Tier 2    │   │   Tier 3    │        │
│  │   Native    │   │  Container  │   │    WASM     │        │
│  │ (Audited)   │   │ (Semi-Trust)│   │ (Untrusted) │        │
│  └─────────────┘   └─────────────┘   └─────────────┘        │
│         │                 │                 │                │
│         └─────────────────┼─────────────────┘                │
│                           ▼                                  │
│              ┌─────────────────────────┐                    │
│              │    Capability Manager   │                    │
│              │   (Permission Tokens)   │                    │
│              └─────────────────────────┘                    │
└──────────────────────────────────────────────────────────────┘
```

### 4.2 STRIDE Analysis

| Threat ID | Category | Description | Likelihood | Impact | Risk | Mitigation |
|-----------|----------|-------------|------------|--------|------|------------|
| THREAT-001 | E | Sandbox escape via bubblewrap misconfiguration | Medium | Critical | **Critical** | Hardcoded capability sets, no runtime config, audit bubblewrap args |
| THREAT-002 | I | API key exfiltration via compromised dependency | Medium | Critical | **Critical** | REQ-3.3: Host proxy only, secrets never in sandbox, cargo-vet |
| THREAT-004 | E | settings.toml RCE via command injection | Medium | Critical | **Critical** | REQ-3.4: Strict TOML validation, command whitelist, no shell metachars |
| THREAT-018 | E | Capability token forgery | Low | Critical | High | HMAC-SHA256 signing, 32-byte signatures (P-SENT-001) |
| THREAT-019 | T | Capability derivation privilege escalation | Low | High | Medium | Derivation only attenuates (P-SENT-002), monotonic decrease |
| THREAT-020 | D | Sandbox resource exhaustion | Medium | High | High | Per-tier resource limits (memory, CPU, time) |
| THREAT-021 | S | Toolchain spoofing for tier selection | Low | High | Medium | Trust level verification, audited toolchain registry |
| THREAT-022 | I | Secret leakage via environment variables | Medium | High | High | REQ-3.3: Forbidden env patterns (*_KEY, *_SECRET, *_TOKEN) |
| THREAT-023 | E | Container breakout via mount escape | Low | Critical | High | Path sanitization, no mounts outside project root |
| THREAT-024 | D | Fork bomb via EXEC_SPAWN capability | Medium | High | High | Max spawn limit per capability, rate limiting |

---

## 5. Component: Brain WASM (COMP-BRAIN-001)

### 5.1 Trust Boundary

```
┌──────────────────────────────────────────────────────────────┐
│                     Brain Trust Boundary                      │
│                                                               │
│   Host Kernel ◄──────────────► Brain WASM (wasmtime)         │
│       │                              │                        │
│       │  Host Functions              │  WASM Exports          │
│       │  (capability-checked)        │  (sandboxed)           │
│       ▼                              ▼                        │
│   ┌──────────────────────────────────────────┐               │
│   │           LLM Provider APIs               │               │
│   │   OpenAI │ Anthropic │ DeepSeek │ Ollama │               │
│   └──────────────────────────────────────────┘               │
│                                                               │
│   Secrets: NEVER in WASM memory (Theorem 3)                  │
└──────────────────────────────────────────────────────────────┘
```

### 5.2 STRIDE Analysis

| Threat ID | Category | Description | Likelihood | Impact | Risk | Mitigation |
|-----------|----------|-------------|------------|--------|------|------------|
| THREAT-003 | E | Brain-Leaking: LLM response escalates privileges | Medium | Critical | **Critical** | REQ-3.2: WASM sandbox, no direct host access, RPC boundary |
| THREAT-025 | S | LLM response spoofing (hallucination) | High | High | High | SOP validation (REQ-4.1), response verification |
| THREAT-026 | I | Prompt injection reveals internal state | Medium | High | High | Input sanitization, prompt boundary enforcement |
| THREAT-027 | T | Malicious code generation | High | High | High | SOP enforcement, Blue Paper validation, user review |
| THREAT-028 | D | Infinite loop in WASM module | Medium | Medium | Medium | Fuel consumption (1B units), 30s timeout per invoke |
| THREAT-029 | I | API key leakage via LLM prompt | Low | Critical | High | REQ-3.3: Host proxy, keys never in WASM (P-BRAIN-004) |
| THREAT-030 | R | Untraceable LLM-generated code | Low | Medium | Low | ADR generation for all code changes, provenance tracking |
| THREAT-031 | E | WASM escape via wasmtime vulnerability | Very Low | Critical | High | wasmtime security updates, sandbox audit, capability check |

---

## 6. Component: Graph-RAG (COMP-GRAPH-001)

### 6.1 Trust Boundary

```
┌──────────────────────────────────────────────────────────────┐
│                   Graph-RAG Trust Boundary                    │
│                                                               │
│  ┌──────────────────┐  ┌──────────────────┐                  │
│  │  SQLite (AST)    │  │  LanceDB (Vector)│                  │
│  │  Structured Data │  │  Embeddings      │                  │
│  └──────────────────┘  └──────────────────┘                  │
│           │                    │                              │
│           └──────────┬─────────┘                              │
│                      ▼                                        │
│              ┌──────────────┐                                 │
│              │  MCP Tools   │                                 │
│              │ (Sandboxed)  │                                 │
│              └──────────────┘                                 │
└──────────────────────────────────────────────────────────────┘
```

### 6.2 STRIDE Analysis

| Threat ID | Category | Description | Likelihood | Impact | Risk | Mitigation |
|-----------|----------|-------------|------------|--------|------|------------|
| THREAT-006 | E | SQL injection via AST query | Low | Critical | **Critical** | Parameterized queries, no raw SQL, query builder |
| THREAT-032 | I | Vector embedding data leakage | Low | Medium | Low | Access control on LanceDB, encryption at rest |
| THREAT-033 | T | AST index poisoning | Medium | High | High | Hash verification, incremental update validation |
| THREAT-034 | D | Query-induced DoS via complex joins | Medium | Medium | Medium | Query timeout, result size limits |
| THREAT-035 | S | MCP tool identity spoofing | Low | High | Medium | Tool signature verification, registry |
| THREAT-036 | I | Sensitive code exposure via search | Medium | High | High | Query filtering, result redaction |

---

## 7. Component: HFT Broker (COMP-BROKER-001)

### 7.1 Trust Boundary

```
┌──────────────────────────────────────────────────────────────┐
│                    Broker Trust Boundary                      │
│                                                               │
│  Market Data ◄──────► SPSC Ring Buffer ◄──────► Signal Gen  │
│     (AF_XDP)              (HugePage)              (WASM)     │
│                               │                              │
│                               ▼                              │
│                       ┌──────────────┐                       │
│                       │ Wallet Guard │                       │
│                       │ (Hard Lock)  │                       │
│                       └──────────────┘                       │
│                               │                              │
│                               ▼                              │
│                       ┌──────────────┐                       │
│                       │Notification  │                       │
│                       │ Gateway      │                       │
│                       └──────────────┘                       │
└──────────────────────────────────────────────────────────────┘
```

### 7.2 STRIDE Analysis

| Threat ID | Category | Description | Likelihood | Impact | Risk | Mitigation |
|-----------|----------|-------------|------------|--------|------|------------|
| THREAT-007 | E | Wallet Guard bypass via race condition | Low | Critical | **Critical** | Lock-free atomic operations, CachePadded, Acquire/Release |
| THREAT-037 | T | Market data manipulation | Low | Critical | High | Checksum validation, sequence number verification |
| THREAT-038 | I | Trade signal interception | Medium | High | High | TLS for notifications, encrypted IPC |
| THREAT-039 | D | Ring buffer overflow causing signal loss | Low | High | Medium | 1GB HugePage, backpressure signaling |
| THREAT-040 | R | Missing trade audit trail | Low | Critical | High | MiFID II timestamping, immutable log |
| THREAT-041 | T | Risk parameter tampering | Low | Critical | High | Signed configuration, runtime integrity check |
| THREAT-042 | S | Fake market data injection | Medium | High | High | Source authentication, signature verification |

---

## 8. Cross-Cutting Threats

### 8.1 Supply Chain Threats

| Threat ID | Category | Description | Likelihood | Impact | Risk | Mitigation |
|-----------|----------|-------------|------------|--------|------|------------|
| THREAT-043 | T | Compromised crate in dependencies | Medium | Critical | **Critical** | cargo-vet, cargo-deny, cryptographic auditing |
| THREAT-044 | I | Malicious code in transitive dependencies | Medium | Critical | **Critical** | Dependency pinning, SBOM, vulnerability scanning |
| THREAT-045 | T | Build system compromise | Low | Critical | High | Reproducible builds, signed artifacts |
| THREAT-046 | S | Fake package typosquatting | Low | High | Medium | Dependency verification, lockfile integrity |

### 8.2 Network Threats

| Threat ID | Category | Description | Likelihood | Impact | Risk | Mitigation |
|-----------|----------|-------------|------------|--------|------|------------|
| THREAT-047 | I | API key interception in transit | Low | Critical | High | TLS 1.3 for all external API calls |
| THREAT-048 | S | LLM provider impersonation | Low | Critical | High | Certificate pinning, provider authentication |
| THREAT-049 | D | API rate limiting causing service degradation | Medium | Medium | Medium | Retry with backoff, multiple provider fallback |

### 8.3 Physical/Environmental Threats

| Threat ID | Category | Description | Likelihood | Impact | Risk | Mitigation |
|-----------|----------|-------------|------------|--------|------|------------|
| THREAT-050 | I | Memory dump analysis for secrets | Low | High | Medium | Memory zeroing (secrecy crate), mlock protection |
| THREAT-051 | D | Hardware failure during critical operation | Low | High | Medium | State persistence, crash recovery |

---

## 9. Threat Mitigation Summary

### 9.1 Mitigation Coverage

| Risk Level | Count | Mitigated | Partial | Unmitigated |
|------------|-------|-----------|---------|-------------|
| Critical | 8 | 8 | 0 | 0 |
| High | 13 | 12 | 1 | 0 |
| Medium | 11 | 10 | 1 | 0 |
| Low | 1 | 1 | 0 | 0 |
| **Total** | **33** | **31** | **2** | **0** |

### 9.2 Partial Mitigations

| Threat ID | Description | Gap | Action Required |
|-----------|-------------|-----|-----------------|
| THREAT-027 | Malicious code generation | SOP cannot detect all attacks | Human review required for all generated code |
| THREAT-033 | AST index poisoning | No real-time detection | Implement hash verification on read |

---

## 10. Security Requirements Traceability

| Requirement | Threats Addressed |
|-------------|-------------------|
| REQ-3.1 (JIT Sandboxing) | THREAT-001, THREAT-020, THREAT-024 |
| REQ-3.2 (Brain Isolation) | THREAT-003, THREAT-031 |
| REQ-3.3 (Secret Redaction) | THREAT-002, THREAT-022, THREAT-029 |
| REQ-3.4 (Anti-RCE Validation) | THREAT-004 |
| REQ-1.3 (Atomic Commit Ledger) | THREAT-012, THREAT-015, THREAT-040 |
| REQ-4.1 (SOP Enforcement) | THREAT-025, THREAT-027 |

---

## 11. Threat Model Validation

### 11.1 Review Schedule

| Event | Frequency | Owner |
|-------|-----------|-------|
| Full threat model review | Quarterly | Security Engineer |
| Critical threat reassessment | Monthly | Security Engineer |
| New component analysis | Per release | Development Team |
| Incident-driven review | Ad-hoc | Security Team |

### 11.2 Assumptions

1. **Platform Security:** Underlying OS (Linux/macOS/Windows) is not compromised
2. **Cryptographic Primitives:** HMAC-SHA256, TLS 1.3 are cryptographically secure
3. **Wasmtime Security:** wasmtime runtime correctly enforces WASM sandbox
4. **Keyring Security:** Platform keyring (libsecret/Keychain) is not compromised
5. **User Trust:** User running Clawdius is not malicious

### 11.3 Out of Scope

1. Physical access attacks to the host machine
2. Social engineering attacks against developers
3. Nation-state level adversaries with zero-day exploits
4. Compromise of LLM provider infrastructure (OpenAI, Anthropic, etc.)

---

**Document Status:** APPROVED  
**Next Review:** 2026-04-01  
**Sign-off:** Security Engineering Team
