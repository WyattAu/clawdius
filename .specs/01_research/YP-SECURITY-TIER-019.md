---
document_id: YP-SECURITY-TIER-019
version: 1.0.0
status: APPROVED
domain: Security / Sandbox Tier Policy
subdomains: [Isolation Classes, Tier Annotation, Policy Honesty]
created: 2026-10-17
author: Loop
proof_files: [tier_isolation.lean]
confidence_level: 0.93
tqa_level: 4
research_iterations: 1
---

# Yellow Paper: Sandbox Tier × Isolation Class — the Policy Must Not Claim What the Boundary Does Not Deliver

## YP-1: Research Basis

| Source | What it grounds |
|---|---|
| YP-CONTAINER-014 (this estate, v1.1.0) | The three isolation classes and their trust outcomes: sharedKernel and intercepted (gVisor) leave the host kernel in the guest TCB; only microVM closes the kernel class. Proven, not cited. |
| YP-SECURITY-POLICY-011 + SandboxPolicy.selectTier | The shipped tier policy: audited → tier1, trusted → tier2, LLM reasoning → tier3 (always), untrusted → tier4. Proven total. |
| gVisor docs, architecture guide/security (carried from YP-CONTAINER-014 it.4) | gVisor is *additional* defense; the host kernel stays in the TCB (memory delegated). |
| NIST SP 800-190 §3 | Tier/isolation guidance: isolation level must match workload trust. |

## YP-2: The gap found in the spec

The two models this estate proved never touched. `selectTier` assigned
tiers as opaque labels — nothing recorded WHAT each tier isolates — and
the isolation model said nothing about WHO gets which class. The
un sound combination was available: a policy edit could route untrusted
workloads through tier2 (shared kernel) and every proof would stay
green, because no theorem constrained the tier→class mapping. A policy
could also have marketed tier3 as "microVM-grade" — nothing contradicted
it.

## YP-3: Analysis — the field's answer

Production platforms annotate tiers with isolation mechanisms: Google's
gVisor runsc vs runc runtime selection, AWS Fargate vs EC2 launch types,
Fly.io's firecracker-vs-process machines. The annotation is first-class
configuration precisely so that policy review can CHECK it. The estate's
equivalent: the annotation becomes a total function
(`tierIsolation : SandboxTier → Isolation`) and the review constraints
become theorems.

## YP-4: The corrected model

- `tierIsolation`: tier1 → sharedKernel, tier2 → sharedKernel,
  tier3 → intercepted, tier4 → microVM.
- `annotatedTier (toolchain, trust) = (selectTier …, tierIsolation …)` —
  the shipped policy with its class attached.
- Asserted (T4): the annotation agrees with the shipped policy on all 21
  combinations — a spec of what IS deployed, not a parallel wish.
- Asserted (T3): audited toolchains get tier1 = sharedKernel — the hard
  truth that tier1 workloads carry the host kernel in their TCB, so the
  container hardening bundle stays mandatory.
- Asserted (T2): the LLM tier is `intercepted` AND
  `hostKernelInTCB intercepted = true` — tier3 does not close the kernel
  class; it is reduced surface, and the machine-checked record keeps that
  admission visible.
- Asserted (T1): untrusted non-LLM toolchains get tier4 = microVM, the
  only class with `hostKernelInTCB = false` — the kernel class is closed
  for exactly the workloads that need it closed.

## YP-5: Falsifiable predictions

- P1 (proved, T1): untrusted non-LLM ⇒ microVM ⇒ kernel class closed.
  Regression to a sharedKernel/intercepted tier for untrusted workloads
  is now a BUILD FAILURE.
- P2 (proved, T2): LLM ⇒ intercepted, with the kernel-class admission
  kept in the same theorem — the honesty cannot be separated from the
  routing.
- P3 (proved, T3): audited ⇒ sharedKernel ⇒ hardening mandatory.
- P4 (proved, T4): the annotation matches the shipped policy everywhere
  — a parallel policy that drifts from the implementation is a proof
  failure.

## YP-6: Findings (loop 4, iteration 2)

1. **The policy constraint is a corollary of the container findings, but
   only once the annotation exists.** "Untrusted gets microVM" was
   TRUE of the shipped policy before this file existed — and unprovable,
   because the tiers carried no isolation meaning. The gap was not a
   wrong policy; it was an unverifiable one.
2. **T2 is deliberately two conjuncts.** Routing the LLM to tier3 and
   admitting tier3 leaves the kernel class open are ONE theorem. Any
   future edit that changes either half breaks the build — the honesty
   is load-bearing.
3. **Restated models, tied by enumeration.** The file restates
   `Isolation`, `Toolchain`, `TrustLevel`, `SandboxTier` instead of
   importing (the proof tree is import-free by design); the duplication
   is neutralized by T4, which pins the annotation to the shipped
   `selectTier` on every combination. Drift between spec-copy and
   implementation is a proof failure, not a review hope.
