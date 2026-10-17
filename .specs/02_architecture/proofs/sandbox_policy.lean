/-
  Lean4 Proof: Sandbox Policy — Tier Selection and Secret Detection
  Component: COMP-SANDBOX-002
  Blue Paper: BP-SANDBOX-TIERS-001

  Migrated from the legacy tree (.clawdius/specs/02_architecture/proofs/)
  in loop 3, iteration 7, completing the legacy-migration defect.

  Provenance of this file's content:
  - Toolchain→tier policy + its two theorems: from legacy proof_sandbox.lean
    (selectTier, llm_gets_wasm_sandbox, untrusted_gets_hardened) — this was
    the half the legacy file had that NO canonical file carried.
  - Secret-key detection (isForbiddenKey, list_any_correctness,
    forbidden_key_disjunction/detected): likewise carried over verbatim.

  NOT carried over (superseded elsewhere):
  - Capability derivation (deriveCapability/attenuation) → capability.lean
    (derive/deriveGrant, attenuation_only_sound) — stronger form.
  - Fuel/limits semantics → sandbox_extended.lean (the sorry-free
    reconstruction; the legacy fuel theorems were vacuous without a
    construction of FuelState).
  - Ring-buffer masking → proof_ring_buffer_extended.lean (len-based
    representation; the legacy mod-based invariants had five theorems that
    were unprovable, including one that was FALSE — empty_iff_zero_len —
    under Nat truncating subtraction).

  The legacy SecurityInvariants record (four Prop fields, all `True`) is
  dropped: a record of True props asserts nothing; the invariants live in
  the YPs and the theorems that prove them.
-/

namespace SandboxPolicy

-- ---------------------------------------------------------------------------
-- Toolchain → sandbox tier policy
-- ---------------------------------------------------------------------------

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

/-- Policy: audited toolchains get tier 1; trusted-but-unaudited get
tier 2; LLM reasoning ALWAYS gets tier 3 (Wasm) regardless of claimed
trust — the model output is never a trusted toolchain; everything else
gets tier 4. -/
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

/-- The LLM gets the Wasm-tier sandbox no matter what trust level the
caller claims — trust escalation for reasoning output is structurally
impossible in the policy function. -/
theorem llm_gets_wasm_sandbox (trust : TrustLevel) :
    selectTier Toolchain.llmReasoning trust = SandboxTier.tier3 := by
  cases trust <;> rfl

/-- Untrusted toolchains never land in tiers 1 or 2. -/
theorem untrusted_gets_hardened (toolchain : Toolchain) :
    toolchain ≠ Toolchain.llmReasoning →
    selectTier toolchain TrustLevel.untrusted = SandboxTier.tier4 := by
  intro h
  cases toolchain with
  | rust => simp [selectTier]
  | cpp => simp [selectTier]
  | vulkan => simp [selectTier]
  | nodeJs => simp [selectTier]
  | python => simp [selectTier]
  | ruby => simp [selectTier]
  | llmReasoning => contradiction

-- ---------------------------------------------------------------------------
-- Secret detection
-- ---------------------------------------------------------------------------

/-- A key is forbidden when it names a credential class. -/
def isForbiddenKey (key : String) : Bool :=
  key == "_KEY" || key == "_SECRET" || key == "_TOKEN" ||
  key == "_PASSWORD" || key == "_CREDENTIAL"

/-- The generic list-any correctness lemma the detector rests on. -/
theorem list_any_correctness {α : Type} (f : α → Bool) (l : List α) :
    l.any f = true → ∃ x ∈ l, f x = true := by
  induction l with
  | nil => simp [List.any]
  | cons a l' ih =>
    simp only [List.any] at *
    intro h
    by_cases hfa : f a = true
    · exact ⟨a, by simp [List.mem_cons], hfa⟩
    · have hany : l'.any f = true := by
        have hf : f a = false := by
          by_cases hf2 : f a <;> simp_all
        have : (false || l'.any f) = true := by rw [hf] at h; exact h
        exact this
      have ⟨x, hx, hfx⟩ := ih hany
      exact ⟨x, List.mem_cons_of_mem a hx, hfx⟩

/-- Detection implies the key names one of the five credential classes —
the disjunction a redaction layer can dispatch on. -/
theorem forbidden_key_disjunction (key : String) :
    isForbiddenKey key = true →
    key == "_KEY" ∨ key == "_SECRET" ∨ key == "_TOKEN" ∨
    key == "_PASSWORD" ∨ key == "_CREDENTIAL" := by
  intro h
  unfold isForbiddenKey at h
  repeat rw [Bool.or_eq_true] at h
  match h with
  | Or.inl (Or.inl (Or.inl (Or.inl hk))) => exact Or.inl hk
  | Or.inl (Or.inl (Or.inl (Or.inr hs))) => exact Or.inr (Or.inl hs)
  | Or.inl (Or.inl (Or.inr ht)) => exact Or.inr (Or.inr (Or.inl ht))
  | Or.inl (Or.inr hp) => exact Or.inr (Or.inr (Or.inr (Or.inl hp)))
  | Or.inr hc => exact Or.inr (Or.inr (Or.inr (Or.inr hc)))

/-- Detection is sound: nothing is flagged unless it names a credential
class (the disjunction form is the detection form). -/
theorem forbidden_key_detected (key : String) :
    isForbiddenKey key = true →
    (key == "_KEY" ∨ key == "_SECRET" ∨ key == "_TOKEN" ∨
     key == "_PASSWORD" ∨ key == "_CREDENTIAL") :=
  forbidden_key_disjunction key

end SandboxPolicy
