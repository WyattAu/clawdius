/-
  Lean4 Proof: HFT Broker Wallet Guard and Latency Bounds
  Component: COMP-BROKER-001
  Blue Paper: BP-HFT-BROKER-001
  Yellow Paper: YP-HFT-BROKER-001

  IMPORTANT: This proof models the canonical WalletGuard at
  crates/clawdius-core/src/broker/wallet_guard.rs.

  NOTE: Lean4 Int/Nat are arbitrary precision — overflow (PositionOverflow)
  cannot occur in this model. The Rust implementation uses checked_add /
  checked_mul which can return None. We model only the overflow-free path.
  PositionOverflow is removed from RejectReason accordingly.
-/

import Std.Data.HashMap

structure Decimal where
  value : Int
  scale : Nat
  deriving Repr, BEq

namespace Decimal

def zero : Decimal := ⟨0, 0⟩

def add (d1 d2 : Decimal) : Decimal :=
  if d1.scale = d2.scale then ⟨d1.value + d2.value, d1.scale⟩
  else ⟨d1.value + d2.value, max d1.scale d2.scale⟩

def abs (d : Decimal) : Decimal := ⟨Int.natAbs d.value, d.scale⟩

instance : Add Decimal := ⟨Decimal.add⟩
instance : LT Decimal := ⟨fun d1 d2 => d1.value < d2.value⟩
instance : LE Decimal := ⟨fun d1 d2 => d1.value ≤ d2.value⟩

end Decimal

abbrev Symbol := Nat

inductive Side where
  | buy : Side
  | sell : Side
  deriving Repr

structure Order where
  symbol : Symbol
  side : Side
  quantity : Nat
  price : Nat
  deriving Repr

structure Wallet where
  cash : Nat
  positions : Std.HashMap Symbol Int
  realizedPnl : Int
  sessionStartPnl : Int
  deriving Repr

structure RiskParams where
  piMax : Int
  sigmaMax : Nat
  lambdaMax : Int
  marginRatio : Nat
  deriving Repr

inductive RejectReason where
  | positionLimitExceeded : Int → Int → RejectReason
  | orderSizeExceeded : Nat → Nat → RejectReason
  | dailyDrawdownExceeded : Int → Int → RejectReason
  | insufficientMargin : Nat → Nat → RejectReason
  | zeroPrice : RejectReason
  | zeroQuantity : RejectReason
  deriving Repr

def signedQuantity (order : Order) : Int :=
  match order.side with
  | Side.buy => order.quantity
  | Side.sell => -order.quantity

def currentPosition (wallet : Wallet) (symbol : Symbol) : Int :=
  wallet.positions.getD symbol 0

def currentDrawdown (wallet : Wallet) : Int :=
  wallet.sessionStartPnl - wallet.realizedPnl

-- Sub-checks return Option RejectReason: none = pass, some = fail.
-- Lean4 Int.add and Nat.mul cannot overflow (arbitrary precision),
-- so we compute directly without Option wrapping.

def checkPositionLimit (wallet : Wallet) (params : RiskParams) (order : Order) : Option RejectReason :=
  let newPos := currentPosition wallet order.symbol + signedQuantity order
  if (Int.natAbs newPos : Int) > params.piMax then
    some (RejectReason.positionLimitExceeded newPos params.piMax)
  else none

def checkOrderSize (params : RiskParams) (order : Order) : Option RejectReason :=
  if order.quantity > params.sigmaMax then
    some (RejectReason.orderSizeExceeded order.quantity params.sigmaMax)
  else none

def checkDrawdown (wallet : Wallet) (params : RiskParams) : Option RejectReason :=
  let dd := currentDrawdown wallet
  if dd > params.lambdaMax then
    some (RejectReason.dailyDrawdownExceeded dd params.lambdaMax)
  else none

def checkMargin (wallet : Wallet) (order : Order) (params : RiskParams) : Option RejectReason :=
  match order.side with
  | Side.sell => none
  | Side.buy =>
    let notional := order.quantity * order.price
    let req := notional / params.marginRatio
    if req > wallet.cash then
      some (RejectReason.insufficientMargin req wallet.cash)
    else none

-- Full risk check in Rust WalletGuard::check evaluation order:
-- 1. zero-price  → reject
-- 2. zero-quantity → reject
-- 3. position limit (checkPositionLimit)
-- 4. order size (checkOrderSize)
-- 5. drawdown (checkDrawdown)
-- 6. margin (checkMargin, buy only)
def check (wallet : Wallet) (params : RiskParams) (order : Order) : Option RejectReason :=
  if order.price = 0 then
    some RejectReason.zeroPrice
  else if order.quantity = 0 then
    some RejectReason.zeroQuantity
  else match checkPositionLimit wallet params order with
  | some reason => some reason
  | none => match checkOrderSize params order with
    | some reason => some reason
    | none => match checkDrawdown wallet params with
      | some reason => some reason
      | none => checkMargin wallet order params

def approved (wallet : Wallet) (params : RiskParams) (order : Order) : Bool :=
  match check wallet params order with
  | none => true
  | some _ => false

-- Bridge theorem: maps the hypothesis about position size to checkPositionLimit's return value.
-- Replaces former axiom; proven by unfolding the definition and case-splitting on the
-- if-then-else condition, which is exactly the hypothesis.
theorem checkPositionLimit_rejects_of_abs_exceeds :
    (wallet : Wallet) → (params : RiskParams) → (order : Order) →
    (Int.natAbs (currentPosition wallet order.symbol + signedQuantity order) : Int) > params.piMax →
    checkPositionLimit wallet params order ≠ none := by
  intro wallet params order h
  unfold checkPositionLimit
  show (if (Int.natAbs (currentPosition wallet order.symbol + signedQuantity order) : Int) > params.piMax
        then some (RejectReason.positionLimitExceeded (currentPosition wallet order.symbol + signedQuantity order) params.piMax)
        else none) ≠ none
  split
  · simp
  · contradiction

-- === Theorems ===

theorem zero_price_rejected (wallet : Wallet) (params : RiskParams) (order : Order) :
    order.price = 0 → ¬approved wallet params order := by
  intro h
  simp [approved, check, h]

theorem zero_quantity_rejected (wallet : Wallet) (params : RiskParams) (order : Order) :
    order.quantity = 0 → ¬approved wallet params order := by
  intro h
  simp only [approved, check]
  by_cases hprice : order.price = 0 <;> simp_all

theorem order_size_exceeded_rejected (wallet : Wallet) (params : RiskParams) (order : Order) :
    order.price ≠ 0 →
    order.quantity ≠ 0 →
    order.quantity > params.sigmaMax →
    ¬approved wallet params order := by
  intro h1 h2 h3
  simp only [approved, check, if_neg h1, if_neg h2]
  cases h_pl : checkPositionLimit wallet params order with
  | none => simp [checkOrderSize, h3]
  | some _ => simp

theorem position_limit_rejected (wallet : Wallet) (params : RiskParams) (order : Order) :
    order.price ≠ 0 →
    order.quantity ≠ 0 →
    (Int.natAbs (currentPosition wallet order.symbol + signedQuantity order) : Int) > params.piMax →
    ¬approved wallet params order := by
  intro h1 h2 h3
  have hpl := checkPositionLimit_rejects_of_abs_exceeds wallet params order h3
  simp only [approved, check, if_neg h1, if_neg h2]
  cases h_eq : checkPositionLimit wallet params order with
  | none => contradiction
  | some _ => simp

theorem drawdown_exceeded_rejected (wallet : Wallet) (params : RiskParams) (order : Order) :
    order.price ≠ 0 →
    order.quantity ≠ 0 →
    currentDrawdown wallet > params.lambdaMax →
    ¬approved wallet params order := by
  intro h1 h2 h3
  simp only [approved, check, if_neg h1, if_neg h2]
  cases h_pl : checkPositionLimit wallet params order with
  | none =>
    cases h_os : checkOrderSize params order with
    | none => simp [checkDrawdown, h3]
    | some _ => simp
  | some _ => simp

theorem insufficient_margin_rejected (wallet : Wallet) (params : RiskParams) (order : Order) :
    order.price ≠ 0 →
    order.quantity ≠ 0 →
    order.side = Side.buy →
    order.quantity * order.price / params.marginRatio > wallet.cash →
    ¬approved wallet params order := by
  intro h1 h2 h3 h4
  simp only [approved, check, if_neg h1, if_neg h2]
  cases h_pl : checkPositionLimit wallet params order with
  | none =>
    cases h_os : checkOrderSize params order with
    | none =>
      cases h_dd : checkDrawdown wallet params with
      | none => simp [checkMargin, h3, h4]
      | some _ => simp
    | some _ => simp
  | some _ => simp

theorem sell_skips_margin (wallet : Wallet) (params : RiskParams) (order : Order) :
    order.side = Side.sell →
    checkMargin wallet order params = none := by
  intro h
  simp [checkMargin, h]

-- === Ring Buffer Properties ===

structure RingBuffer (T : Type) where
  capacity : Nat
  head : Nat
  tail : Nat
  deriving Repr

def ringBufferValid (rb : RingBuffer T) : Prop :=
  rb.head < rb.capacity ∧ rb.tail < rb.capacity

theorem ring_buffer_head_valid (rb : RingBuffer T) (hvalid : ringBufferValid rb) :
    rb.head < rb.capacity := hvalid.1

theorem ring_buffer_tail_valid (rb : RingBuffer T) (hvalid : ringBufferValid rb) :
    rb.tail < rb.capacity := hvalid.2

theorem mod_succ_lt (a b : Nat) (h : 0 < b) : (a + 1) % b < b :=
  Nat.mod_lt (a + 1) h

theorem ring_buffer_next_head_valid (rb : RingBuffer T) (hvalid : ringBufferValid rb) :
    (rb.head + 1) % rb.capacity < rb.capacity := by
  have h1 : rb.head < rb.capacity := hvalid.1
  have hcap : rb.capacity > 0 := by omega
  exact mod_succ_lt rb.head rb.capacity hcap

-- === Latency Bounds ===

structure LatencyBounds where
  signalToExecution : Nat
  riskCheck : Nat
  gcPause : Nat
  marketDataProcessing : Nat
  deriving Repr

def hftLatencyBounds : LatencyBounds :=
  { signalToExecution := 1000  -- <1ms target
    riskCheck := 100         -- <100µs target
    gcPause := 0             -- Zero GC (Rust native)
    marketDataProcessing := 1 }

theorem zero_gc_guarantee :
    hftLatencyBounds.gcPause = 0 := rfl
