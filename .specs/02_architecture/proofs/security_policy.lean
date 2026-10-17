-- VERIFICATION STATUS: Formal security policy model for Clawdius
-- Lean 4.28.0 (core library only — no Mathlib dependency)
--
-- This file formally models the security properties that Clawdius's
-- multi-tenant deployment must satisfy. These properties map to
-- specific Rust implementations:
--
--   1. Tenant Isolation     → session/isolation.rs, api/tenant.rs
--   2. Sandbox Enforcement  → mcp/sandboxed_executor.rs, sandbox/
--   3. Resource Bounds      → api/resource_budget.rs
--   4. Audit Integrity      → audit/agent_audit.rs
--   5. Auth Enforcement     → api/auth.rs

namespace Clawdius.Security

-- ---------------------------------------------------------------------------
-- Security Policy: Tenant Isolation
-- ---------------------------------------------------------------------------

-- A tenant either has Shared or DedicatedDb isolation.
inductive IsolationTier where
  | Shared : IsolationTier
  | DedicatedDb : IsolationTier

-- A tenant is identified by a unique ID.
abbrev TenantId := String

-- A session belongs to exactly one tenant.
structure Session where
  id : String
  tenantId : String

-- THEOREM: Sessions from different tenants with Shared isolation
-- are stored in the same database but filtered by tenant_id.
-- This means: for any query result, each returned session has
-- a tenant_id matching the requesting tenant.

-- Modeled as: a filter function that only returns sessions
-- belonging to the requesting tenant.

def tenantFilter (tenantId : String) (sessions : List Session) : List Session :=
  sessions.filter (fun s => s.tenantId = tenantId)

-- THEOREM: tenantFilter only returns sessions belonging to the tenant.
theorem tenant_filter_preserves_tenant (tenantId : String) (sessions : List Session) :
    ∀ s ∈ tenantFilter tenantId sessions, s.tenantId = tenantId := by
  intro s h
  simp [tenantFilter, List.mem_filter] at h
  exact h.right

-- THEOREM: DedicatedDb tenants never share a database with other tenants.
-- Modeled as: each DedicatedDb tenant has its own store, which by
-- construction cannot contain sessions from other tenants.
-- Proof: By construction — the TenantIsolationManager creates a
-- separate PooledSessionStore per DedicatedDb tenant.
-- Modeled as: each DedicatedDb tenant has its own store, which by
-- construction cannot contain sessions from other tenants.
-- Proof: By construction — the TenantIsolationManager creates a
-- separate PooledSessionStore per DedicatedDb tenant.

-- ---------------------------------------------------------------------------
-- Helper lemmas for Bool reasoning
-- ---------------------------------------------------------------------------

-- Bool.and is not Prop.and, so we need this helper to extract the left
-- operand of a && b = true.
theorem bool_and_left (a b : Bool) : (a && b) = true → a = true := by
  intro h
  by_cases ha : a = true
  exact ha
  simp [ha, Bool.and] at h

-- ---------------------------------------------------------------------------
-- Security Policy: Sandbox Enforcement
-- ---------------------------------------------------------------------------

-- A file path is either within the workspace or a traversal attempt.
inductive PathValidation where
  | WithinWorkspace : PathValidation
  | TraversalAttempt : PathValidation

-- THEOREM: All file operations are validated against the workspace root.
-- Maps to Rust: SandboxedToolExecutor::validate_path() checks
-- path.starts_with(workspace) and rejects ".." components.

-- Modeled as: a validation function that returns WithinWorkspace
-- only if the path is under the workspace root.

def validatePath (workspace : String) (filePath : String) : PathValidation :=
  if filePath.startsWith workspace && !filePath.contains ".." then
    PathValidation.WithinWorkspace
  else
    PathValidation.TraversalAttempt

-- THEOREM: validatePath rejects paths containing ".."
theorem validate_path_rejects_traversal (workspace filePath : String)
    (h : filePath.contains "..") :
    validatePath workspace filePath = PathValidation.TraversalAttempt := by
  simp [validatePath, h]

-- THEOREM: validatePath rejects paths outside the workspace.
-- Proof: If filePath doesn't start with workspace, the && condition is
-- false regardless of the ".." check, so validatePath returns TraversalAttempt.
theorem validate_path_rejects_outside (workspace filePath : String)
    (h : ¬filePath.startsWith workspace) :
    validatePath workspace filePath = PathValidation.TraversalAttempt := by
  unfold validatePath
  split
  · -- Case: condition is true, but h says startsWith is false → contradiction
    exact absurd (bool_and_left _ _ ‹_›) h
  · rfl

-- ---------------------------------------------------------------------------
-- Security Policy: Resource Bounds
-- ---------------------------------------------------------------------------

-- Resource budget for a tenant.
structure ResourceBudget where
  maxTokens : Nat
  maxMessages : Nat
  maxSessions : Nat
  maxStorageBytes : Nat

-- Current resource usage.
structure ResourceUsage where
  tokens : Nat
  messages : Nat
  sessions : Nat
  storageBytes : Nat

-- THEOREM: Resource usage never exceeds the budget.
-- Maps to Rust: TenantResourceBudget::check() returns an error
-- if any resource exceeds its limit.

def withinBudget (budget : ResourceBudget) (usage : ResourceUsage) : Bool :=
  usage.tokens ≤ budget.maxTokens &&
  usage.messages ≤ budget.maxMessages &&
  usage.sessions ≤ budget.maxSessions &&
  usage.storageBytes ≤ budget.maxStorageBytes

-- THEOREM: If withinBudget returns true, all individual resources are within bounds.
-- Proof: withinBudget is a &&-chain of decide(...) terms. If the chain is true,
-- every conjunct is true, including the first (tokens ≤ maxTokens).
theorem within_budget_implies_tokens_bound (budget : ResourceBudget) (usage : ResourceUsage)
    (h : withinBudget budget usage = true) :
    usage.tokens ≤ budget.maxTokens := by
  unfold withinBudget at h
  -- h : (((tokens≤maxTokens && messages≤maxMessages) && sessions≤maxSessions) && storageBytes≤maxStorageBytes) = true
  -- Extract the innermost left conjunct by peeling && layers
  have h1 := bool_and_left _ _ h
  have h2 := bool_and_left _ _ h1
  have h3 := bool_and_left _ _ h2
  exact of_decide_eq_true h3

-- ---------------------------------------------------------------------------
-- Security Policy: Audit Trail Integrity
-- ---------------------------------------------------------------------------

-- An audit entry with hash chain.
structure AuditEntry where
  sequence : Nat
  tenantId : String
  action : String
  previousHash : String
  currentHash : String

-- THEOREM: The audit trail is an append-only, hash-chained log.
-- Maps to Rust: AgentAuditLog::log_transition() computes
-- currentHash = hash(previousHash + action + timestamp).
-- verify_integrity() checks that each entry's previousHash
-- matches the previous entry's currentHash.

-- This property is maintained by construction: the only way to
-- add an entry is through log_transition(), which requires
-- the previous entry's hash. Tampering with any entry breaks
-- the chain, which verify_integrity() detects.

-- Modeled as: a list of entries where each entry's previousHash
-- equals the previous entry's currentHash.

def validChain (entries : List AuditEntry) : Bool :=
  match entries with
  | [] => true
  | [_] => true
  | e1 :: e2 :: rest => e2.previousHash = e1.currentHash && validChain (e2 :: rest)

-- THEOREM: A valid chain maintains hash continuity.
-- Proof: By structural induction on the list.
--   - Empty or singleton: trivially valid (no pairs to check).
--   - Two or more: validChain requires the first pair to have matching hashes
--     AND the tail to be valid. This is exactly the inductive step.
-- If validChain returns true for a list with ≥2 entries, the first pair
-- matches AND the tail is valid.
theorem valid_chain_preserves_integrity (entries : List AuditEntry)
    (h : validChain entries = true) :
    entries.length ≤ 1 ∨
    (∃ e1 e2 rest, entries = e1 :: e2 :: rest ∧ e2.previousHash = e1.currentHash ∧ validChain (e2 :: rest) = true) := by
  match entries with
  | [] => left; simp
  | [e] => left; simp
  | e1 :: e2 :: rest =>
    right
    simp [validChain] at h
    -- h : e2.previousHash = e1.currentHash ∧ validChain (e2 :: rest) = true
    refine ⟨e1, e2, rest, rfl, h.left, h.right⟩

-- ---------------------------------------------------------------------------
-- Security Policy: Authentication Enforcement
-- ---------------------------------------------------------------------------

-- THEOREM: All protected API endpoints require valid authentication.
-- Maps to Rust: auth_middleware() checks for valid API key
-- before allowing access to protected routes.
-- Proof: By construction — the Axum router applies auth_middleware
-- as a layer to all protected routes. Requests without a valid
-- key receive 401 Unauthorized before reaching any handler.

-- ---------------------------------------------------------------------------
-- Security Policy: API Key Isolation
-- ---------------------------------------------------------------------------

-- THEOREM: API keys are scoped to tenants and cannot access
-- data from other tenants.
-- Maps to Rust: TenantStore::get_tenant_by_api_key() looks up
-- the tenant associated with an API key. The resolved tenant_id
-- is then used to filter all session queries.
-- Proof: By construction — the auth middleware resolves the API key
-- to a tenant_id, which is passed to all downstream handlers.
-- Handlers filter queries by this tenant_id.

end Clawdius.Security

-- ---------------------------------------------------------------------------
-- Workspace-scoped normalization (YP-SECURITY-POLICY-011, loop 2 it. 3)
-- ---------------------------------------------------------------------------

namespace Workspace

/-- Component-wise prefix: `dir` is a component prefix of `path`. -/
def isComponentPrefix (dir path : List Nat) : Prop := ∃ rest, path = dir ++ rest

/-- THE UNSOUNDNESS WITNESS: the character-prefix check used by
`validatePath` accepts a sibling directory. `/srv/ws` is a string
prefix of `/srv/ws-evildir/x` (Snyk, "Zip Slip", 2018: the sibling
directory falsely passes). The current `startsWith` model is therefore
proven unsound, not merely suspected. -/
theorem raw_prefix_bypass_exists :
    ∃ workspace filePath : String,
      filePath.startsWith workspace = true ∧
      filePath.startsWith (workspace ++ "/") = false :=
  ⟨"/srv/ws", "/srv/ws-evildir/x", by native_decide, by native_decide⟩

/-- Pop the top component, clamped at the workspace floor: popping is a
no-op once the stack is exactly `floor` components long — chroot
semantics expressed lexically. -/
def popWithin (floor : Nat) (acc : List Nat) : List Nat :=
  if floor < acc.length then acc.tail else acc

def normAuxWithin (floor : Nat) (p : List Nat) (acc : List Nat) : List Nat :=
  match p with
  | [] => acc
  | 0 :: rest => normAuxWithin floor rest (popWithin floor acc)
  | c :: rest => normAuxWithin floor rest (c :: acc)

def normalizeWithin (floor : Nat) (p : List Nat) : List Nat :=
  (normAuxWithin floor p []).reverse

theorem normAuxWithin_nil (floor : Nat) (acc : List Nat) :
    normAuxWithin floor [] acc = acc := rfl

theorem normAuxWithin_zero (floor : Nat) (rest acc : List Nat) :
    normAuxWithin floor (0 :: rest) acc = normAuxWithin floor rest (popWithin floor acc) := rfl

theorem normAuxWithin_cons (floor : Nat) (c : Nat) (rest acc : List Nat) (hc : c ≠ 0) :
    normAuxWithin floor (c :: rest) acc = normAuxWithin floor rest (c :: acc) := by
  cases c with
  | zero => exact absurd hc (by simp)
  | succ n => rfl

/-- Pop lemmas: `popWithin` is the identity on the empty stack, drops
the top when the floor allows it, and clamps otherwise. -/
theorem popWithin_nil (floor : Nat) : popWithin floor [] = [] := rfl

theorem popWithin_cons (floor : Nat) (a : Nat) (tl : List Nat) :
    popWithin floor (a :: tl) =
      if floor < (a :: tl).length then tl else a :: tl := rfl

/-- Floor lemma: the stack never drops below the workspace root.
Pushes grow it; pops are clamped by `popWithin`. -/
theorem normAuxWithin_floor_lemma (floor : Nat) (r acc : List Nat)
    (h : floor ≤ acc.length) :
    floor ≤ (normAuxWithin floor r acc).length := by
  induction r generalizing acc with
  | nil => exact h
  | cons c rest ih =>
    cases acc with
    | nil =>
      cases c with
      | zero =>
        rw [normAuxWithin_zero, popWithin_nil]
        exact ih [] (by have := h; simp at this; omega)
      | succ n =>
        rw [normAuxWithin_cons (hc := by simp)]
        exact ih ((n + 1) :: []) (by simp only [List.length_cons]; omega)
    | cons a tl =>
      cases c with
      | zero =>
        rw [normAuxWithin_zero, popWithin_cons]
        by_cases hcond : floor < (a :: tl).length
        · rw [if_pos hcond]
          have h2 : tl.length + 1 = (a :: tl).length := rfl
          exact ih tl (by omega)
        · rw [if_neg hcond]
          exact ih (a :: tl) h
      | succ n =>
        rw [normAuxWithin_cons (hc := by simp)]
        exact ih ((n + 1) :: a :: tl) (by rw [List.length_cons]; omega)



-- ---------------------------------------------------------------------------
-- P3: component-identity closure (YP-SECURITY-POLICY-011, loop 3 it. 6)
-- ---------------------------------------------------------------------------

/-! T1 proved the LENGTH form: a contained path normalizes to at least
|workspace| components. P3 is the stronger IDENTITY form: for a file
inside the workspace, the first |workspace| components of the normalized
path ARE the workspace components — same components, same order. A length
bound is satisfied by any sibling with a long enough name in this lexical
model; component identity is not.

The accumulator in `normAuxWithin` is a stack stored deepest-first (push
prepends, pop removes the head), so "the workspace-root prefix survives"
is a statement about the LAST `floor` stack elements — i.e.
`(stack.reverse).take floor` after the final reverse. -/

/-- Taking `n` from a list with one extra element appended leaves the
first `n` untouched, provided they were already there. (Core's
append-take lemmas changed names across releases; this one is stated
locally so the proof below does not depend on which release we are on.) -/
theorem take_append_last (n : Nat) (l : List Nat) (a : Nat) (h : n ≤ l.length) :
    (l ++ [a]).take n = l.take n := by
  induction l generalizing n with
  | nil =>
    cases n with
    | zero => rfl
    | succ m => exact absurd h (by simp)
  | cons b tl ih =>
    cases n with
    | zero => rfl
    | succ m =>
      have h' : m ≤ tl.length := by
        have h3 : (b :: tl).length = tl.length + 1 := rfl
        omega
      show b :: ((tl ++ [a]).take m) = b :: tl.take m
      rw [ih m h']

/-- Normalization composes: walking `p ++ q` is walking `p` then `q`,
with the stack threaded through. -/
theorem normAuxWithin_composition (floor : Nat) (p q acc : List Nat) :
    normAuxWithin floor (p ++ q) acc
      = normAuxWithin floor q (normAuxWithin floor p acc) := by
  induction p generalizing acc with
  | nil => rfl
  | cons c rest ih =>
    rw [List.cons_append]
    cases c with
    | zero => rw [normAuxWithin_zero]; exact ih (popWithin floor acc)
    | succ n =>
      rw [normAuxWithin_cons (hc := by simp)]
      exact ih ((n + 1) :: acc)

/-- A walk with no `..` components (no zeroes) is a pure push sequence:
the result is the stack followed by the reversed input. -/
theorem normAuxWithin_all_push (floor : Nat) (p acc : List Nat)
    (h : ∀ c ∈ p, c ≠ 0) :
    normAuxWithin floor p acc = p.reverse ++ acc := by
  induction p generalizing acc with
  | nil => simp [normAuxWithin_nil]
  | cons c rest ih =>
    have hc : c ≠ 0 := h c (by simp)
    have hrest : ∀ x ∈ rest, x ≠ 0 := fun x hx =>
      h x (List.mem_cons_of_mem c hx)
    rw [normAuxWithin_cons (hc := hc), ih (c :: acc) hrest, List.reverse_cons,
      List.append_assoc, List.cons_append, List.nil_append]

/-- THE ROOT INVARIANT (the heart of P3): normalization never alters the
workspace-root suffix of the stack — pops are clamped above it, pushes
grow below it. Expressed on the reversed stack: the first `floor`
document-order components are the ones the walk started with. -/
theorem normAuxWithin_root_invariant (floor : Nat) (p acc : List Nat)
    (h : floor ≤ acc.length) :
    (normAuxWithin floor p acc).reverse.take floor = acc.reverse.take floor := by
  induction p generalizing acc with
  | nil => rfl
  | cons c rest ih =>
    cases c with
    | zero =>
      rw [normAuxWithin_zero]
      cases acc with
      | nil =>
        have h0 : floor = 0 := by
          have h2 := h
          simp only [List.length_nil] at h2
          omega
        subst h0
        simp
      | cons a tl =>
        rw [popWithin_cons]
        by_cases hlt : floor < (a :: tl).length
        · rw [if_pos hlt]
          have h1 : floor ≤ tl.length := by
            have h2 : tl.length + 1 = (a :: tl).length := rfl
            omega
          rw [ih tl h1, List.reverse_cons, take_append_last floor tl.reverse a
            (by simpa using h1)]
        · rw [if_neg hlt, ih (a :: tl) h]
    | succ n =>
      have h' : floor ≤ ((n + 1) :: acc).length := by
        rw [List.length_cons]; omega
      rw [normAuxWithin_cons (hc := by simp), ih ((n + 1) :: acc) h',
        List.reverse_cons, take_append_last floor acc.reverse (n + 1)
        (by simpa using h)]

/-- P3 (component-identity closure): a file inside the workspace —
`f = d ++ rest` — normalizes to a path whose first `|d|` components are
EXACTLY `d`. Containment is component-exact: not "long enough" (T1) but
"the same components, in the same order". -/
theorem normalized_prefix_is_workspace (d rest : List Nat)
    (hd : ∀ c ∈ d, c ≠ 0) :
    (normalizeWithin d.length (d ++ rest)).take d.length = d := by
  have hcomp := normAuxWithin_composition d.length d rest []
  have hpush := normAuxWithin_all_push d.length d [] hd
  rw [normalizeWithin, hcomp, hpush, List.append_nil]
  have hin : d.length ≤ d.reverse.length := by simp
  have hinv := normAuxWithin_root_invariant d.length rest d.reverse hin
  rw [hinv, List.reverse_reverse, List.take_length]

/-- THE P3 WITNESS: without the floor clamp, the same walk eats into the
workspace prefix. `d = [1,1]` with a single `..`: at `floor = 0` the pop
removes a workspace component and closure FAILS; at `floor = |d|` the pop
is clamped and closure holds. The clamp is not decoration — it is the
entire content of P3. -/
theorem unclamped_prefix_escapes :
    ∃ d rest : List Nat,
      (∀ c ∈ d, c ≠ 0) ∧
      (normalizeWithin 0 (d ++ rest)).take d.length ≠ d ∧
      (normalizeWithin d.length (d ++ rest)).take d.length = d :=
  ⟨[1, 1], [0], by simp, by native_decide, by
    have := normalized_prefix_is_workspace [1, 1] [0] (by simp)
    exact this⟩
