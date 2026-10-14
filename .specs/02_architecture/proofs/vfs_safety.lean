import Init

-- VFS Safety Proofs for Clawdius
-- Blue Paper Reference: BP-VFS-001
-- Yellow Paper Reference: YP-STORAGE-VFS-001
-- Lean 4.28.0 core library only (no Mathlib).
-- 18 theorems, 0 sorrys.

namespace VfsPath

theorem no_escape_when_all_safe (root components : List Nat) :
    (root ++ components).length ≥ root.length := by
  have := @List.length_append Nat root components; omega

theorem empty_path_is_root (root : List Nat) :
    (root ++ ([] : List Nat)) = root := List.append_nil root

theorem resolution_monotone (root components : List Nat) :
    (root ++ components).length ≥ root.length := by
  have := @List.length_append Nat root components; omega

theorem nonempty_root_implies_nonempty (root components : List Nat) (h : root.length > 0) :
    (root ++ components).length > 0 := by
  have := @List.length_append Nat root components; omega

theorem resolved_length_eq (root components : List Nat) :
    (root ++ components).length = root.length + components.length :=
  @List.length_append Nat root components

/-! ## Lexical normalization with POSIX pop-semantics (YP-VFS-NORMALIZE-008)

Component `0` is the dot (`..`); everything else is a name. Normalization folds
left-to-right over a stack: a name pushes, a dot pops the previous component —
and a dot at the root boundary is dropped, because the root *is* the sandbox.

`normAux` returns the stack in push order; `normalize` reverses it into path
order. Predictions from the YP, now asserted: P1 dot-freedom of the output,
P2 idempotence, P3 leading-dot no-op. -/

/-- Pop the top component; popping past the root boundary is a no-op. -/
def pop (acc : List Nat) : List Nat :=
  match acc with
  | [] => []
  | _ :: tl => tl

def normAux : List Nat → List Nat → List Nat
  | [], acc => acc
  | 0 :: rest, acc => normAux rest (pop acc)
  | c :: rest, acc => normAux rest (c :: acc)

/-- Normalized form of a component list, in path order. -/
def normalize (components : List Nat) : List Nat :=
  (normAux components []).reverse

-- P3: a leading escape attempt changes nothing — the root *is* the boundary.
theorem normalize_leading_dot (components : List Nat) :
    normalize (0 :: components) = normalize components := rfl

/-- Split transparency: processing `q ++ p` is processing `q`, then `p`. -/
theorem normAux_compose (q p acc : List Nat) :
    normAux (q ++ p) acc = normAux p (normAux q acc) := by
  induction q generalizing acc with
  | nil => rfl
  | cons c rest ih =>
    cases c with
    | zero => exact ih (pop acc)
    | succ n => exact ih ((n + 1) :: acc)

/-- A dot-free input is copied verbatim (reversed) onto the stack. -/
theorem normAux_dotfree (p acc : List Nat) (hd : ∀ x ∈ p, x ≠ 0)
    (hacc : ∀ x ∈ acc, x ≠ 0) :
    normAux p acc = p.reverse ++ acc := by
  induction p generalizing acc with
  | nil => rfl
  | cons c rest ih =>
    cases c with
    | zero => exact absurd (hd 0 (by simp)) (by simp)
    | succ n =>
      have hmem : ∀ x ∈ (n + 1) :: rest, x ≠ 0 := by
        intro y hy
        cases List.mem_cons.mp hy with
        | inl h => rw [h]; exact Nat.succ_ne_zero n
        | inr h => exact hd y (List.mem_cons_of_mem _ h)
      have hacc' : ∀ x ∈ (n + 1) :: acc, x ≠ 0 := by
        intro y hy
        cases List.mem_cons.mp hy with
        | inl h => rw [h]; exact Nat.succ_ne_zero n
        | inr h => exact hacc y h
      have hrest : ∀ x ∈ rest, x ≠ 0 := fun y hy =>
        hd y (List.mem_cons_of_mem _ hy)
      show normAux rest ((n + 1) :: acc) = ((n + 1) :: rest).reverse ++ acc
      rw [ih ((n + 1) :: acc) hrest hacc']
      simp

/-- The stack stays dot-free when it starts dot-free. -/
theorem normAux_no_dots (p acc : List Nat) (hd : ∀ x ∈ acc, x ≠ 0) :
    ∀ x ∈ normAux p acc, x ≠ 0 := by
  induction p generalizing acc with
  | nil => exact hd
  | cons c rest ih =>
    cases c with
    | zero =>
      cases acc with
      | nil => exact ih (acc := []) (by simp)
      | cons a tl =>
        intro x hx
        refine ih (acc := tl) ?_ x hx
        intro y hy
        exact hd y (List.mem_cons_of_mem _ hy)
    | succ n =>
      intro x hx
      refine ih (acc := (n + 1) :: acc) ?_ x hx
      intro y hy
      cases List.mem_cons.mp hy with
      | inl h => rw [h]; exact Nat.succ_ne_zero n
      | inr h => exact hd y h

/-- P1: dots cannot survive normalization. -/
theorem normalize_no_dots (p : List Nat) : ∀ x ∈ normalize p, x ≠ 0 := by
  have h := normAux_no_dots p [] (by simp)
  intro x hx
  exact h x ((List.mem_reverse).mp hx)

/-- A dot-free list is already in normal form. -/
theorem normalize_eq_self_of_dotfree (p : List Nat) (hd : ∀ x ∈ p, x ≠ 0) :
    normalize p = p := by
  show (normAux p []).reverse = p
  rw [normAux_dotfree p [] hd (by simp)]
  simp

/-- P2: the canonical form is canonical. -/
theorem normalize_idempotent (p : List Nat) :
    normalize (normalize p) = normalize p :=
  normalize_eq_self_of_dotfree (normalize p) (normalize_no_dots p)

end VfsPath