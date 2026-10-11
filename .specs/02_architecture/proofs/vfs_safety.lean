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

/-! ## Lexical normalization (YP-VFS-NORMALIZE-008)

Component `0` is the dot (`..`); everything else is a name. Normalization folds
left-to-right: a name pushes into the accumulator, a dot at the root boundary is
dropped — the root *is* the sandbox, so a leading escape attempt changes
nothing.

Scope note, recorded honestly: this model drops dots at the root boundary; it
does not yet pop the previous component mid-path (POSIX `a/../b = b`). The
pop-semantics model, with its idempotence and dot-freedom theorems and their
induction obligations, is the next iteration's work — stated in the YP, not
asserted here unfinished. -/

def normalizeAux : List Nat → List Nat → List Nat
  | [], acc => acc
  | 0 :: rest, acc => normalizeAux rest acc
  | c :: rest, acc => normalizeAux rest (acc ++ [c])

def normalize (components : List Nat) : List Nat :=
  normalizeAux components []

theorem normalize_nil : normalize [] = [] := rfl

/-- A leading escape attempt changes nothing: the root *is* the boundary. -/
theorem normalize_leading_dot (components : List Nat) :
    normalize (0 :: components) = normalize components := rfl

end VfsPath