# HIPAA Compliance Template

**Version:** 1.0.0
**Last Updated:** 2026-05-31
**Document Owner:** Clawdius Compliance Team
**Review Cycle:** Quarterly
**Regulatory Reference:** 45 CFR Parts 160, 162, 164 (HIPAA Privacy, Security, and Breach Notification Rules)

---

## 1. Document Purpose

This template defines HIPAA compliance requirements for deploying Clawdius in healthcare environments where the tool may process, transmit, or store electronic Protected Health Information (ePHI). It maps HIPAA Security Rule safeguards to Clawdius components and provides a Business Associate Agreement (BAA) template.

This document addresses the risk of AI coding assistants inadvertently processing PHI through code snippets, logs, or LLM prompts. Clawdius does not purposefully process PHI, but healthcare organizations may use it in environments where PHI is present on workstations or in codebases.

**Machine-readable controls:** `crates/clawdius-core/src/compliance.rs` (Framework::Hipaa, control ID `HIPAA-164.312a`)

---

## 2. PHI Handling Requirements

### 2.1 Data Classification

| Classification Level | Description | Clawdius Handling | Status |
|-----------------------|-------------|------------------|--------|
| ePHI | Individually identifiable health information | Clawdius MUST NOT log, transmit, or store ePHI. LLM prompts containing ePHI must be blocked or redacted. | PLANNED |
| Sensitive Data | Credentials, API keys, PII | Encrypted at rest via AES-256-GCM; redacted in logs | COMPLIANT |
| Operational Data | Session metadata, usage stats, telemetry | No PHI content; encrypted storage; audit logging | COMPLIANT |
| Public Data | Code context, non-sensitive project files | No special handling required | COMPLIANT |

### 2.2 Encryption Requirements

| Requirement | HIPAA Reference | Clawdius Implementation | Status |
|-------------|----------------|------------------------|--------|
| Encryption at rest (AES-256 minimum) | 164.312(a)(2)(iv) | AES-256-GCM via `crates/clawdius-core/src/encryption.rs`. HKDF-SHA256 key derivation with random salt per message. AEAD with AAD binding to tenant ID. | COMPLIANT |
| Encryption in transit (TLS 1.2+) | 164.312(e)(1) | rustls-tls on all HTTP clients (no OpenSSL). LLM provider connections enforce HTTPS. | COMPLIANT |
| Key management | 164.312(a)(2)(iv), 164.310(b) | `MasterKey` struct with env var and keyfile loading. Key bytes redacted in Debug/Display. Automated rotation: PLANNED. HSM integration: PLANNED. | IN_PROGRESS |
| End-to-end encryption for LLM traffic | 164.312(e)(2)(ii) | HTTPS to LLM providers. On-prem LLM support for air-gapped deployments via `AirGapConfig::strict()`. | COMPLIANT |

### 2.3 Access Logging

| Requirement | HIPAA Reference | Clawdius Implementation | Status |
|-------------|----------------|------------------------|--------|
| Audit log creation | 164.312(b) | `AuditConfig` with configurable backend (file or SQLite), flush interval, and retention days (default 90). Evidence kind `EvidenceKind::AuditLog` in compliance module. | COMPLIANT |
| Audit log immutability | 164.312(b) | SQLite WAL mode. Append-only log design. Tamper detection: PLANNED. | IN_PROGRESS |
| Audit log retention | 164.530(j) | Configurable `retention_days` (default 90, minimum enforced > 0 in config validation). | COMPLIANT |
| Failed access attempt logging | 164.312(b) | Rate limit exceeded events logged. API key auth failures logged via telemetry. | COMPLIANT |
| User activity tracking | 164.312(b) | `UsageRecord` tracks tenant_id, session_id, user_id, provider, model, timestamps, token counts. Persisted to SQLite via `TenantUsageTracker::save_to_sqlite()`. | COMPLIANT |

---

## 3. Business Associate Agreement Template

The following clauses are recommended for inclusion in a BAA between a Covered Entity (CE) and Clawdius Inc. (Business Associate, BA) when Clawdius is deployed in a HIPAA-regulated environment.

### 3.1 Required BAA Clauses

**Clause 1 -- Permitted Uses and Disclosures**

The BA shall not use or disclose PHI except as permitted by 45 CFR 164.502 and as necessary to perform the services described in this Agreement. The BA shall not use PHI for any purpose other than the services, including but not limited to marketing, sale, or research without additional authorization.

**Clause 2 -- PHI Safeguards -- Technical**

The BA shall implement the following technical safeguards in accordance with 45 CFR 164.312:
- (a)(1) Unique user identification and emergency access procedure
- (a)(2)(iv) Encryption and decryption of ePHI at rest (AES-256-GCM) and in transit (TLS 1.2+)
- (a)(2)(i) Access controls based on role and minimum necessary standard
- (b) Audit controls with automatic logging of access to ePHI
- (c) Integrity controls to prevent unauthorized alteration or destruction of ePHI
- (e)(1) Transmission security via TLS with certificate validation

**Clause 3 -- PHI Safeguards -- Administrative**

The BA shall designate a HIPAA Privacy Officer and Security Officer, conduct risk assessments per 45 CFR 164.308(a)(1)(ii)(A), and maintain written security policies.

**Clause 4 -- Reporting Security Incidents**

The BA shall report to the CE any unauthorized use, access, or disclosure of PHI within 24 hours of discovery, including the nature of the PHI involved, the individuals affected, and the remediation steps taken.

**Clause 5 -- Subcontractor Requirements**

The BA shall require all subcontractors that create, receive, maintain, or transmit PHI on behalf of the BA to execute a BAA with equivalent safeguards. This includes LLM provider sub-processors.

**Clause 6 -- Termination**

Upon termination, the BA shall return or destroy all PHI in its possession, provide written certification of destruction, and retain no copies except as required by law.

**Clause 7 -- AI-Specific Provisions**

(a) The BA's AI coding assistant shall not retain PHI in model training data.
(b) Prompt data containing PHI sent to LLM providers shall be encrypted in transit and shall not be stored by the LLM provider unless covered by a BAA.
(c) The BA shall provide configuration options to prevent PHI from being included in telemetry or crash reports (air-gapped mode).
(d) The CE is responsible for ensuring that code or data provided to the AI assistant does not contain PHI unless appropriate safeguards are in place.

---

## 4. Mapping to Clawdius Modules

### 4.1 Encryption Module

| HIPAA Safeguard | Module | Implementation |
|----------------|--------|---------------|
| Access Control (164.312(a)(1)) | `crates/clawdius-core/src/encryption.rs:211-269` | `MasterKey::from_env()` / `from_file()` with key redaction in Display |
| Encryption at Rest (164.312(a)(2)(iv)) | `crates/clawdius-core/src/encryption.rs:85-111` | `encrypt()` with AES-256-GCM, HKDF-SHA256, random nonce+salt, AAD binding |
| Encryption in Transit (164.312(e)(1)) | `crates/clawdius-gateway/Cargo.toml` | `reqwest` with `rustls-tls` feature; no plaintext HTTP support |

### 4.2 Audit and Compliance Module

| HIPAA Safeguard | Module | Implementation |
|----------------|--------|---------------|
| Audit Controls (164.312(b)) | `crates/clawdius-core/src/config.rs:862-889` | `AuditConfig` with backend, path, flush interval, retention days |
| Compliance Generation | `crates/clawdius-core/src/compliance.rs:348-359` | `Control` for `HIPAA-164.312a` with `PartiallyImplemented` status |
| Evidence Tracking | `crates/clawdius-core/src/compliance.rs:76-115` | `EvidenceRef` with `EvidenceKind::AuditLog`, `EvidenceKind::CodeReview` |

### 4.3 Access Control Module

| HIPAA Safeguard | Module | Implementation |
|----------------|--------|---------------|
| User Authentication (164.312(a)(1)) | `crates/clawdius-gateway/src/admin.rs:36` | Admin API key authentication via `AdminState.api_key` |
| System Account Management (164.312(d)) | `crates/clawdius-gateway/src/admin.rs:203-276` | Tenant CRUD: create, list, get, delete |
| Automatic Logoff (164.312(a)(2)(iii)) | -- | Not implemented. PLANNED for session timeout. |
| Encryption Key Management (164.312(a)(2)(iv)) | `crates/clawdius-core/src/encryption.rs:180-208` | Key load from env/file, key save to file (hex-encoded). Rotation: PLANNED. |

### 4.4 Air-Gap and Telemetry Module

| HIPAA Safeguard | Module | Implementation |
|----------------|--------|---------------|
| PHI in Telemetry | `crates/clawdius-core/src/airgap.rs:50-83` | `AirGapConfig::strict()` blocks all telemetry, crash reports, and auto-updates. Enforced via `AirGapEnforcer`. |
| Opt-In Consent | `crates/clawdius-core/src/telemetry/mod.rs` | `TelemetryConfig` with opt-in flag |
| On-Prem Deployment | `crates/clawdius-core/src/airgap.rs:17-33` | `local_storage_only` flag, allowlist-based outbound control |

### 4.5 Sandbox Module

| HIPAA Safeguard | Module | Implementation |
|----------------|--------|---------------|
| Workforce Security (164.308(a)(3)) | `crates/clawdius-core/src/sandbox.rs:106-117` | 4-tier sandbox: `TrustedAudited`, `Trusted`, `Untrusted`, `Hardened`. OS-level isolation for AI-generated code via Firecracker/gVisor/bubblewrap. |
| System Integrity (164.308(a)(1)(ii)(B)) | `crates/clawdius-core/src/sandbox.rs:96-101` | Backend support: `backends/`, `executor/`, `firewall/`, `safety/`, `wasi/` |

---

## 5. Risk Assessment for Healthcare Use of AI Coding Tools

### 5.1 Risk Register

| Risk ID | Risk Description | Likelihood | Impact | Risk Level | Mitigation | Status |
|---------|-----------------|-----------|--------|------------|-----------|--------|
| R-HIPAA-001 | PHI embedded in code snippets sent to LLM provider | Medium | Critical | High | Air-gapped mode blocks external calls; on-prem LLM support; BAA with LLM providers | IN_PROGRESS |
| R-HIPAA-002 | PHI in crash reports or telemetry | Low | Critical | Medium | Opt-in telemetry; air-gap mode disables all external data; structured telemetry excludes payload content | COMPLIANT |
| R-HIPAA-003 | PHI in session history (SQLite) | Medium | High | High | AES-256-GCM encryption for sensitive fields; configurable retention with automated cleanup | IN_PROGRESS |
| R-HIPAA-004 | Unauthorized access to ePHI via compromised API key | Low | Critical | Medium | API key auth on admin endpoints; rate limiting; RBAC planned | IN_PROGRESS |
| R-HIPAA-005 | AI-generated code introduces PHI vulnerability | Low | High | Medium | Sandbox execution tiers; code review before execution in healthcare contexts | PLANNED |
| R-HIPAA-006 | LLM provider data retention of prompts containing PHI | Medium | Critical | High | BAA requirement for LLM sub-processors; on-prem LLM option; zero-retention configuration | PLANNED |
| R-HIPAA-007 | Encryption key compromise | Low | Critical | Medium | HKDF per-message key derivation; AAD binding; key redaction in logs; planned key rotation | IN_PROGRESS |

### 5.2 Risk Scoring Matrix

```
         | Negligible (1) | Minor (2) | Moderate (3) | Major (4) | Critical (5)
---------|----------------|-----------|---------------|-----------|---------------
Certain  |       5        |    10     |      15       |    20     |     25
Likely   |       4        |     8     |      12       |    16     |     20
Possible |       3        |     6     |       9       |    12     |     15
Unlikely |       2        |     4     |       6       |     8     |     10
Rare     |       1        |     2     |       3       |     4     |      5

Risk Level: <= 4 Low | 5-9 Medium | 10-15 High | > 15 Critical
```

### 5.3 Healthcare Deployment Recommendations

1. **Mandatory air-gap mode** for any deployment where PHI may be present: `AirGapConfig::strict()` with `local_storage_only: true`
2. **On-premises LLM only** -- disable all external LLM provider connections by configuring `allowed_hosts` to exclude external providers
3. **Enable all audit logging** with `AuditConfig` set to SQLite backend, minimum 6-year retention (per 45 CFR 164.530(j))
4. **Encrypt all session storage** using `MasterKey` with HSM-backed key management
5. **RBAC enforcement** before production deployment (Phase F)
6. **Execute BAA** with Clawdius Inc. and all LLM sub-processors
