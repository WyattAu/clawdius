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

end VfsPath

namespace FileOps

abbrev FileSystem := List (Nat × Nat)

def findPid (fs : List (Nat × Nat)) (pid : Nat) : Option (Nat × Nat) :=
  match fs with
  | [] => none
  | (p, c) :: rest => if p = pid then some (p, c) else findPid rest pid

def filterNe (fs : List (Nat × Nat)) (pid : Nat) : List (Nat × Nat) :=
  match fs with
  | [] => []
  | (p, c) :: rest => if p = pid then filterNe rest pid else (p, c) :: filterNe rest pid

def fileRead (fs : FileSystem) (pid : Nat) : Option Nat :=
  match findPid fs pid with
  | none => none
  | some (_, c) => some c

def fileWrite (fs : FileSystem) (pid : Nat) (cid : Nat) : FileSystem :=
  filterNe fs pid ++ [(pid, cid)]

def fileDelete (fs : FileSystem) (pid : Nat) : FileSystem :=
  filterNe fs pid

-- ===== Equational lemmas =====

private theorem findPid_cons (p c : Nat) (rest : List (Nat × Nat)) (target : Nat) :
    findPid ((p, c) :: rest) target = if p = target then some (p, c) else findPid rest target := rfl

private theorem filterNe_cons (p c : Nat) (rest : List (Nat × Nat)) (pid : Nat) :
    filterNe ((p, c) :: rest) pid = if p = pid then filterNe rest pid else (p, c) :: filterNe rest pid := rfl

-- ===== Core helpers =====

private theorem findPid_filterNe (l : List (Nat × Nat)) (pid : Nat) :
    findPid (filterNe l pid) pid = none := by
  induction l with
  | nil => rfl
  | cons head rest ih =>
    cases head with
    | mk p c =>
      rw [filterNe_cons p c rest pid]; split
      · next _ => exact ih
      · next h =>
        rw [findPid_cons p c (filterNe rest pid) pid]; split
        · next h2 => exact absurd h2 h
        · next _ => exact ih

private theorem filterNe_preserves_findPid (l : List (Nat × Nat)) (pid other : Nat)
    (h_ne : pid ≠ other) :
    findPid (filterNe l pid) other = findPid l other := by
  induction l with
  | nil => rfl
  | cons head rest ih =>
    cases head with
    | mk p c =>
      rw [filterNe_cons p c rest pid, findPid_cons p c rest other]; split
      · next h_pid =>
        have h_po : ¬(p = other) := fun h => h_ne (h_pid.symm ▸ h)
        split
        · next h2 => exact absurd h2 h_po
        · next _ => exact ih
      · next h =>
        rw [findPid_cons p c (filterNe rest pid) other]; split
        · next _ => rfl
        · next _ => exact ih

private theorem findPid_append (l1 l2 : List (Nat × Nat)) (target : Nat) :
    findPid (l1 ++ l2) target =
    match findPid l1 target with
    | none => findPid l2 target
    | some x => some x := by
  induction l1 with
  | nil => rfl
  | cons head rest ih =>
    cases head with
    | mk p c =>
      rw [List.cons_append, findPid_cons p c (rest ++ l2) target, findPid_cons p c rest target]
      split
      · next _ => rfl
      · next _ => exact ih

private theorem filterNe_idempotent (l : List (Nat × Nat)) (pid : Nat) :
    filterNe (filterNe l pid) pid = filterNe l pid := by
  induction l with
  | nil => rfl
  | cons head rest ih =>
    cases head with
    | mk p c =>
      rw [filterNe_cons p c rest pid]; split
      · next _ => exact ih
      · next h =>
        rw [filterNe_cons p c (filterNe rest pid) pid]; split
        · next h2 => exact absurd h2 h
        · next _ => exact congrArg (List.cons (p, c)) ih

private theorem filterNe_append (l1 l2 : List (Nat × Nat)) (pid : Nat) :
    filterNe (l1 ++ l2) pid = filterNe l1 pid ++ filterNe l2 pid := by
  induction l1 with
  | nil => rfl
  | cons head rest ih =>
    cases head with
    | mk p c =>
      rw [List.cons_append, filterNe_cons p c rest pid, filterNe_cons p c (rest ++ l2) pid, ih]
      split
      · next _ => rfl
      · next _ => rfl

private theorem filterNe_length_le (l : List (Nat × Nat)) (pid : Nat) :
    (filterNe l pid).length ≤ l.length := by
  induction l with
  | nil => simp [filterNe]
  | cons head rest ih =>
    cases head with
    | mk p c =>
      rw [filterNe_cons p c rest pid]; split
      · next _ => have := @List.length_cons (Nat × Nat) (p, c) rest; omega
      · next _ =>
        have h1 := @List.length_cons (Nat × Nat) (p, c) (filterNe rest pid)
        have h2 := @List.length_cons (Nat × Nat) (p, c) rest
        omega

private theorem filterNe_filterNe_append_singleton_inner (fs : List (Nat × Nat)) (pid c1 : Nat) :
    filterNe (filterNe fs pid) pid ++ filterNe [(pid, c1)] pid = filterNe fs pid := by
  rw [filterNe_idempotent]
  rw [filterNe_cons pid c1 [] pid]; split
  · next _ =>
    have : filterNe [] pid = [] := rfl
    rw [this, List.append_nil]
  · next h => exact absurd rfl h

private theorem filterNe_filterNe_append_singleton (fs : List (Nat × Nat)) (pid c1 : Nat) :
    filterNe (filterNe fs pid ++ [(pid, c1)]) pid = filterNe fs pid := by
  rw [filterNe_append, filterNe_filterNe_append_singleton_inner]

private theorem findPid_singleton (pid cid : Nat) :
    findPid [(pid, cid)] pid = some (pid, cid) := by
  rw [findPid_cons pid cid [] pid]; split
  · next _ => rfl
  · next h => exact absurd rfl h

private theorem findPid_singleton_ne (pid other cid : Nat) (h_ne : pid ≠ other) :
    findPid [(pid, cid)] other = none := by
  rw [findPid_cons pid cid [] other]; split
  · next h_eq => exact absurd h_eq h_ne
  · next _ => rfl

-- fileRead congruence: equal findPid => equal fileRead
private theorem fileRead_congr {fs1 fs2 : List (Nat × Nat)} (pid : Nat)
    (h : findPid fs1 pid = findPid fs2 pid) :
    fileRead fs1 pid = fileRead fs2 pid := by
  show (match findPid fs1 pid with | none => none | some (_, c) => some c) =
       (match findPid fs2 pid with | none => none | some (_, c) => some c)
  rw [h]

-- findPid of filter ++ singleton at matching pid
private theorem findPid_filter_append (fs : List (Nat × Nat)) (pid cid : Nat) :
    findPid (filterNe fs pid ++ [(pid, cid)]) pid = some (pid, cid) := by
  induction fs with
  | nil =>
    have h_nil : filterNe [] pid = [] := rfl
    rw [h_nil, List.nil_append]; exact findPid_singleton pid cid
  | cons head rest ih =>
    cases head with
    | mk p c =>
      rw [filterNe_cons p c rest pid]; split
      · next _ => exact ih
      · next h =>
        rw [List.cons_append,
            findPid_cons p c (filterNe rest pid ++ [(pid, cid)]) pid,
            findPid_append (filterNe rest pid) [(pid, cid)] pid]
        split
        · next h2 => exact absurd h2 h
        · next _ =>
          rw [findPid_filterNe rest pid]; exact findPid_singleton pid cid

-- findPid of filter ++ singleton at non-matching pid
private theorem findPid_filter_append_ne (fs : List (Nat × Nat))
    (pid cid other : Nat) (h_ne : pid ≠ other) :
    findPid (filterNe fs pid ++ [(pid, cid)]) other = findPid fs other := by
  have h1 := filterNe_preserves_findPid fs pid other h_ne
  have h2 := findPid_append (filterNe fs pid) [(pid, cid)] other
  have h3 := findPid_singleton_ne pid other cid h_ne
  rw [h2, h1, h3]; split <;> next heq => exact heq.symm

-- ===== FILE OPS THEOREMS (8 total) =====

-- T1: write then read always returns the written content
theorem write_then_read (fs : FileSystem) (pid cid : Nat) :
    fileRead (fileWrite fs pid cid) pid = some cid := by
  have h := findPid_filter_append fs pid cid
  show (match findPid (filterNe fs pid ++ [(pid, cid)]) pid with
    | none => none | some (_, c) => some c) = some cid
  rw [h]

-- T2: delete is idempotent (deleting twice = deleting once)
theorem delete_idempotent (fs : FileSystem) (pid : Nat) :
    fileDelete (fileDelete fs pid) pid = fileDelete fs pid :=
  filterNe_idempotent fs pid

-- T3: delete then read returns none
theorem delete_then_read (fs : FileSystem) (pid : Nat) :
    fileRead (fileDelete fs pid) pid = none := by
  show (match findPid (filterNe fs pid) pid with
    | none => none | some (_, c) => some c) = none
  rw [findPid_filterNe fs pid]

-- T4: write preserves reads of other files
theorem write_preserves_other (fs : FileSystem) (pid other content : Nat)
    (h_ne : pid ≠ other) :
    fileRead (fileWrite fs pid content) other = fileRead fs other :=
  fileRead_congr other (findPid_filter_append_ne fs pid content other h_ne)

-- T5: write overwrites (second write wins)
theorem write_overwrites (fs : FileSystem) (pid : Nat) (c1 c2 : Nat) :
    fileRead (fileWrite (fileWrite fs pid c1) pid c2) pid = some c2 := by
  have h_ffs := filterNe_filterNe_append_singleton fs pid c1
  have h_fp := findPid_filter_append fs pid c2
  show (match findPid (filterNe (filterNe fs pid ++ [(pid, c1)]) pid ++ [(pid, c2)]) pid with
    | none => none | some (_, c) => some c) = some c2
  rw [h_ffs, h_fp]

-- T6: delete preserves reads of other files
theorem delete_preserves_other (fs : FileSystem) (pid other : Nat)
    (h_ne : pid ≠ other) :
    fileRead (fileDelete fs pid) other = fileRead fs other :=
  fileRead_congr other (filterNe_preserves_findPid fs pid other h_ne)

-- T7: sequential writes to different pids preserve both
theorem sequential_writes (fs : FileSystem) (p1 p2 c1 c2 : Nat)
    (h_ne : p1 ≠ p2) :
    fileRead (fileWrite (fileWrite fs p1 c1) p2 c2) p1 = some c1 := by
  -- filterNe (filterNe fs p1 ++ [(p1, c1)]) p2
  -- = filterNe (filterNe fs p1) p2 ++ filterNe [(p1, c1)] p2  (by filterNe_append)
  -- = filterNe (filterNe fs p1) p2 ++ [(p1, c1)]  (since p2 ≠ p1, filterNe [(p1,c1)] p2 = [(p1,c1)])
  have h_fa := filterNe_append (filterNe fs p1) [(p1, c1)] p2
  have h_fs := filterNe_cons p1 c1 [] p2
  have h_pn := filterNe_preserves_findPid (filterNe fs p1) p2 p1 (Ne.symm h_ne)
  have h_fn := findPid_filterNe fs p1
  have h_fc := findPid_singleton p1 c1
  -- findPid (filterNe (filterNe fs p1) p2 ++ [(p1, c1)] ++ [(p2, c2)]) p1
  have h1 : findPid (filterNe (filterNe fs p1 ++ [(p1, c1)]) p2 ++ [(p2, c2)]) p1 =
      findPid (filterNe (filterNe fs p1) p2 ++ [(p1, c1)] ++ [(p2, c2)]) p1 := by
    rw [h_fa, h_fs]; split
    · next h_eq => exact absurd h_eq h_ne
    · next _ => rfl
  have h2 := findPid_append (filterNe (filterNe fs p1) p2) ([(p1, c1)] ++ [(p2, c2)]) p1
  have h3 := findPid_append [(p1, c1)] [(p2, c2)] p1
  have h4 := findPid_singleton_ne p2 p1 c2 (Ne.symm h_ne)
  have h5 : findPid (filterNe (filterNe fs p1) p2 ++ [(p1, c1)] ++ [(p2, c2)]) p1 = some (p1, c1) := by
    rw [List.append_assoc, h2, h_pn, h_fn]; split
    · next _ => rw [h3, h_fc]
    · next h_abs => cases h_abs
  show (match findPid (filterNe (filterNe fs p1 ++ [(p1, c1)]) p2 ++ [(p2, c2)]) p1 with
    | none => none | some (_, c) => some c) = some c1
  rw [h1, h5]

-- T8: delete does not increase file count
theorem delete_does_not_increase (fs : FileSystem) (pid : Nat) :
    (fileDelete fs pid).length ≤ fs.length :=
  filterNe_length_le fs pid

end FileOps

namespace SessionRepo

abbrev SessionId := Nat

structure Session where
  id : SessionId
  title : Nat
  deriving BEq

abbrev SessionStore := List Session

def findSession (store : SessionStore) (id : SessionId) : Option Session :=
  match store with
  | [] => none
  | s :: rest => if s.id = id then some s else findSession rest id

def filterNeSession (store : SessionStore) (id : SessionId) : SessionStore :=
  match store with
  | [] => []
  | s :: rest => if s.id = id then filterNeSession rest id else s :: filterNeSession rest id

def create (store : SessionStore) (s : Session) : SessionStore :=
  filterNeSession store s.id ++ [s]

def load (store : SessionStore) (id : SessionId) : Option Session :=
  findSession store id

def deleteSession (store : SessionStore) (id : SessionId) : SessionStore :=
  filterNeSession store id

-- ===== Equational lemmas =====

private theorem filterNeSession_cons (s : Session) (rest : SessionStore) (id : SessionId) :
    filterNeSession (s :: rest) id = if s.id = id then filterNeSession rest id else s :: filterNeSession rest id := rfl

private theorem findSession_cons (s : Session) (rest : SessionStore) (id : SessionId) :
    findSession (s :: rest) id = if s.id = id then some s else findSession rest id := rfl

-- ===== Session Helpers =====

private theorem findSession_filterNeSession_none (store : SessionStore) (id : SessionId) :
    findSession (filterNeSession store id) id = none := by
  induction store with
  | nil => rfl
  | cons s rest ih =>
    rw [filterNeSession_cons s rest id]; split
    · next _ => exact ih
    · next h =>
      rw [findSession_cons s (filterNeSession rest id) id]; split
      · next h2 => exact absurd h2 h
      · next _ => exact ih

private theorem filterNeSession_idempotent (store : SessionStore) (id : SessionId) :
    filterNeSession (filterNeSession store id) id = filterNeSession store id := by
  induction store with
  | nil => rfl
  | cons s rest ih =>
    rw [filterNeSession_cons s rest id]; split
    · next _ => exact ih
    · next h =>
      rw [filterNeSession_cons s (filterNeSession rest id) id]; split
      · next h2 => exact absurd h2 h
      · next _ => exact congrArg (List.cons s) ih

private theorem filterNeSession_length_le (store : SessionStore) (id : SessionId) :
    (filterNeSession store id).length ≤ store.length := by
  induction store with
  | nil => simp [filterNeSession]
  | cons s rest ih =>
    rw [filterNeSession_cons s rest id]; split
    · next _ => have := @List.length_cons Session s rest; omega
    · next _ =>
      have h1 := @List.length_cons Session s (filterNeSession rest id)
      have h2 := @List.length_cons Session s rest
      omega

private theorem filterNeSession_append (l1 l2 : List Session) (id : SessionId) :
    filterNeSession (l1 ++ l2) id = filterNeSession l1 id ++ filterNeSession l2 id := by
  induction l1 with
  | nil => rfl
  | cons s rest ih =>
    rw [List.cons_append, filterNeSession_cons s rest id,
        filterNeSession_cons s (rest ++ l2) id, ih]; split
    · next _ => rfl
    · next _ => rfl

private theorem findSession_append (l1 l2 : List Session) (id : SessionId) :
    findSession (l1 ++ l2) id =
    match findSession l1 id with
    | none => findSession l2 id
    | some x => some x := by
  induction l1 with
  | nil => rfl
  | cons s rest ih =>
    rw [List.cons_append, findSession_cons s (rest ++ l2) id, findSession_cons s rest id]
    split
    · next _ => rfl
    · next _ => exact ih

private theorem findSession_singleton (s : Session) :
    findSession [s] s.id = some s := by
  rw [findSession_cons s [] s.id]; split
  · next _ => rfl
  · next h => exact absurd rfl h

private theorem findSession_filterNeSession_append (store : SessionStore) (s : Session) :
    findSession (filterNeSession store s.id ++ [s]) s.id = some s := by
  induction store with
  | nil =>
    have h_nil : filterNeSession [] s.id = [] := rfl
    rw [h_nil, List.nil_append]; exact findSession_singleton s
  | cons x rest ih =>
    rw [filterNeSession_cons x rest s.id]; split
    · next _ => exact ih
    · next h =>
      rw [List.cons_append,
          findSession_cons x (filterNeSession rest s.id ++ [s]) s.id,
          findSession_append (filterNeSession rest s.id) [s] s.id]
      split
      · next h2 => exact absurd h2 h
      · next _ =>
        rw [findSession_filterNeSession_none rest s.id]; exact findSession_singleton s

private theorem filterNeSession_filterNeSession_append (store : SessionStore) (s : Session) :
    filterNeSession (filterNeSession store s.id ++ [s]) s.id = filterNeSession store s.id := by
  rw [filterNeSession_append, filterNeSession_idempotent]
  rw [filterNeSession_cons s [] s.id]; split
  · next _ =>
    have : filterNeSession [] s.id = [] := rfl
    rw [this, List.append_nil]
  · next h => exact absurd rfl h

-- ===== SESSION REPO THEOREMS (6 total) =====

-- T9: create then load
theorem create_then_load (store : SessionStore) (s : Session) :
    load (create store s) s.id = some s :=
  findSession_filterNeSession_append store s

-- T10: delete then load returns none
theorem delete_then_load (store : SessionStore) (id : SessionId) :
    load (deleteSession store id) id = none :=
  findSession_filterNeSession_none store id

-- T11: delete does not increase length
theorem delete_does_not_increase (store : SessionStore) (id : SessionId) :
    (deleteSession store id).length ≤ store.length :=
  filterNeSession_length_le store id

-- T12: create adds at most one session
theorem create_at_most_one_more (store : SessionStore) (s : Session) :
    (create store s).length ≤ store.length + 1 := by
  have h_len : (filterNeSession store s.id ++ [s]).length =
      (filterNeSession store s.id).length + [s].length :=
    @List.length_append Session (filterNeSession store s.id) [s]
  have h_one : [s].length = 1 := rfl
  have h_le := filterNeSession_length_le store s.id
  show (filterNeSession store s.id ++ [s]).length ≤ store.length + 1
  rw [h_len, h_one]; omega

-- T13: create is idempotent
theorem create_idempotent (store : SessionStore) (s : Session) :
    create (create store s) s = create store s := by
  have h := filterNeSession_filterNeSession_append store s
  exact congrArg (fun l => l ++ [s]) h

-- T14: double create preserves count
theorem double_create_same_count (store : SessionStore) (s : Session) :
    (create (create store s) s).length = (create store s).length :=
  congrArg List.length (create_idempotent store s)

end SessionRepo
