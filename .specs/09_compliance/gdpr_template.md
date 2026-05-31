# GDPR Compliance Assessment Template

**Version:** 1.0.0
**Last Updated:** 2026-05-31
**Document Owner:** Clawdius Compliance Team
**Review Cycle:** Quarterly
**Regulatory Reference:** Regulation (EU) 2016/679 (General Data Protection Regulation)

---

## 1. Document Purpose

This template defines GDPR compliance requirements for the Clawdius monorepo. It maps GDPR articles to Clawdius components, defines data subject rights procedures, and specifies data processing records. Clawdius processes personal data in the following categories:

- **User identifiers:** tenant_id, user_id (stored in `UsageRecord`)
- **Session data:** conversation history, metadata, token usage (stored in `SessionStore`)
- **Telemetry:** crash reports, usage patterns (opt-in via `TelemetryConfig`)
- **Configuration:** LLM provider API keys, workspace settings

**Machine-readable controls:** `crates/clawdius-core/src/compliance.rs` (Framework::Gdpr, control ID `GDPR-ART32`)

---

## 2. Data Subject Rights

### 2.1 Right of Access (Art. 15)

| Requirement | Clawdius Implementation | Status |
|-------------|-----------------------|--------|
| Provide copy of personal data upon request | Session data exportable from `SessionStore::load_session()`. Usage data exportable from `TenantUsageTracker::get_usage()`. Full telemetry records exportable from `TelemetryLayer`. | IN_PROGRESS |
| Structured, commonly used format | JSON export supported via `ComplianceGenerator::report_to_json()`. Portable format (JSON-LD): PLANNED. | IN_PROGRESS |
| Provide within 30 days | Manual process. Automated DSAR pipeline: PLANNED. | PLANNED |

**Component mapping:**
- `crates/clawdius-core/src/session/store.rs` -- session data retrieval
- `crates/clawdius-core/src/usage.rs:301-307` -- `TenantUsageTracker::get_usage()` returns (tokens, cost_cents, calls)
- `crates/clawdius-core/src/compliance.rs:462-466` -- `report_to_json()` for structured export

### 2.2 Right to Erasure (Art. 17)

| Requirement | Clawdius Implementation | Status |
|-------------|-----------------------|--------|
| Delete personal data upon request | Tenant deletion via `crates/clawdius-gateway/src/admin.rs:268-276`. Cancels subscription and marks for deletion. | COMPLIANT |
| Delete session history | `SessionStore` supports session deletion. Batch deletion for a user's all sessions: PLANNED. | IN_PROGRESS |
| Delete audit logs | Audit log retention with configurable `retention_days`. Automated deletion after retention period. Immediate deletion on request: PLANNED. | IN_PROGRESS |
| Propagate to LLM providers | No mechanism to recall prompts from LLM providers. Caveat: prompts are transient, not stored by Clawdius. On-prem LLM recommended for GDPR compliance. | PLANNED |
| Propagate to backups | Checkpoint system supports restore but checkpoint deletion: PLANNED. | PLANNED |

**Component mapping:**
- `crates/clawdius-gateway/src/admin.rs:268-276` -- `delete_tenant` handler
- `crates/clawdius-core/src/config.rs:877` -- `AuditConfig::retention_days` (default 90)
- `crates/clawdius-core/src/checkpoint.rs` -- checkpoint management

### 2.3 Right to Data Portability (Art. 20)

| Requirement | Clawdius Implementation | Status |
|-------------|-----------------------|--------|
| Receive data in structured, machine-readable format | JSON export via `serde_json::to_string_pretty()` for sessions, usage, compliance reports | COMPLIANT |
| Transmit directly to another controller | API endpoint for data export: PLANNED. Manual JSON export: available. | IN_PROGRESS |
| Standard interoperable format | JSON. CSV export: PLANNED. | IN_PROGRESS |

**Component mapping:**
- `crates/clawdius-core/src/session/types.rs` -- `Session`, `Message`, `SessionMeta` all derive `Serialize`
- `crates/clawdius-core/src/usage.rs:14-41` -- `UsageRecord` derives `Serialize`
- `crates/clawdius-core/src/compliance.rs:462` -- JSON export

### 2.4 Right to Rectification (Art. 16)

| Requirement | Clawdius Implementation | Status |
|-------------|-----------------------|--------|
| Correct inaccurate personal data | `SessionManager` supports session updates. Tenant metadata update via admin API. | COMPLIANT |
| Update user identifiers | `user_id` field in `UsageRecord` is mutable via `with_user()` builder | COMPLIANT |

### 2.5 Right to Restrict Processing (Art. 18)

| Requirement | Clawdius Implementation | Status |
|-------------|-----------------------|--------|
| Suspend processing upon request | Air-gapped mode (`AirGapEnforcer::enable()`) halts all external processing | COMPLIANT |
| Block telemetry | `AirGapConfig::block_telemetry` enforced at runtime via `AirGapEnforcer::check_telemetry()` | COMPLIANT |
| Block crash reporting | `AirGapConfig::block_crash_reports` enforced at runtime | COMPLIANT |

**Component mapping:**
- `crates/clawdius-core/src/airgap.rs:109-203` -- `AirGapEnforcer` with `enable()`, `disable()`, `check_telemetry()`, `check_crash_report()`

### 2.6 Right to Object (Art. 21)

| Requirement | Clawdius Implementation | Status |
|-------------|-----------------------|--------|
| Object to profiling based on automated processing | Telemetry is opt-in. User can disable at any time via `TelemetryConfig` or air-gap mode. | COMPLIANT |
| Object to processing for direct marketing | Clawdius does not perform marketing profiling. | NOT_APPLICABLE |

---

## 3. Data Processing Records (Art. 30)

### 3.1 Record of Processing Activities (ROPA)

| Field | Value |
|-------|-------|
| **Data Controller** | Clawdius Inc. (or deploying organization for self-hosted) |
| **Data Protection Officer** | To be designated per organizational requirement |
| **Purposes of Processing** | (1) Provide AI coding assistance (2) Usage metering and billing (3) Telemetry and crash reporting (4) Session persistence |
| **Categories of Data Subjects** | Developers, administrators, enterprise tenants |
| **Categories of Personal Data** | User IDs, tenant IDs, session metadata, token usage, IP addresses (from HTTP), platform identifiers |
| **Categories of Recipients** | LLM providers (external), internal audit systems |
| **International Transfers** | LLM providers (OpenAI, Anthropic, OpenRouter, Zai) may process in US/EU. Standard Contractual Clauses: PLANNED. |
| **Retention Periods** | Sessions: configurable. Audit logs: 90 days default (`AuditConfig::retention_days`). Telemetry: until opt-out. |
| **Technical/Organizational Measures** | AES-256-GCM encryption, rustls-tls, air-gap mode, access controls, Lean4 formal verification |

### 3.2 Lawful Basis for Processing

| Processing Activity | Lawful Basis | Justification |
|--------------------|-------------|---------------|
| AI coding assistance | Contract performance | Service delivery requires session and code context processing |
| Usage metering and billing | Legitimate interest | Necessary for subscription management and billing accuracy |
| Telemetry and crash reporting | Consent | Opt-in via `TelemetryConfig`; can be disabled at any time |
| Session persistence | Contract performance | Required for service continuity and context management |

---

## 4. Cookie and Consent Requirements (Art. 5(3), Art. 6(1)(a))

### 4.1 Current State

Clawdius is primarily a CLI/TUI application and gateway service. It does not currently serve web pages or set browser cookies. The following consent mechanisms exist:

| Data Collection | Consent Mechanism | Status |
|----------------|------------------|--------|
| Telemetry | Opt-in flag in `TelemetryConfig`. Default: disabled for new installations. | COMPLIANT |
| Crash reporting | Controlled by `AirGapConfig::block_crash_reports`. Default: blocked in air-gap mode. | COMPLIANT |
| LLM prompt transmission | Implied consent through active usage. Explicit disclosure: PLANNED. | IN_PROGRESS |
| Usage data collection | Implied through service subscription. Privacy notice: PLANNED. | IN_PROGRESS |

### 4.2 Gateway HTTP Considerations

The `crates/clawdius-gateway` serves REST APIs via axum. If web UI or browser-based access is added:

- Cookie consent banner required before setting non-essential cookies
- Session cookies for authentication must be marked `Secure`, `HttpOnly`, `SameSite=Strict`
- No third-party tracking cookies without explicit consent

### 4.3 Consent Withdrawal

Users can withdraw consent at any time:
1. **Telemetry:** Set `telemetry.enabled = false` in configuration or enable air-gap mode
2. **All external processing:** Enable `AirGapConfig::strict()` which blocks telemetry, crash reports, and auto-updates
3. **Data deletion:** Submit erasure request (Art. 17 procedure above)

---

## 5. Mapping to Clawdius Modules

### 5.1 Telemetry and Privacy Module

| GDPR Article | Module | Implementation |
|-------------|--------|---------------|
| Art. 5(1)(c) Data minimisation | `crates/clawdius-core/src/telemetry/structured.rs` | Structured telemetry with only necessary fields: session_id, event_type, timestamp |
| Art. 5(1)(e) Storage limitation | `crates/clawdius-core/src/config.rs:877` | Configurable retention: `AuditConfig::retention_days`, default 90 days |
| Art. 7 Consent | `crates/clawdius-core/src/telemetry/mod.rs` | Opt-in telemetry via `TelemetryConfig` |
| Art. 17 Right to erasure | `crates/clawdius-core/src/airgap.rs` | Full processing halt via `AirGapConfig::strict()` |
| Art. 21 Right to object | `crates/clawdius-core/src/airgap.rs:149-159` | `AirGapEnforcer::check_telemetry()` blocks on opt-out |

### 5.2 Storage and Session Module

| GDPR Article | Module | Implementation |
|-------------|--------|---------------|
| Art. 15 Right of access | `crates/clawdius-core/src/session/store.rs` | `SessionStore::load_session()`, `list_sessions()` for data retrieval |
| Art. 20 Data portability | `crates/clawdius-core/src/session/types.rs` | `Serialize` derives on all data types; JSON export |
| Art. 25 Data protection by design | `crates/clawdius-core/src/encryption.rs` | AES-256-GCM encryption for sensitive session data |
| Art. 32 Security of processing | `crates/clawdius-core/src/compliance.rs:329-346` | `GDPR-ART32` control with `Implemented` status |

### 5.3 Configuration Module

| GDPR Article | Module | Implementation |
|-------------|--------|---------------|
| Art. 5(2) Accountability | `crates/clawdius-core/src/config.rs:862-889` | `AuditConfig` provides audit trail configuration with persistence |
| Art. 30 Records of processing | `crates/clawdius-core/src/config.rs:644` | `AuditConfig` as part of `MessagingConfig` in `Config` struct |

### 5.4 Compliance Generator Module

| GDPR Article | Module | Implementation |
|-------------|--------|---------------|
| Art. 24 Responsibility | `crates/clawdius-core/src/compliance.rs:154-476` | `ComplianceGenerator` produces machine-readable compliance reports for any supported framework including GDPR |
| Art. 28 Processor requirements | `crates/clawdius-core/src/compliance.rs:76-115` | `EvidenceRef` tracks evidence chain for processor accountability |

---

## 6. Gap Analysis

| Priority | Gap | Affected Articles | Effort Estimate | Status |
|----------|-----|------------------|-----------------|--------|
| High | Automated DSAR pipeline (access, erasure, portability) | Art. 15, 17, 20 | 7-10 days | PLANNED |
| High | Privacy policy publication | Art. 13, 14 | 2-3 days | PLANNED |
| High | Standard Contractual Clauses for LLM providers | Art. 46(2)(c) | 5-7 days | PLANNED |
| Medium | Formal DPO appointment | Art. 37 | 1 day | PLANNED |
| Medium | Data Protection Impact Assessment (DPIA) for AI features | Art. 35 | 5-10 days | PLANNED |
| Medium | Session history batch deletion by user | Art. 17 | 3-5 days | PLANNED |
| Medium | CSV/machine-readable export format for portability | Art. 20 | 2-3 days | PLANNED |
| Low | Cookie consent banner (if web UI added) | Art. 5(3) | 1-2 days | PLANNED |
| Low | Automated GDPR compliance score generation | Art. 24 | 3-5 days | PLANNED |

---

## 7. Compliance Score Calculation

The `ComplianceGenerator` in `crates/clawdius-core/src/compliance.rs` computes GDPR compliance as follows:

```rust
// From crates/clawdius-core/src/compliance.rs:433-435
let score = if total > 0 {
    (implemented as f64 + partial as f64 * 0.5) / total as f64
} else {
    0.0
};
```

| Control ID | Name | Status | Score Contribution |
|------------|------|--------|-------------------|
| GDPR-ART32 | Security of Processing | COMPLIANT | 1.0 |

**Note:** Additional GDPR controls should be registered in `ComplianceGenerator::load_default_controls()` to expand coverage. Currently one GDPR control is defined. Target: 15-20 controls covering all articles referenced above.
