# SOC 2 Type II Compliance Template

**Version:** 1.0.0
**Last Updated:** 2026-05-31
**Document Owner:** Clawdius Compliance Team
**Review Cycle:** Quarterly
**Framework Reference:** AICPA TSC 2017 (Trust Services Criteria)

---

## 1. Document Purpose

This template defines the SOC 2 Type II compliance controls, gap analysis, and audit procedures for the Clawdius monorepo. It maps Trust Service Criteria to implemented components and identifies remaining gaps. This document is consumable by the `ComplianceGenerator` in `crates/clawdius-core/src/compliance.rs`.

Related machine-readable artifacts: `.specs/compliance/soc2_readiness.toml`

---

## 2. Control Categories and Status

### 2.1 Security Controls (CC6-CC9)

| Control ID | Control Name | Requirement | Status | Clawdius Component | Evidence Location |
|------------|-------------|-------------|--------|-------------------|-------------------|
| CC6.1 | Logical and Physical Access Controls | Implement logical access security measures over information assets | COMPLIANT | API key auth middleware, AES-256-GCM encryption, air-gap mode | `crates/clawdius-gateway/src/admin.rs`, `crates/clawdius-core/src/encryption.rs`, `crates/clawdius-core/src/airgap.rs` |
| CC6.2 | System Account Management | Manage system accounts through lifecycle (create, modify, disable, delete) | COMPLIANT | Tenant CRUD, subscription lifecycle, plan changes | `crates/clawdius-gateway/src/admin.rs` |
| CC6.3 | Data Encryption | Encrypt data at rest and in transit | COMPLIANT | AES-256-GCM with HKDF-SHA256, rustls-tls transport | `crates/clawdius-core/src/encryption.rs`, `crates/clawdius-gateway/Cargo.toml` |
| CC6.4 | Authentication | Authenticate users before granting access | IN_PROGRESS | API key auth exists; JWT and RBAC planned | `crates/clawdius-gateway/src/admin.rs:36` |
| CC6.5 | Role-Based Access Control | Restrict access based on least-privilege roles | PLANNED | Single admin API key model; multi-role planned for Phase F | -- |
| CC7.1 | Detection and Monitoring | Detect and monitor intrusions and anomalies | COMPLIANT | Structured telemetry, audit logging, crash reporting | `crates/clawdius-core/src/telemetry/structured.rs`, `crates/clawdius-core/src/config.rs:862` |
| CC7.2 | Incident Response | Respond to identified incidents to mitigate impact | IN_PROGRESS | Error classification taxonomy (10 levels); formal runbook needed | `crates/clawdius-core/src/error.rs` |
| CC7.3 | Security Event Evaluation | Evaluate security events for potential incidents | PLANNED | Structured telemetry events exist; automated evaluation pipeline not implemented | `crates/clawdius-core/src/telemetry/structured.rs` |
| CC8.1 | Change Management | Authorize, design, develop, configure, test, and approve changes | COMPLIANT | Git-based change tracking, Lean4 formal proofs (114 theorems, 0 sorry), CI/CD | `proofs/`, `.github/` |
| CC8.2 | System Development Life Cycle | Maintain a documented SDLC with security checkpoints | COMPLIANT | Rust module structure, `deny.toml` dependency audit, `.cargo-audit.toml` | `deny.toml`, `.cargo-audit.toml` |
| CC9.1 | Risk Mitigation | Identify and mitigate risks that could affect objectives | IN_PROGRESS | Sandbox tiers (4 levels), circuit breaker pattern; formal risk register pending | `crates/clawdius-core/src/sandbox.rs`, `crates/clawdius-core/src/retry.rs` |
| CC9.2 | Threat Intelligence | Identify and respond to threats | PLANNED | No automated threat feed integration | -- |

### 2.2 Availability Controls (A1)

| Control ID | Control Name | Requirement | Status | Clawdius Component | Evidence Location |
|------------|-------------|-------------|--------|-------------------|-------------------|
| A1.1 | Backup and Recovery | Maintain and test backup procedures | COMPLIANT | 4 storage backends (SQLite, PostgreSQL, MariaDB, InMemory), checkpoint system | `crates/clawdius-core/src/storage/`, `crates/clawdius-core/src/checkpoint.rs` |
| A1.2 | Performance Monitoring | Monitor system performance and availability | COMPLIANT | Resource governor, usage metering, sliding-window rate limiter | `crates/clawdius-core/src/usage.rs`, `crates/clawdius-gateway/src/rate_limit.rs` |
| A1.3 | Disaster Recovery | Implement disaster recovery procedures | IN_PROGRESS | Session persistence via SQLite; DR runbook and cross-region replication pending | `crates/clawdius-core/src/session/store.rs` |
| A1.4 | SLA Tracking | Monitor and report against SLAs | PLANNED | `MetricsDashboard` exists; SLA threshold alerting not implemented | `crates/clawdius-core/src/telemetry/` |

### 2.3 Processing Integrity Controls (PI)

| Control ID | Control Name | Requirement | Status | Clawdius Component | Evidence Location |
|------------|-------------|-------------|--------|-------------------|-------------------|
| PI1.1 | Input Validation | Validate and sanitize all inputs | COMPLIANT | Serde deserialization with typed structs, config validation | `crates/clawdius-core/src/config.rs` |
| PI1.2 | Error Handling | Handle and log processing errors consistently | COMPLIANT | 10-level error classification, structured error propagation via `thiserror` | `crates/clawdius-core/src/error.rs` |
| PI1.3 | Data Integrity | Ensure data integrity during processing and storage | COMPLIANT | AES-256-GCM AEAD with AAD binding (tenant ID), SQLite ACID | `crates/clawdius-core/src/encryption.rs:85`, `crates/clawdius-core/src/session/store.rs` |

### 2.4 Confidentiality Controls (C1)

| Control ID | Control Name | Requirement | Status | Clawdius Component | Evidence Location |
|------------|-------------|-------------|--------|-------------------|-------------------|
| C1.1 | Confidentiality at Rest | Protect confidential information at rest via encryption | COMPLIANT | AES-256-GCM with HKDF-SHA256 key derivation, per-message salts | `crates/clawdius-core/src/encryption.rs` |
| C1.2 | Confidentiality in Transit | Protect confidential information in transit via TLS | COMPLIANT | rustls-tls on all HTTP clients, no plaintext HTTP | `crates/clawdius-gateway/Cargo.toml` |
| C1.3 | Key Management | Manage encryption keys through lifecycle | IN_PROGRESS | MasterKey wrapper with env/file loading; automated rotation and HSM integration planned | `crates/clawdius-core/src/encryption.rs:209` |
| C1.4 | Data Classification | Classify data by sensitivity level | PLANNED | No formal data classification schema | -- |

### 2.5 Privacy Controls (P1)

| Control ID | Control Name | Requirement | Status | Clawdius Component | Evidence Location |
|------------|-------------|-------------|--------|-------------------|-------------------|
| P1.1 | Privacy Notice | Provide notice of privacy practices | PLANNED | Privacy policy document not yet published | -- |
| P1.2 | Consent and Choice | Obtain consent and provide choice for data collection | COMPLIANT | Opt-in telemetry, air-gap mode disables all external data | `crates/clawdius-core/src/telemetry/mod.rs`, `crates/clawdius-core/src/airgap.rs` |
| P1.3 | Data Retention | Retain data only as long as necessary | IN_PROGRESS | Configurable audit retention (`AuditConfig::retention_days`, default 90); automated deletion pending | `crates/clawdius-core/src/config.rs:877` |
| P1.4 | Data Access Request Handling | Handle data subject access requests | PLANNED | No formal DSAR process | -- |

---

## 3. Gap Analysis Summary

| Priority | Gap | Affected Controls | Effort Estimate | Target Phase |
|----------|-----|------------------|-----------------|--------------|
| High | Role-based access control (RBAC) | CC6.5, C1.4 | 5-7 days | Phase F |
| High | Formal incident response runbook | CC7.2, CC7.3 | 2-3 days | Phase E |
| Medium | Key rotation and HSM integration | C1.3 | 5-10 days | Phase F |
| Medium | Automated threat intelligence feed | CC9.2 | 7-14 days | Phase G |
| Medium | Disaster recovery runbook and cross-region replication | A1.3 | 5-10 days | Phase F |
| Low | Privacy policy publication | P1.1 | 1 day | Phase E |
| Low | Formal data classification schema | C1.4, P1.4 | 2-3 days | Phase F |
| Low | SLA threshold alerting | A1.4 | 3-5 days | Phase F |

---

## 4. Audit Procedure Outline

### 4.1 Pre-Audit Preparation

1. Run `ComplianceGenerator::generate_report(Framework::Soc2, ...)` to produce machine-readable evidence
2. Export audit logs from SQLite backend (path per `AuditConfig::sqlite_path`)
3. Compile code coverage reports for encryption and compliance modules
4. Gather Lean4 proof artifacts from `proofs/` directory

### 4.2 Evidence Collection by Control

| Control | Evidence Type | Collection Method |
|---------|--------------|-------------------|
| CC6.1-CC6.3 | Code review, test results | `cargo test -p clawdius-core -- encryption compliance`, `cargo test -p clawdius-gateway -- admin` |
| CC7.1 | Audit log export | Export from `audit.db` SQLite for review period |
| CC8.1 | Formal proofs | Lean4 theorem dump from `proofs/` |
| A1.1 | Backup test results | Restore test from checkpoint system |
| C1.1 | Encryption test results | Property-based tests in `crates/clawdius-core/src/encryption.rs` |

### 4.3 Auditor Walkthrough Procedures

1. **Access Control (CC6):** Demonstrate tenant creation/deletion flow in admin API. Show API key enforcement on `/api/admin/*` endpoints.
2. **Encryption (CC6.3, C1):** Walk through `encrypt()` -> `decrypt()` roundtrip. Demonstrate AAD binding with tenant ID. Show key derivation with HKDF-SHA256.
3. **Monitoring (CC7.1):** Show structured telemetry event structure from `TelemetryEvent`. Demonstrate audit log persistence to SQLite.
4. **Change Management (CC8.1):** Present Lean4 proof suite (114 theorems, 0 sorry). Walk through CI pipeline.
5. **Sandbox (CC9.1):** Demonstrate 4-tier sandbox isolation. Show tier selection logic in `SandboxTier` enum.
6. **Availability (A1):** Demonstrate session persistence with `SessionStore`. Show checkpoint restore.

---

## 5. Mapping to Clawdius Components

```
crates/clawdius-core/
  src/encryption.rs        -> CC6.3, C1.1, C1.3 (AES-256-GCM, HKDF, MasterKey)
  src/compliance.rs        -> CC7.1, CC8.1 (ComplianceGenerator, evidence artifacts)
  src/airgap.rs             -> CC6.1, P1.2 (AirGapEnforcer, telemetry blocking)
  src/session.rs            -> A1.1, A1.3 (SessionStore, SessionManager)
  src/session/store.rs     -> A1.1 (SQLite persistence)
  src/sandbox.rs            -> CC9.1 (4-tier isolation: TrustedAudited through Hardened)
  src/usage.rs              -> A1.2, PI1.3 (UsageMeter, TenantUsageTracker, quota enforcement)
  src/config.rs             -> CC8.2, PI1.1 (Config validation, AuditConfig, TelemetryConfig)
  src/telemetry/            -> CC7.1, P1.2 (structured telemetry, opt-in, crash reporting)
  src/error.rs              -> PI1.2, CC7.2 (10-level error taxonomy)
  src/checkpoint.rs         -> A1.1 (checkpoint/restore system)

crates/clawdius-gateway/
  src/admin.rs              -> CC6.1, CC6.2, CC6.4 (API key auth, tenant CRUD, rate limiting)
  src/rate_limit.rs         -> A1.2 (sliding-window rate limiter)
  src/handler.rs            -> CC7.1 (request processing and logging)

proofs/                     -> CC8.1 (Lean4 formal verification)
deny.toml                   -> CC8.2 (cargo-deny dependency audit)
.cargo-audit.toml           -> CC8.2 (cargo-audit configuration)
```
