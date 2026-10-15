---
document_id: YP-AUDIT-013
version: 1.0.0
status: APPROVED
domain: Compliance / Audit Integrity
subdomains: [Tamper-Evident Logging, Hash Chains, Append-Only Storage]
created: 2026-10-15
author: Loop
proof_files: [audit.lean]
confidence_level: 0.93
tqa_level: 4
research_iterations: 3
---

# Yellow Paper: Audit — The Integrity Layer Must Commit to Content, Not Assert It

## YP-1: Research Basis

| Source | What it establishes |
|---|---|
| RFC 6962, Certificate Transparency (2013) | Append-only logs via Merkle trees; consistency proofs let any party detect a log showing different histories to different people; logs "do not prevent misissue, but ensure detection". |
| Crosby & Wallach (2009), *Efficient Tamper-Evident Logs* | Foundational: hash-built authenticated data structures make log tampering detectable with efficient proofs. |
| Russ Cox, *Transparent Logs for Skeptical Clients* (2019) | The client-side view: verification must not trust the log operator; prefix consistency is checkable from proofs alone. |
| Stebila et al., *Secure Logging Schemes and CT* (ESORICS 2016) | Security model for logging schemes: tamper-evidence, append-only, and the trust boundary of the hash function. |

## YP-2: The gap found in the spec

The legacy audit model proved real properties — append-only (`modifyEvent`
total: `none`), size bounds, event preservation, query completeness,
SOC2 action coverage — but its **integrity layer was vacuous**:
`computeChecksum _ = ""` and `verifyIntegrity _ _ = true`. Any tampering
(reordering, truncation, substitution) was invisible to the model. A
compliance story built on that layer detects nothing.

## YP-3: Analysis — the field's answer

RFC 6962 and Crosby–Wallach: bind each entry cryptographically to its
position and to the previous entries; publish the head; then **any
divergence between two histories under the same head is efficiently
detectable**. The log itself is untrusted — only the head and the hash
function are. Position binding is load-bearing: it converts reordering
into a detected event (hash chains without position binding miss swaps).

**Are we solving a problem the field could not?** No. The estate's
contribution is replacing vacuous stubs with the field's construction,
machine-checked in Lean4: chain injectivity (T0), tamper evidence (T1),
and append-only consistency (T2) — with the hash's collision-resistance
recorded as the same class of justified cryptographic assumption as the
capability system's signature postulate.

## YP-4: The corrected model

- `ehash : AuditEvent → Nat` with `ehash_inj` (axiomatized
  collision-resistance — justified assumption, documented).
- `chainOf events = events.map ehash` — position-bound (map preserves
  order), length-bound (truncation detected).
- `Verify events stored := chainOf events = stored`; the honest head is
  `chainOf events` itself (`verify_holds`, rfl).
- Asserted (T0): `chainOf_inj` — the chain determines the events.
- Asserted (T1): `tamper_evident` — two sequences verifying under one
  stored chain are identical.
- Asserted (T2): `chain_append` + `verify_append` — appending extends
  the chain and preserves verification (RFC 6962 consistency, list
  form).
- Legacy properties re-verified under the migrated model:
  event_immutability, queryRange completeness, size bounds, event
  preservation, SOC2 coverage.

## YP-5: Falsifiable predictions

- P1 (proved, T1): there is no pair of distinct event sequences that
  verify under the same stored chain head.
- P2 (proved, T2): for every suffix event `e`, the appended log
  verifies against the extended head — the monitor's consistency check
  always succeeds for honest appends.
- P3 (recorded, not asserted): an external signer on the chain head
  (periodic anchoring, per the zatona/CT checklist) — the model trusts
  whoever stores the head; anchoring distributes that trust. Next
  iteration's work, alongside the component-identity closure of
  YP-SECURITY-POLICY-011 P3.
