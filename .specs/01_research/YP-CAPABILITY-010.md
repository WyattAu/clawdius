---
document_id: YP-CAPABILITY-010
version: 1.0.0
status: APPROVED
domain: Security / Access Control
subdomains: [Object Capabilities, Attenuation, Non-Delegatable Authority]
created: 2026-10-14
author: Loop
proof_files: [capability.lean]
confidence_level: 0.93
tqa_level: 4
research_iterations: 4
---

# Yellow Paper: Capability System — Attenuation Is Not Enough, Authority Must Be Non-Delegatable

## YP-1: Research Basis

| Source | What it establishes |
|---|---|
| Miller 2006, *Robust Composition* (PhD thesis) | Founding object-capability model: rights transfer only by explicit designation; attenuation alone is one of three composition disciplines. |
| Miller, Yee, Shapiro, *Capability Myths Demolished* | Seven properties separating capability systems from ACL systems: delegation, amplification, confinement, **no ambient authority (D)**, revocation, confined, irrevocability. The paper's honest boundary: proliferation (uncontrolled re-delegation) is *not* prevented by attenuation. |
| seL4, capDL (ASPD workshop 2010) | Capability spaces where every cap carries explicit rights *and* derive discipline (mint/reuse distinguishing grant from use). |
| *Non-Delegatable Authorities in Capability Systems* (Oxford) | The direct answer: a per-capability non-delegatable bit that propagates monotonically — non-delegatable sources yield non-delegatable descendants. |
| OCPL, *Robust and Compositional Verification of OCAP Patterns* (MPI-SWS) | Pattern-level verification (admin, renderer, factory) is compositional when the underlying cap algebra carries these bits. |

## YP-2: The gap found in the spec

`proof_capability.lean` (legacy tree) proved attenuation-only soundness,
escalation blocking, and transitive attenuation — but nothing constrained
*proliferation*: a holder could `derive` unbounded copies of the same
permission set and hand them to any principal. Under *Capability Myths
Demolished*'s taxonomy the estate enforced attenuation (E) and
no-ambient-authority (D) while leaving delegation (A) entirely open — the
exact asymmetry the Oxford paper identifies as "the delegation problem."

## YP-3: Analysis — the field's answer

The field's consensus answer is a **per-capability grantability bit whose
value is chosen at derive time and can only be lost**: seL4's mint/reuse
carry a rights template; EROS/KeyKOS brand discipline; Oxford's
non-delegatable authorities formalize monotone loss. The model is *not*
novel — the honest question is whether our token algebra admits it, and
the answer is yes with a one-field change: `grantable : Bool` on the
token, `deriveGrant` computing the child bit as `parent.grantable && grant`.

**Are we solving a problem the field could not?** No. We are applying a
known discipline so that the Rust `capability.rs` mirror gains
proliferation resistance as a *machine-checked* property — which the
referenced systems verify in Coq/Isabelle, not Lean4.

## YP-4: The corrected model

- `CapabilityToken` gains `grantable : Bool`.
- `deriveGrant token subset grant`: child = attenuated permissions,
  `grantable := token.grantable && grant`.
- `derive token subset := deriveGrant token subset token.grantable`
  (pure attenuation preserves the parent's grantability).
- Asserted (T1): non-grantable source ⇒ every child non-grantable
  (`non_grantable_child`).
- Asserted (T2): a grantable child implies a grantable parent —
  monotone traceability (`grantability_monotone`).
- Asserted (T3): derivation from a non-grantable source never yields a
  grantable token (`non_grantable_yields_non_grantable`).

## YP-5: Falsifiable predictions

- P1: no derivation chain, of any length, from a non-grantable token
  reaches a grantable token. (T1/T3 cover length 1; T2 + induction cover
  all lengths — the induction is future work, recorded honestly.)
- P2: pure `derive` never changes grantability (it is `deriveGrant` with
  `grant := parent.grantable`).
- P3: escalation blocking and attenuation-only soundness continue to hold
  unchanged under the extended model (all legacy theorems re-verified).

## YP-6: Proof outcome (loop 2, iteration 2 — 2026-10-14)

`capability.lean` migrated into the canonical proof tree (legacy
`.clawdius/` copy retired for this component); T1–T3 asserted, all
legacy theorems re-verified. `lake build`: 43 jobs, 0 sorries, 0 errors.
