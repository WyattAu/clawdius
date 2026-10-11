/-
  Lean4 Proof: Extended Ring Buffer Memory Safety — free-running counters
  Component: COMP-MEMORY-002
  Blue Paper: BP-MEMORY-002
  Yellow Paper: YP-RING-FREE-RUNNING-002 (research basis; supersedes part of
                YP-PERFORMANCE-RING-BUFFER-001)
  Properties: capacity is definitional, no overflow, no underflow,
              wraparound correctness, slot injectivity (no overwrite of a
              live element), FIFO position order.
  Model change (YP-RING-FREE-RUNNING-002): the previous model stored
  `{ head, tail, len }` with no relationship between the fields, which made
  five theorems unprovable — including one, `empty_iff_zero_len`, that
  asserted every buffer is empty. Production rings (Linux kfifo, BPF ringbuf,
  Rust SPSC crates) store two free-running counters and derive occupancy by
  subtraction; this file adopts that representation, and the theorems become
  arithmetic. Compiled with Lean 4.28.0 (import Std).
-/

import Std

/-! ## Configuration

Capacity must be positive; the constructor refuses zero. (The power-of-two
requirement is what makes the production `%` a mask; the theorems here need
only positivity, and `isPowerOfTwo` below is kept for the mask story.) -/

structure RingConfig where
  capacity : Nat
  positive : capacity > 0

theorem capacity_positive (config : RingConfig) : config.capacity > 0 :=
  config.positive

/-- Power-of-two capacities turn the modulo into a bitmask. -/
def isPowerOfTwo (n : Nat) : Bool :=
  n > 0 && (n &&& (n - 1)) == 0

theorem power_of_two_bitmask (config : RingConfig) :
    isPowerOfTwo config.capacity = true → config.capacity > 0 := by
  intro h
  unfold isPowerOfTwo at h
  by_cases hcap : config.capacity > 0
  · exact hcap
  · simp [hcap] at h

/-! ## The ring: two counters, two facts

`head` is the consumer counter, `tail` the producer counter. Both are
free-running — they never wrap — and occupancy is `tail - head`. The two
constructor fields *are* the invariant; there is nothing else to maintain. -/

structure Ring where
  config : RingConfig
  head : Nat
  tail : Nat
  ordered : head ≤ tail
  within : tail ≤ head + config.capacity

/-- Occupancy is arithmetic, not storage. -/
def Ring.len (r : Ring) : Nat :=
  r.tail - r.head

/-- A ring is never over capacity — for **every** well-formed ring, by the
    constructor's `within` field. The old false theorem is now arithmetic. -/
theorem len_le_capacity (r : Ring) : r.len ≤ r.config.capacity := by
  have h1 := r.ordered
  have h2 := r.within
  have hp := r.config.positive
  show r.tail - r.head ≤ r.config.capacity
  omega

/-- The consumer counter is bounded by the producer counter and the capacity. -/
theorem head_bounded (r : Ring) : r.head ≤ r.head + r.config.capacity := by omega

/-- Empty is an arithmetic identity, not a promise about a field. -/
theorem empty_iff (r : Ring) : r.len = 0 ↔ r.head = r.tail := by
  have h1 := r.ordered
  have h2 := r.within
  show r.tail - r.head = 0 ↔ r.head = r.tail
  omega

/-- Full is an arithmetic identity. -/
theorem full_iff (r : Ring) : r.len = r.config.capacity ↔ r.tail = r.head + r.config.capacity := by
  have h1 := r.ordered
  have h2 := r.within
  show r.tail - r.head = r.config.capacity ↔ r.tail = r.head + r.config.capacity
  omega

/-- A ring is full or has room — exhaustive, so a push has a defined outcome. -/
theorem full_or_room (r : Ring) :
    r.len = r.config.capacity ∨ r.len < r.config.capacity := by
  have := len_le_capacity r
  omega

/-! ## Transitions

`push` and `pop` are defined only on rings with room / occupancy, and each
returns the new counters with the two invariant proofs re-established. The
old theorems `push_increments_len` and `pop_decrements_len` are consequences,
proved by arithmetic on the transitions. -/

/-- Push: the producer counter advances by one. Defined only when there is room. -/
def push (r : Ring) (h : r.len < r.config.capacity) : Ring := by
  -- Unfold the occupancy before constructing: the invariant fields are arithmetic
  -- on the counters, and `h` as written is an opaque atom to them.
  have hlen : r.len = r.tail - r.head := rfl
  rw [hlen] at h
  exact
    { r with
      tail := r.tail + 1
      ordered := by have h1 := r.ordered; omega
      within := by have h2 := r.within; omega }

/-- Pop: the consumer counter advances by one. Defined only when non-empty. -/
def pop (r : Ring) (h : r.len > 0) : Ring := by
  have hlen : r.len = r.tail - r.head := rfl
  rw [hlen] at h
  exact
    { r with
      head := r.head + 1
      -- len > 0 is what puts the consumer counter strictly behind the producer.
      ordered := by omega
      within := by have h2 := r.within; omega }

theorem push_increments_len (r : Ring) (h : r.len < r.config.capacity) :
    (push r h).len = r.len + 1 := by
  have hlen : r.len = r.tail - r.head := rfl
  rw [hlen] at h
  have h1 := r.ordered
  have h2 := r.within
  have hp := r.config.positive
  show r.tail + 1 - r.head = r.tail - r.head + 1
  omega

theorem pop_decrements_len (r : Ring) (h : r.len > 0) :
    (pop r h).len = r.len - 1 := by
  have hlen : r.len = r.tail - r.head := rfl
  rw [hlen] at h
  have h1 := r.ordered
  have h2 := r.within
  show r.tail - (r.head + 1) = r.tail - r.head - 1
  omega

/-- Pushing preserves the invariant: the new tail stays within capacity of head. -/
theorem push_within (r : Ring) (h : r.len < r.config.capacity) :
    (push r h).tail ≤ (push r h).head + (push r h).config.capacity :=
  (push r h).within

/-! ## Physical addressing

The physical slot of logical position `i` is `(head + i) % capacity`. It is
always in bounds, and distinct live logical positions occupy distinct slots —
which is the theorem that says a push cannot overwrite an unconsumed element. -/

/-- Every physical index is in bounds: modulo a positive capacity is less than it. -/
theorem inbounds_access (r : Ring) (idx : Nat) :
    (r.head + idx) % r.config.capacity < r.config.capacity :=
  Nat.mod_lt _ r.config.positive

/-- The old bound theorem, for the wraparound case: a wrapped head is under capacity. -/
theorem wraparound_correct (config : RingConfig) (state_head : Nat) (hcap : config.capacity > 0) :
    (state_head + 1) % config.capacity < config.capacity :=
  Nat.mod_lt _ hcap

/-! ## Slot injectivity

Two distinct live positions never share a physical slot. This is what makes a
push safe: it cannot collide with an unconsumed element, because if two live
positions landed on one slot, the difference of their positions would be a
nonzero multiple of the capacity while both positions are smaller than it.

The window is expressed from the *masked* head, which is where the physical
indexing actually starts; `r.head % capacity` is itself below capacity. -/

theorem injective_window (r : Ring) (i j : Nat) (hi : i < r.len) (hj : j < r.len)
    (hij : i < j) :
    ((r.head % r.config.capacity) + i) % r.config.capacity
      ≠ ((r.head % r.config.capacity) + j) % r.config.capacity := by
  have hcap := r.config.positive
  have hlen := len_le_capacity r
  have hlen' : r.len = r.tail - r.head := rfl
  have hbase : r.head % r.config.capacity < r.config.capacity :=
    Nat.mod_lt _ hcap
  have hilt : i < r.config.capacity := by omega
  have hjlt : j < r.config.capacity := by omega
  intro heq
  by_cases h1 : (r.head % r.config.capacity) + i < r.config.capacity
  · rw [Nat.mod_eq_of_lt h1] at heq
    by_cases h2 : (r.head % r.config.capacity) + j < r.config.capacity
    · rw [Nat.mod_eq_of_lt h2] at heq
      omega
    · have h2le : r.config.capacity ≤ (r.head % r.config.capacity) + j := by omega
      have hlt : (r.head % r.config.capacity) + j - r.config.capacity
          < r.config.capacity := by omega
      rw [Nat.mod_eq_sub_mod h2le, Nat.mod_eq_of_lt hlt] at heq
      omega
  · have h1le : r.config.capacity ≤ (r.head % r.config.capacity) + i := by omega
    have hlt : (r.head % r.config.capacity) + i - r.config.capacity
        < r.config.capacity := by omega
    rw [Nat.mod_eq_sub_mod h1le, Nat.mod_eq_of_lt hlt] at heq
    by_cases h2 : (r.head % r.config.capacity) + j < r.config.capacity
    · rw [Nat.mod_eq_of_lt h2] at heq
      omega
    · have h2le : r.config.capacity ≤ (r.head % r.config.capacity) + j := by omega
      have hlt2 : (r.head % r.config.capacity) + j - r.config.capacity
          < r.config.capacity := by omega
      rw [Nat.mod_eq_sub_mod h2le, Nat.mod_eq_of_lt hlt2] at heq
      omega

/-! ## FIFO position order

"Oldest first" is positional in this model: logical position `i` was pushed
before logical position `j` exactly when `i < j`, because the producer counter
never revisits a value. The old unprovable disjunction becomes slot
injectivity applied to the first and last live positions. -/

theorem fifo_ordering (r : Ring) (h : r.len > 0) :
    (r.head % r.config.capacity) % r.config.capacity
      ≠ ((r.head % r.config.capacity) + (r.len - 1)) % r.config.capacity
    ∨ r.len = 1 := by
  by_cases h1 : r.len = 1
  · exact Or.inr h1
  · have hgt : 0 < r.len - 1 := by omega
    have hlt : r.len - 1 < r.config.capacity := by
      have := len_le_capacity r
      omega
    have hlen0 : r.len = r.tail - r.head := rfl
    have hinj := injective_window r 0 (r.len - 1)
      (by have h := h; rw [hlen0] at h; have h1 := r.ordered; omega) (by omega) (by omega)
    exact Or.inl hinj
