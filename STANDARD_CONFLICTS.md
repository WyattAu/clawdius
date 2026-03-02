# Clawdius Standard Conflicts Register

**Document ID:** SC-CLAWDIUS-001  
**Version:** 1.0.0  
**Phase:** 0 (Requirements Engineering)  
**Created:** 2026-03-01  
**Status:** ACTIVE  

---

## 1. Purpose

This document tracks conflicts between applicable standards, specifications, and requirements. Each conflict is documented with its resolution status and rationale.

---

## 2. Active Conflicts

### CONF-001: Async Runtime Selection (RESOLVED)

| Attribute | Value |
|-----------|-------|
| **Conflict ID** | CONF-001 |
| **Severity** | Critical |
| **Status** | ✅ RESOLVED |
| **Discovery Date** | 2026-03-01 |
| **Resolution Date** | 2026-03-01 |

#### Conflicting Standards

| Source | Document | Position |
|--------|----------|----------|
| basic_spec.md | §1.1 | Specifies `tokio` runtime |
| rust_sop.md | §2.1 | Recommends `monoio` or `glommio` for >100k connections |
| User Directive | Phase -0.5 | Mandates `monoio` |

#### Conflict Description

The technical specification (`basic_spec.md`) specified `tokio` as the async runtime, while the Rust SOP (`rust_sop.md`) recommends `monoio` or `glommio` for high-performance scenarios. For HFT applications (Broker mode), `monoio` provides thread-per-core architecture with `io_uring` support, eliminating thread-stealing jitter.

#### Resolution

**Selected:** `monoio`

**Rationale:**
1. User directive explicitly mandates monoio
2. Thread-per-core architecture eliminates jitter for HFT
3. `io_uring` provides superior I/O performance
4. Zero-GC requirement aligns with monoio design

**Impact:**
- All async code must use monoio APIs
- `tokio::spawn` replaced with `monoio::spawn`
- Timer and I/O APIs use monoio equivalents
- Some ecosystem crates may require compatibility shims

**Affected Requirements:**
- REQ-5.2 (High-Frequency Ingestion)
- REQ-6.3 (Resource Efficiency)

---

### CONF-002: Performance vs Ergonomics (MONITORING)

| Attribute | Value |
|-----------|-------|
| **Conflict ID** | CONF-002 |
| **Severity** | Medium |
| **Status** | 🔄 MONITORING |
| **Discovery Date** | 2026-03-01 |

#### Conflicting Standards

| Source | Document | Position |
|--------|----------|----------|
| HFT Requirements | DA-CLAWDIUS-001 §3.3 | Zero GC pauses required |
| Enterprise Requirements | DA-CLAWDIUS-001 §4 | Developer ergonomics valued |

#### Conflict Description

HFT mode requires zero-allocation hot paths and no GC pauses, while enterprise mode benefits from ergonomic error handling (e.g., `error-stack`) which allocates.

#### Resolution Strategy

**Approach:** Configurable profiles

| Profile | Error Handling | Allocator | Runtime Tuning |
|---------|---------------|-----------|----------------|
| HFT | Flat `#[repr(u8)]` enums | mimalloc | Thread-pinned |
| Enterprise | `error-stack` | snmalloc | Standard pool |

**Implementation:**
```rust
#[cfg(feature = "hft")]
type Error = HftError;  // Zero-allocation

#[cfg(not(feature = "hft"))]
type Error = error_stack::Report<EnterpriseError>;
```

**Affected Requirements:**
- REQ-5.2 (High-Frequency Ingestion)
- REQ-5.3 (Wallet Guard)

---

### CONF-003: Zero-Copy vs Debuggability (MONITORING)

| Attribute | Value |
|-----------|-------|
| **Conflict ID** | CONF-003 |
| **Severity** | Low |
| **Status** | 🔄 MONITORING |
| **Discovery Date** | 2026-03-01 |

#### Conflicting Standards

| Source | Document | Position |
|--------|----------|----------|
| HFT Requirements | rust_sop.md §3.3 | Zero-copy parsing mandatory |
| Debugging Needs | General | Descriptive error messages valuable |

#### Conflict Description

Zero-copy parsing (via `bytemuck`) provides maximum performance but produces less descriptive error messages compared to validated parsing.

#### Resolution Strategy

**Approach:** Conditional compilation

| Build Mode | Parsing | Error Messages |
|------------|---------|----------------|
| Release (HFT) | Zero-copy | Minimal |
| Debug/Dev | Validated | Descriptive |

**Implementation:**
```rust
#[cfg(feature = "hft")]
fn parse_packet(data: &[u8]) -> Result<Packet, HftError> {
    bytemuck::pod_read_unaligned(data)
}

#[cfg(not(feature = "hft"))]
fn parse_packet(data: &[u8]) -> Result<Packet, RichError> {
    validated_parse(data)  // With context
}
```

**Affected Requirements:**
- REQ-5.2 (High-Frequency Ingestion)
- REQ-6.3 (Resource Efficiency)

---

## 3. Resolved Conflicts

| ID | Description | Resolution | Date |
|----|-------------|------------|------|
| CONF-001 | tokio vs monoio | monoio selected | 2026-03-01 |

---

## 4. Monitoring Conflicts

| ID | Description | Trigger for Resolution |
|----|-------------|----------------------|
| CONF-002 | Performance vs Ergonomics | Profile selection at runtime |
| CONF-003 | Zero-Copy vs Debuggability | Build mode selection |

---

## 5. Conflict Resolution Protocol

### 5.1 Resolution Priority

1. **Safety-critical** (IEC 61508, ISO 26262) → Highest
2. **Regulatory** (FIPS, NIST, MiFID II) → High
3. **Domain-specific** (HFT constraints) → Medium
4. **General** (ISO/IEC 12207) → Low

### 5.2 Resolution Authority

| Conflict Level | Authority |
|----------------|-----------|
| Critical | User / Principal Architect |
| High | Technical Lead |
| Medium | Senior Engineer |
| Low | Developer |

### 5.3 Documentation Requirements

All resolved conflicts must document:
1. Conflicting standards
2. Selected resolution
3. Rationale
4. Impact analysis
5. Affected requirements

---

## 6. Potential Future Conflicts

| ID | Description | Likelihood | Impact |
|----|-------------|------------|--------|
| CONF-FUT-001 | FIPS 140-2 crypto vs performance | Medium | High |
| CONF-FUT-002 | WASM sandbox vs native speed | High | Medium |
| CONF-FUT-003 | Cross-platform vs platform-specific optimizations | High | Medium |

---

## 7. Change Log

| Date | Conflict | Change |
|------|----------|--------|
| 2026-03-01 | CONF-001 | Resolved: monoio selected |
| 2026-03-01 | CONF-002 | Documented: profile-based resolution |
| 2026-03-01 | CONF-003 | Documented: conditional compilation |

---

**Approval:** All identified conflicts are documented with resolution strategies. CONF-001 is resolved; CONF-002 and CONF-003 are monitored with defined resolution triggers.
