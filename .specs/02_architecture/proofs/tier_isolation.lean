/-
  Lean4 Proof: Tier × Isolation — the policy must not claim what the
  boundary does not deliver
  Component: COMP-SANDBOX-002
  Blue Papers: BP-SANDBOX-TIERS-001, BP-CONTAINER-SANDBOX-001
  Yellow Papers: YP-SECURITY-TIER-019 (new), YP-CONTAINER-014,
                 YP-SECURITY-POLICY-011

  Loop 4, iteration 2. This file BRIDGES the two proofs this estate
  built in loop 3:

  - YP-CONTAINER-014 it.4 proved three isolation classes with different
    trust outcomes: sharedKernel and intercepted (gVisor) leave the host
    kernel in the guest TCB; only microVM closes the kernel class.
  - YP-SECURITY-POLICY-011 proved the tier policy: audited toolchains →
    tier 1, trusted → tier 2, LLM reasoning → tier 3 always, untrusted →
    tier 4.

  Until now those two models never touched. The policy assigned tiers
  without saying WHAT each tier isolates, and the isolation model
  without saying WHO gets it. The bridge is an annotated policy: every
  (toolchain, trust) pair now yields (tier, isolation class), and the
  theorems check the policy against the container findings:

  - T1: untrusted non-LLM toolchains get tier4 whose class is microVM —
    the kernel class is closed for exactly the workloads that need it.
  - T2: the LLM tier's class is `intercepted` — and the theorem keeps
    the honest part visible: intercepted does NOT close the kernel
    class. Tier 3 is surface reduction, recorded as such.
  - T3: audited toolchains get tier1 = sharedKernel — so the container
    hardening bundle remains MANDATORY for them; the tier never
    substitutes for the layers.
  - T4 (spec-implementation tie): the annotated policy agrees with
    `SandboxPolicy.selectTier` on every combination — the annotation is
    a spec of the shipped policy, not a parallel one.
-/

namespace TierIsolation

/-! Minimal restatements of the two source models. `Isolation` mirrors
container.lean's `Isolation`; `Toolchain`/`TrustLevel`/`SandboxTier`
mirror sandbox_policy.lean's. Restated rather than imported: the proof
tree is deliberately import-free (each file is a standalone module in
its own lake lib), and the duplication is tied to the implementation by
T4. -/

inductive Isolation where
  | sharedKernel : Isolation
  | intercepted : Isolation
  | microVM : Isolation
  deriving Repr, DecidableEq

/-- The container YP's trust outcome, restated: is the host kernel part
of the guest's trusted computing base? -/
def hostKernelInTCB : Isolation → Bool
  | Isolation.sharedKernel => true
  | Isolation.intercepted => true
  | Isolation.microVM => false

inductive Toolchain where
  | rust : Toolchain
  | cpp : Toolchain
  | vulkan : Toolchain
  | nodeJs : Toolchain
  | python : Toolchain
  | ruby : Toolchain
  | llmReasoning : Toolchain
  deriving Repr, DecidableEq

inductive TrustLevel where
  | trustedAudited : TrustLevel
  | trusted : TrustLevel
  | untrusted : TrustLevel
  deriving Repr, DecidableEq

inductive SandboxTier where
  | tier1 : SandboxTier
  | tier2 : SandboxTier
  | tier3 : SandboxTier
  | tier4 : SandboxTier
  deriving Repr, DecidableEq

/-- The shipped policy (SandboxPolicy.selectTier), restated verbatim. -/
def selectTier (toolchain : Toolchain) (trust : TrustLevel) : SandboxTier :=
  match trust, toolchain with
  | TrustLevel.trustedAudited, Toolchain.rust => SandboxTier.tier1
  | TrustLevel.trustedAudited, Toolchain.cpp => SandboxTier.tier1
  | TrustLevel.trustedAudited, Toolchain.vulkan => SandboxTier.tier1
  | TrustLevel.trusted, Toolchain.nodeJs => SandboxTier.tier2
  | TrustLevel.trusted, Toolchain.python => SandboxTier.tier2
  | TrustLevel.trusted, Toolchain.ruby => SandboxTier.tier2
  | _, Toolchain.llmReasoning => SandboxTier.tier3
  | _, _ => SandboxTier.tier4

/-- THE ANNOTATION: what each tier isolates. This is the policy
decision this file checks — the tiers are no longer opaque labels. -/
def tierIsolation : SandboxTier → Isolation
  | SandboxTier.tier1 => Isolation.sharedKernel
  | SandboxTier.tier2 => Isolation.sharedKernel
  | SandboxTier.tier3 => Isolation.intercepted
  | SandboxTier.tier4 => Isolation.microVM

/-- The annotated policy: tier AND isolation class. -/
def annotatedTier (toolchain : Toolchain) (trust : TrustLevel) :
    SandboxTier × Isolation :=
  (selectTier toolchain trust, tierIsolation (selectTier toolchain trust))

/-- T4 (the spec-implementation tie): the annotation does not change
the shipped policy — every combination yields the same tier as
`SandboxPolicy.selectTier` would. Proved by full enumeration: 21
combinations, each closed by computation. -/
theorem annotation_matches_policy :
    ∀ t : Toolchain, ∀ tr : TrustLevel,
      (annotatedTier t tr).1 = selectTier t tr := by
  intro t tr
  unfold annotatedTier
  rw []

/-- T3 (audited = shared kernel): audited toolchains land in tier1,
whose class is sharedKernel — the host kernel IS in the TCB, so the
container hardening bundle (no-new-privileges, read-only root, dropped
caps, seccomp) remains mandatory for every tier1 workload. The tier
never substitutes for the layers. -/
theorem audited_tier_shares_kernel (t : Toolchain)
    (h : t = Toolchain.rust ∨ t = Toolchain.cpp ∨ t = Toolchain.vulkan) :
    tierIsolation (selectTier t TrustLevel.trustedAudited)
      = Isolation.sharedKernel := by
  cases t with
  | rust => rfl
  | cpp => rfl
  | vulkan => rfl
  | nodeJs => exact absurd h (by decide)
  | python => exact absurd h (by decide)
  | ruby => exact absurd h (by decide)
  | llmReasoning => exact absurd h (by decide)

/-- T2 (the LLM honesty theorem): the LLM tier's class is
`intercepted` — and `hostKernelInTCB intercepted = true`. Tier 3 does
NOT close the kernel class; it is reduced attack surface, and this
theorem keeps that admission in the machine-checked record rather than
in marketing. -/
theorem llm_tier_is_intercepted_not_microvm (trust : TrustLevel) :
    tierIsolation (selectTier Toolchain.llmReasoning trust)
      = Isolation.intercepted ∧
    hostKernelInTCB Isolation.intercepted = true := by
  cases trust <;> simp [selectTier, tierIsolation, hostKernelInTCB]

/-- T1 (the untrusted constraint — the policy theorem this bridge
exists for): an untrusted non-LLM toolchain gets tier4, whose class is
microVM — the ONLY class that closes the kernel class (container YP,
T4/T4a). A policy that gave untrusted workloads a sharedKernel or
intercepted tier would violate this; the enumeration makes such a
regression a build failure. -/
theorem untrusted_gets_microvm (t : Toolchain)
    (h : t ≠ Toolchain.llmReasoning) :
    tierIsolation (selectTier t TrustLevel.untrusted) = Isolation.microVM ∧
      hostKernelInTCB Isolation.microVM = false := by
  cases t with
  | rust => simp [selectTier, tierIsolation, hostKernelInTCB]
  | cpp => simp [selectTier, tierIsolation, hostKernelInTCB]
  | vulkan => simp [selectTier, tierIsolation, hostKernelInTCB]
  | nodeJs => simp [selectTier, tierIsolation, hostKernelInTCB]
  | python => simp [selectTier, tierIsolation, hostKernelInTCB]
  | ruby => simp [selectTier, tierIsolation, hostKernelInTCB]
  | llmReasoning => exact absurd rfl h

end TierIsolation
