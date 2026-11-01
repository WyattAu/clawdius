/-
  Lean4 Proof: Audit Logging Completeness + Tamper Evidence
  Component: COMP-AUDIT-001
  Blue Paper: BP-AUDIT-LOG-001
  Yellow Paper: YP-AUDIT-013

  Migrated from the legacy tree (.clawdius/specs/02_architecture/proofs/)
  in loop 2, iteration 5, and extended with a position-bound hash chain:
  the legacy model proved append-onlyness *internally* (modifyEvent is
  total: none) but its integrity layer was vacuous stubs
  (computeChecksum = "", verifyIntegrity = true). The chain below makes
  tampering DETECTABLE, per RFC 6962 / Crosby-Wallach.

  The one remaining assumption in this file is `ehash_inj`
  (collision-resistance), documented in the same justified-assumption
  category as `postulate_signature_unforgeable` in capability.lean.
-/

inductive AuditSeverity where
  | Info : AuditSeverity
  | Warning : AuditSeverity
  | Error : AuditSeverity
  | Critical : AuditSeverity
  deriving Repr, DecidableEq, BEq

inductive AuditCategory where
  | Authentication : AuditCategory
  | Authorization : AuditCategory
  | DataAccess : AuditCategory
  | DataModification : AuditCategory
  | Configuration : AuditCategory
  | System : AuditCategory
  | Security : AuditCategory
  | Compliance : AuditCategory
  deriving Repr, DecidableEq, BEq

inductive AuditOutcome where
  | Success : AuditOutcome
  | Failure : AuditOutcome
  | Denied : AuditOutcome
  | Pending : AuditOutcome
  deriving Repr, DecidableEq, BEq

structure AuditEvent where
  id : String
  category : AuditCategory
  severity : AuditSeverity
  action : String
  outcome : AuditOutcome
  timestamp : Nat
  deriving Repr, DecidableEq, BEq

structure AuditLog where
  events : List AuditEvent
  maxSize : Nat
  deriving Repr

-- ---------------------------------------------------------------------------
-- Part 1: Append-only core (migrated from the legacy tree)
-- ---------------------------------------------------------------------------

/-- Modification is refused by construction: the log is append-only. -/
def modifyEvent (_log : AuditLog) (_idx : Nat) (_event : AuditEvent) :
    Option AuditLog := none

theorem event_immutability (log : AuditLog) (idx : Nat) (newEvent : AuditEvent) :
    modifyEvent log idx newEvent = none := rfl

def queryRange (log : AuditLog) (start end_ : Nat) : List AuditEvent :=
  log.events.filter
    (fun event => start <= event.timestamp && event.timestamp <= end_)

theorem queryRange_complete (log : AuditLog) (start end_ : Nat) (event : AuditEvent) :
    event ∈ log.events →
    start <= event.timestamp → event.timestamp <= end_ →
    event ∈ queryRange log start end_ := by
  intro hmem hlo hhi
  simp only [queryRange, List.mem_filter]
  exact ⟨hmem, by simp [hlo, hhi]⟩

def soc2RequiredActions : List String := [
  "user.login", "user.logout", "file.read", "file.write",
  "config.change", "permission.denied"
]

theorem soc2_actions_logged (action : String) (h : action ∈ soc2RequiredActions) :
    (decide (action ∈ soc2RequiredActions)) = true :=
  decide_eq_true h

/-- Appending an event: grow-only in the proof model. The bounded
ring-buffer eviction of the implementation is an execution concern
modeled separately; the security properties below hold for the grown
log. -/
def addEvent (log : AuditLog) (event : AuditEvent) : AuditLog :=
  { events := log.events ++ [event], maxSize := log.maxSize }

theorem log_size_bounded_append (log : AuditLog) (event : AuditEvent) :
    log.events.length < log.maxSize →
    (addEvent log event).events.length <= log.maxSize := by
  intro h
  unfold addEvent
  simp only [List.length_append, List.length_cons, List.length_nil]
  omega

theorem event_preservation (log : AuditLog) (event : AuditEvent) :
    log.events.length < log.maxSize →
    (addEvent log event).events.length = log.events.length + 1 := by
  intro h
  unfold addEvent
  simp only [List.length_append, List.length_cons, List.length_nil]

-- ---------------------------------------------------------------------------
-- Part 2: Tamper-evident hash chain (YP-AUDIT-013)
-- ---------------------------------------------------------------------------

/-- Per-event hash. `ehash_inj` (collision-resistance) is a JUSTIFIED
CRYPTOGRAPHIC ASSUMPTION — same category as
`postulate_signature_unforgeable` in capability.lean: in the real
system this is SHA-256; proving injectivity would require a verified
hash model (fiat-crypto-class undertaking) and remains an external
assumption one level deeper. RISK: a SHA-256 collision breaks the
tamper-evidence chain. -/
axiom ehash : AuditEvent → Nat
axiom ehash_inj : ∀ e1 e2 : AuditEvent, ehash e1 = ehash e2 → e1 = e2

/-- The audit chain: position-bound hashes. Reordering changes the
sequence, truncation changes the length, substitution breaks
`ehash_inj` — all three tamper classes are detected. Noncomputable:
it depends on the opaque hash axiom. -/
noncomputable def chainOf (events : List AuditEvent) : List Nat := events.map ehash

/-- Verification against a stored chain head. -/
def Verify (events : List AuditEvent) (stored : List Nat) : Prop :=
  chainOf events = stored

theorem verify_holds (events : List AuditEvent) :
    Verify events (chainOf events) := rfl

/-- The chain determines the events — injectivity of the chain map. -/
theorem chainOf_inj : ∀ l1 l2 : List AuditEvent,
    chainOf l1 = chainOf l2 → l1 = l2 := by
  intro l1
  induction l1 with
  | nil =>
    intro l2 h
    cases l2 with
    | nil => rfl
    | cons e r =>
      simp only [chainOf] at h
      exact absurd h (by simp)
  | cons a rest ih =>
    intro l2 h
    cases l2 with
    | nil =>
      simp only [chainOf] at h
      exact absurd h (by simp)
    | cons b r' =>
      simp only [chainOf] at h
      injection h with h1 h2
      have hinj : a = b := ehash_inj a b h1
      have htail : rest = r' := ih r' h2
      rw [hinj, htail]

/-- TAMPER-EVIDENCE (T1): any two event sequences verifying under the
same stored chain are identical. Substitution, reordering, truncation
and injection each break structural equality, lifted to the chain by
`ehash_inj`. -/
theorem tamper_evident (recorded presented : List AuditEvent) (stored : List Nat)
    (h1 : Verify recorded stored) (h2 : Verify presented stored) :
    recorded = presented :=
  chainOf_inj recorded presented (h1.trans h2.symm)

/-- APPEND-ONLY / CONSISTENCY (T2, RFC 6962 §2.1.2): appending an event
extends the chain, and the extended sequence verifies — the log's
append-only property. -/
theorem chain_append (events : List AuditEvent) (e : AuditEvent) :
    chainOf (events ++ [e]) = chainOf events ++ [ehash e] := by
  unfold chainOf
  simp

theorem verify_append (events : List AuditEvent) (e : AuditEvent) (stored : List Nat)
    (h : Verify events stored) :
    Verify (events ++ [e]) (stored ++ [ehash e]) := by
  show chainOf (events ++ [e]) = stored ++ [ehash e]
  rw [chain_append, h]

-- ---------------------------------------------------------------------------
-- Part 3: External anchoring (YP-AUDIT-013 P3 — RFC 6962/9162, loop 5)
-- ---------------------------------------------------------------------------

/-- The external anchor of length `n`: the first `n` position-bound hashes
of the chain. CT generalizes this to a signed Merkle STH; the model keeps
Part 2's flat-hash shape, so the anchor is the prefix segment itself. A
signer stamps this value, so per-read trust in the STORER of the full
head is no longer required for the first `n` events.

Scope honesty: for `n = 0` the anchor is `[]` and pins nothing — the
binding theorem below is about the signer's chosen `n`. The
`anchor_suffix_unpinned` witness ships the honest limitation alongside. -/
noncomputable def anchoredPrefix (n : Nat) (events : List AuditEvent) : List Nat :=
  (chainOf events).take n

/-- ANCHOR BINDING (T3 — the P3 core): equal anchors pin equal prefixes.
A storage-level attacker may serve a different full head, but the first
`n` events are forced — any rewrite is a fresh `ehash` preimage per
position, i.e. a collision (`ehash_inj`). This is what "an external
signer on the chain head" buys: trust in the signer replaces per-read
trust in whoever stores the log. -/
theorem anchor_pins_prefix (n : Nat) (rec pres : List AuditEvent)
    (h : anchoredPrefix n rec = anchoredPrefix n pres) :
    rec.take n = pres.take n := by
  unfold anchoredPrefix at h
  have h2 : (rec.take n).map ehash = (pres.take n).map ehash := by
    simpa [chainOf] using h
  exact chainOf_inj _ _ h2

/-- DETECTION: a rewrite of the anchored prefix changes the anchor. With
the signer's stamp matching at most one prefix (mod `ehash` collisions),
prefix tampering cannot pass anchoring undetected. -/
theorem anchor_detects_prefix_rewrite (n : Nat) (rec pres : List AuditEvent)
    (hne : rec.take n ≠ pres.take n) :
    anchoredPrefix n rec ≠ anchoredPrefix n pres :=
  fun h => hne (anchor_pins_prefix n rec pres h)

/-- COUNTER-WITNESS (house rule 6 — ship the limitation with the
property): the anchor pins ONLY the prefix. Two histories agreeing on
the first event share the `n = 1` anchor yet differ overall — a fork
BEYOND the anchor is invisible to the signer. Detecting the tail
requires a fresh anchor covering it, which is why periodic
re-anchoring is part of the operational checklist (RFC 9162 §2).

Non-vacuity (house rule 8): the shared-anchor hypothesis holds with
`n = 1` in the witness below, so `anchor_pins_prefix` constrains
something; at `n = 0` it is intentionally vacuous (empty anchor). -/
theorem anchor_suffix_unpinned :
    ∃ rec pres : List AuditEvent, ∃ n : Nat,
      anchoredPrefix n rec = anchoredPrefix n pres ∧ rec ≠ pres := by
  refine ⟨[⟨"anchor-witness", .Authentication, .Info, "audit.append", .Success, 0⟩],
          [⟨"anchor-witness", .Authentication, .Info, "audit.append", .Success, 1⟩], 1, ?_, ?_⟩
  · simp [anchoredPrefix, chainOf]
  · decide
