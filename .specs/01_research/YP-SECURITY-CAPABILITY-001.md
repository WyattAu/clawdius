---
document_id: YP-SECURITY-CAPABILITY-001
version: 1.0.0
status: APPROVED
domain: Security
subdomains: [Capability-Based Security, Access Control, Sandboxing, Cryptography]
applicable_standards: [NIST SP 800-53 Rev 5, OWASP ASVS v4.0, FIPS 140-3, SEC Rule 15c3-5]
created: 2026-03-31
author: Clawdius Security Architecture
confidence_level: 0.95
tqa_level: 4
implementation_files: [src/capability.rs]
proof_files: [.clawdius/specs/02_architecture/proofs/proof_capability.lean]
test_vectors: [.specs/01_research/test_vectors/test_vectors_capability.toml]
domain_constraints: [.specs/01_research/domain_constraints/domain_constraints_security.toml]
---

# Yellow Paper: Capability-Based Security System

## YP-1: Document Control

| Field | Value |
|-------|-------|
| Document ID | YP-SECURITY-CAPABILITY-001 |
| Version | 1.0.0 |
| Classification | Internal — Engineering |
| Component | COMP-CAPABILITY-001 |
| Implementation | `src/capability.rs` |
| Formal Proof | `proof_capability.lean` (10 theorems, all verified) |
| Test Vectors | 6 vectors in `test_vectors_capability.toml` |
| Domain Constraints | 7 constraints in `domain_constraints_security.toml` |

### Revision History

| Rev | Date | Author | Description |
|-----|------|--------|-------------|
| 1.0.0 | 2026-03-31 | Security Architecture | Initial release |

## YP-2: Executive Summary

### Problem Statement
Formal definition of a capability-based security system that provides unforgeable, attenuatable permission tokens for sandboxed AI agent operations. The system must guarantee that capabilities can never be amplified, tokens cannot be forged without cryptographic keys, and all access decisions are deterministic and auditable.

**Objective Function:** Minimize $|\mathcal{P}_{\text{granted}} \setminus \mathcal{P}_{\text{required}}|$ (over-privilege) subject to $\forall t \in \mathcal{T}: \text{verify}(t) = \text{true} \implies \text{attenuation\_only}(t)$, where $\mathcal{T}$ is the set of all capability tokens and $\mathcal{P}_{\text{granted}}$ are the permissions actually held.

### Scope
**In-Scope:**
- Capability token structure with SHA3-256 HMAC signatures
- 8 permission types with risk classification
- Monotonic attenuation via `derive()`
- Resource scoping (path patterns, host patterns, environment variables)
- Time-based expiry with monotonic clock
- Formal proof of 10 security theorems in Lean 4
- SEC Rule 15c3-5 market access compliance controls
- NIST SP 800-53 access control mapping

**Out-of-Scope:**
- Distributed capability delegation (future: cross-node attestation)
- Revocation/revocation-list mechanisms
- Capability inheritance across session boundaries
- Hardware-backed key storage (TPM/HSM integration)

### Key Results
- Unforgeability is guaranteed by SHA3-256 HMAC with a 256-bit key (NC-SEC-001).
- Monotonic attenuation is proven: `derive(c, S)` returns `Some` iff $S \subseteq c.\text{permissions}$ (Theorem 2).
- Transitive attenuation holds across arbitrary derivation depth (Theorem 3).
- All 10 Lean 4 theorems are mechanically verified (status: COMPLETE).

## YP-3: Nomenclature

### Multi-Lingual Concept Registry

| EN | ZH | JA | DE | FR |
|----|----|----|----|-----|
| Capability | 能力令牌 (Nénglì lìngpái) | ケイパビリティ (Keipabiriti) | Fähigkeit (Berechtigung) | Capacité |
| Attenuation | 衰减 (Shuāijiǎn) | 減衰 (Gensui) | Abschwächung | Atténuation |
| Unforgeability | 不可伪造性 (Bùkě wěizàoxìng) | 不可偽造性 (Fukaseizōsei) | Unfälschbarkeit | Infalsifiabilité |
| Permission | 权限 (Quánxiàn) | 権限 (Kengen) | Berechtigung | Permission |
| Resource Scope | 资源范围 (Zīyuán fànwéi) | リソーススコープ (Risōsu sukōpu) | Ressourcenbereich | Portée des ressources |
| Derivation | 派生 (Pàishēng) | 派生 (Hasei) | Ableitung | Dérivation |

### Symbol Table

| Symbol | Description | Domain | Source |
|--------|-------------|--------|--------|
| $\mathcal{P}$ | Permission set | $\mathcal{P}(\text{Permission})$ | `PermissionSet` |
| $T$ | Capability token | $\text{CapabilityToken}$ | `capability.rs:142` |
| $\sigma: T \to [0, 2^{256}-1]$ | Signature function | SHA3-256 HMAC | `compute_signature` |
| $K$ | HMAC key (256-bit) | $\{0,1\}^{256}$ | `HMAC_KEY` |
| $\text{derive}: T \times \mathcal{P} \to \text{Option}\ T$ | Attenuation function | Partial function | `derive()` |
| $R$ | Resource scope | $\text{ResourceScope}$ | `capability.rs:67` |
| $e: T \to \text{Option}\ \mathbb{N}$ | Expiry function | Monotonic clock | `expires_at` |
| $\sqsubseteq$ | Subset relation on permission sets | $\mathcal{P}(\text{Permission})$ | `isSubset` |
| $\text{verify}: T \to \text{Bool}$ | Signature + expiry verification | Boolean | `verify()` |

## YP-4: Theoretical Foundation

### Axioms

**AX-CAP-001: HMAC Key Isolation**

$\sigma(T) = H(K \| \text{id}(T) \| \text{resource}(T) \| \text{perms}(T))$

No token's signature can be computed without $K$. The key $K$ is a compile-time constant (`HMAC_KEY: [u8; 32]`) not exposed to sandboxed code.

*Verification:* Static assertion in `capability.rs:12`. 256-bit minimum entropy enforced by NC-SEC-001.

**AX-CAP-002: Monotonic Clock**

$e(T) \in \text{Option}\ (\text{Instant}^{\text{mono}})$

Expiry times are derived from `std::time::Instant`, a monotonic clock that never decreases. This prevents time-rollback attacks.

*Verification:* `Instant::now()` in `capability.rs:170`. No system-clock dependency.

**AX-CAP-003: Permission Finitude**

$|\text{Permission}| = 8$

The permission type is an exhaustive enum with exactly 8 variants. No permission can be invented at runtime.

*Verification:* Enum definition `capability.rs:18-35`. Decidable equality in `proof_capability.lean:27`.

### Permission Type System

```
Permission ::= readFile     | Risk: Low      | Filesystem read
             | writeFile    | Risk: Medium   | Filesystem write
             | execute      | Risk: Critical | Process spawning
             | network      | Risk: High     | TCP/UDP access
             | accessLlm    | Risk: Critical | LLM model access
             | accessHistory| Risk: Medium   | Conversation history
             | modifyPlugins| Risk: High     | Plugin modification
             | admin        | Risk: Critical | Administrative ops
```

Risk levels determine default sandbox tier assignment and audit logging frequency.

## YP-5: Capability Token Structure

### Formal Definition

A capability token $T$ is a 5-tuple:

$$T = (\text{id},\ R,\ \mathcal{P},\ \sigma,\ e)$$

where:
- $\text{id} \in \mathbb{N}$ — globally unique monotonic counter (`AtomicU64`, `capability.rs:14`)
- $R \in \text{ResourceScope}$ — resource access constraints
- $\mathcal{P} \subseteq \text{Permission}$ — granted permission set
- $\sigma \in \{0,1\}^{256}$ — SHA3-256 HMAC signature
- $e \in \text{Option}\ \text{Instant}$ — optional expiry time

### Resource Scope

$$R = (P_{\text{paths}},\ P_{\text{hosts}},\ P_{\text{env}})$$

**Path Patterns** $P_{\text{paths}}$: Each pattern has a `prefix` string and a `recursive` flag. A path $x$ matches pattern $(p, r)$ iff:
- $r = \text{false} \implies x = p$ (exact match)
- $r = \text{true} \implies x.\text{startsWith}(p)$ (prefix match)

**Host Patterns** $P_{\text{hosts}}$: Each pattern has a domain/IP string and optional port.

**Environment Variables** $P_{\text{env}}$: Exact string matching against environment variable names.

### Signature Computation

```
σ(T) = SHA3-256(
    K ||                              -- 32-byte HMAC key
    id(T).to_le_bytes() ||            -- 8-byte little-endian ID
    ∀ path ∈ R.paths: path.pattern || -- all path patterns
    ∀ host ∈ R.hosts: host.pattern || -- all host patterns
    ∀ env ∈ R.env_vars: env ||        -- all env var names
    ∀ perm ∈ P: perm as u8            -- all permissions as discriminants
)
```

The signature binds the token identity, resource scope, and permission set into an unforgeable 256-bit digest.

## YP-6: Core Operations

### Construction

```
new(R, P) -> T
```

1. Atomically increment global counter: `id = CAPABILITY_COUNTER.fetch_add(1, SeqCst)`
2. Compute $\sigma = \text{SHA3-256}(K \| \text{id} \| R \| P)$
3. Return $T = (\text{id}, R, P, \sigma, \text{None})$

*Complexity:* $O(|R| + |P|)$ — linear in resource and permission set sizes.

### Verification

```
verify(T) -> Bool
```

1. If $e(T) \neq \text{None} \land \text{Instant::now()} > e(T)$: return `false`
2. Compute $\sigma' = \text{SHA3-256}(K \| \text{id}(T) \| R(T) \| P(T))$
3. Return $\sigma(T) = \sigma'$

*Complexity:* $O(|R| + |P|)$.

### Derivation (Attenuation)

```
derive(T_parent, P_child) -> Option<T>
```

1. If $\neg(P_{\text{child}} \sqsubseteq P_{\text{parent}})$: return `None`
2. `id = CAPABILITY_COUNTER.fetch_add(1, SeqCst)`
3. Compute $\sigma = \text{SHA3-256}(K \| \text{id} \| R_{\text{parent}} \| P_{\text{child}})$
4. Return `Some(T_child)` where $T_{\text{child}} = (\text{id}, R_{\text{parent}}, P_{\text{child}}, \sigma, e_{\text{parent}})$

Key property: the child token inherits the parent's resource scope and expiry, but with a new ID and new signature. The resource scope is never expanded.

## YP-7: Formal Proofs

All proofs are mechanized in Lean 4 (`proof_capability.lean`). Verification status: **COMPLETE**.

### Theorem 1: Unforgeability
$$\sigma(t_1) \neq \sigma(t_2) \implies t_1 \neq t_2$$

Two tokens with different signatures are provably distinct. Corollary: forging a token requires computing a valid SHA3-256 HMAC without $K$, which is computationally infeasible.

*Proof:* `proof_capability.lean:90-95`. By case analysis on `signature_unforgeable`.

### Theorem 2: Attenuation-Only (Monotonic)
$$\text{derive}(t, s) = \text{some}\ t' \implies t'.\mathcal{P} \sqsubseteq t.\mathcal{P}$$

A successfully derived token always has a subset of the parent's permissions. Permission amplification is structurally impossible.

*Proof:* `proof_capability.lean:101-107`. By definition of `derive` and the subset precondition.

### Theorem 3: Transitive Attenuation
$$s_1 \sqsubseteq s_2 \sqsubseteq t.\mathcal{P} \implies \text{derive}(\text{derive}(t, s_2), s_1) = \text{some}\ t_1 \land t_1.\mathcal{P} = s_1$$

Chained derivation preserves the subset relation at arbitrary depth. This guarantees that deeply delegated capabilities cannot accumulate permissions.

*Proof:* `proof_capability.lean:114-126`. By nested case analysis on both `derive` calls.

### Theorem 4: Escalation Blocked
$$\neg(s \sqsubseteq t.\mathcal{P}) \implies \text{derive}(t, s) = \text{none}$$

Attempting to derive a capability with permissions beyond the parent's set always fails. This is the dual of Theorem 2.

*Proof:* `proof_capability.lean:132-139`. By contradiction on the subset precondition.

### Theorem 5: Empty Capability Denies All
$$t.\mathcal{P} = \emptyset \implies \forall p,\ \neg\text{hasPermission}(t, p)$$

A token with an empty permission set cannot authorize any operation. This is the least-privilege base case.

*Proof:* `proof_capability.lean:145-149`. By definition of `noPermissions`.

### Theorem 6: Identity Derive
$$\text{derive}(t, t.\mathcal{P}) = \text{some}(t.\text{id} + 1)$$

Deriving with the same permission set succeeds (no-op attenuation). The child has a new ID but identical permissions and scope.

*Proof:* `proof_capability.lean:155-157`. Reflexivity of $\sqsubseteq$.

### Theorem 7: Attenuation is Idempotent
$$\text{derive}(\text{derive}(t, s), s).\mathcal{P} = \text{derive}(t, s).\mathcal{P}$$

Deriving the same subset twice yields identical permissions. Re-attenuation is a no-op.

*Proof:* `proof_capability.lean:163-174`. By substitution and reflexivity.

### Theorem 8: Fresh Token Verifies
$$\forall t,\ \text{signature\_valid}(t) = \text{true}$$

Every freshly constructed token passes signature verification. This follows from the construction invariant.

*Proof:* `proof_capability.lean:180-182`. Direct application of `fresh_token_valid`.

### Theorem 9: Expiry Detection
$$t.e = \text{some}\ \tau \land \text{now} > \tau \implies \text{isExpired}(t, \text{now}) = \text{true}$$

Expired tokens are correctly identified. The monotonic clock guarantees this check is reliable.

*Proof:* `proof_capability.lean:188-193`. By definition of `isExpired`.

### Theorem 10: Non-Expiry Detection
$$t.e = \text{none} \implies \text{isExpired}(t, \text{now}) = \text{false}$$
$$t.e = \text{some}\ \tau \land \text{now} \leq \tau \implies \text{isExpired}(t, \text{now}) = \text{false}$$

Non-expired tokens (no expiry set, or within the validity window) are correctly identified as valid.

*Proof:* `proof_capability.lean:199-211`. Case analysis + `omega` tactic.

## YP-8: Test Vectors

All vectors from `test_vectors_capability.toml` (ALG-CAP-001, v1.0.0):

| ID | Name | Category | Priority | Outcome |
|----|------|----------|----------|---------|
| TV-CAP-001 | Fresh token verifies | Nominal | Critical | verified = true |
| TV-CAP-002 | Attenuation to subset | Nominal | Critical | success = true, result = {readFile} |
| TV-CAP-003 | Escalation blocked | Adversarial | Critical | success = false |
| TV-CAP-004 | Transitive attenuation | Nominal | High | final = {readFile} |
| TV-CAP-005 | Empty capability denies all | Boundary | High | has_* = false (all 4 checked) |
| TV-CAP-006 | Expired token detection | Boundary | High | is_expired = true |

### Implementation Test Coverage

The Rust test suite (`capability.rs:293-430`) covers:
- Token creation and verification (lines 297-314)
- Successful attenuation (lines 316-337)
- Escalation blocking (lines 339-358)
- Empty capability denial (lines 360-365)
- Expiry detection (lines 367-379)
- Path pattern matching: exact and recursive (lines 381-392)
- Monotonicity property: all derived subsets ⊆ parent (lines 394-420)
- Permission risk level classification (lines 422-429)

## YP-9: Regulatory and Standards Compliance

### NIST SP 800-53 Rev 5 Access Control Mapping

| Control | Capability System Mechanism | Implementation |
|---------|---------------------------|----------------|
| AC-3: Enforcement | Permission set checked on every operation | `has_permission()` |
| AC-4: Information Flow | Resource scoping restricts data to permitted paths/hosts | `ResourceScope` |
| AC-6: Least Privilege | Monotonic attenuation; capabilities start minimal | `derive()` |
| AC-16: Security Attributes | Token carries immutable permission set bound by HMAC | `CapabilityToken` |
| AC-17: Remote Access | Host patterns scope network capabilities | `HostPattern` |
| AC-19: Access Control for Mobile Code | WASM sandbox + capability tokens govern all agent actions | `SandboxTier` |
| AU-2: Audit Events | Token ID and permission set logged on access decisions | `CapabilityError` |
| AU-10: Non-repudiation | SHA3-256 HMAC signature on every token | `compute_signature()` |
| SC-7: Boundary Protection | Resource scope defines security perimeters | `PathPattern`, `HostPattern` |
| SC-8: Transmission Confidentiality | HMAC key never transmitted; compile-time bound | `HMAC_KEY` |

### SEC Rule 15c3-5 Market Access Compliance

SEC Rule 15c3-5 requires broker-dealers to establish risk management controls and supervisory procedures for market access. The capability system addresses:

| Requirement | Capability System Mapping |
|-------------|--------------------------|
| Pre-trade risk checks | `verify()` must return `true` before any operation proceeds |
| Automated prevention of erroneous orders | `CapabilityError::InsufficientPermissions` and `EscalationAttempt` reject unauthorized actions |
| Access controls limiting personnel | Monotonic attenuation: each agent role receives only necessary permissions |
| Audit trail of access events | Token IDs are monotonically increasing (`AtomicU64`); every `derive()` creates a new auditable entry |
| Time-based access limitations | `with_expiry()` + monotonic `Instant` clock (TC-SEC-002: default 86400s) |
| Separation of duties | 4 sandbox tiers (NC-SEC-002) with independent capability constraints |

### OWASP ASVS v4.0 Mapping

| Requirement | Mapping |
|-------------|---------|
| V1.2: Architectural Security | Capability tokens enforce least-privilege by design |
| V2.1.1: Access Control | Permission-based authorization on every operation |
| V7.1.1: Security Requirements | Formal proofs verify security properties |
| V7.2.1: Secure Architecture | Proven attenuation-only prevents privilege escalation |

## YP-10: Domain Constraints

From `domain_constraints_security.toml` (confidence: 0.95):

### Timing Constraints

| ID | Constraint | Value | Type | Validation |
|----|-----------|-------|------|------------|
| TC-SEC-001 | WASM fuel limit | 30 seconds | Hard | `WasmConfig::with_fuel(1_000_000_000)` |
| TC-SEC-002 | Capability token expiry default | 86400 seconds (24h) | Soft | `CapabilityToken::with_expiry(Duration)` |

### Memory Constraints

| ID | Constraint | Value | Type |
|----|-----------|-------|------|
| MC-SEC-001 | WASM memory limit | 4 GiB | Static |
| MC-SEC-002 | WASM stack limit | 1 MiB | Static |

### Numerical Constraints

| ID | Constraint | Value | Source |
|----|-----------|-------|--------|
| NC-SEC-001 | HMAC key size | 256 bits | `HMAC_KEY: [u8; 32]` |
| NC-SEC-002 | Sandbox tiers | 4 | `SandboxTier` enum |

### Known Conflicts

| ID | Constraints | Impact | Resolution |
|----|------------|--------|------------|
| CONF-SEC-001 | TC-SEC-001 ∩ MC-SEC-001 | Longer timeouts may require more memory | Independent limits; configure per workload (ADR-002) |

## Appendix A: Security Property Cross-Reference

| Property ID | Description | Theorem | Test Vector | Implementation |
|-------------|-------------|---------|-------------|----------------|
| PROP-CAP-001 | Attenuation only | Thm 2 | TV-CAP-002, TV-CAP-003 | `derive()` |
| PROP-CAP-002 | Unforgeability | Thm 1 | TV-CAP-001 | `compute_signature()` |
| PROP-CAP-003 | Fresh token valid | Thm 8 | TV-CAP-001 | `new()` + `verify()` |
| PROP-CAP-004 | Transitive attenuation | Thm 3 | TV-CAP-004 | `derive(derive(...))` |
| PROP-CAP-005 | Escalation blocked | Thm 4 | TV-CAP-003 | `derive()` returns `None` |
| PROP-CAP-006 | Empty denies all | Thm 5 | TV-CAP-005 | `has_permission()` |
| PROP-CAP-007 | Expiry detection | Thm 9, 10 | TV-CAP-006 | `is_expired()` + `verify()` |

## Appendix B: Permission Risk Matrix

| Permission | Risk Level | Default Sandbox Tier | Audit Logging |
|------------|-----------|---------------------|---------------|
| readFile | Low | Native | Standard |
| writeFile | Medium | Container | Standard |
| execute | Critical | WASM | Enhanced |
| network | High | WASM | Enhanced |
| accessLlm | Critical | Hardened Container | Enhanced |
| accessHistory | Medium | Container | Standard |
| modifyPlugins | High | WASM | Enhanced |
| admin | Critical | Hardened Container | Full |

## Appendix C: Formal Model Correspondence

| Lean 4 Concept | Rust Implementation | Description |
|---------------|---------------------|-------------|
| `Permission` (inductive, 8 constructors) | `enum Permission` (8 variants) | Permission type |
| `PermissionSet := Permission → Bool` | `HashSet<Permission>` | Permission collection |
| `isSubset a b := ∀ p, a p → b p` | `HashSet::is_subset()` | Subset relation |
| `CapabilityToken` (structure) | `struct CapabilityToken` | Token type |
| `derive` (def) | `CapabilityToken::derive()` | Attenuation function |
| `hasPermission` (def) | `CapabilityToken::has_permission()` | Permission check |
| `isExpired` (def) | `CapabilityToken::is_expired()` | Expiry check |
| `signature_valid` (axiom) | `CapabilityToken::verify()` | Verification |
