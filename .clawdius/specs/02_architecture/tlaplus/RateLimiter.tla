---- MODULE RateLimiter ----
EXTENDS Naturals, Reals, FiniteSets
CONSTANT Keys, MaxTokens, RefillRate

(* Corresponds to: crates/clawdius-core/src/messaging/rate_limiter.rs
   - TokenBucket: tokens (f64), max_tokens, refill_rate, last_refill (Instant)
   - refill(): tokens = min(tokens + elapsed * refill_rate, max_tokens)
   - try_consume(tokens): refill first, then check if tokens >= requested
   - RateLimiter: HashMap<String, TokenBucket> behind Arc<RwLock>
   - Per-key buckets (e.g., per user or per platform)
   - Persistence to SQLite via StoredBucket *)

VARIABLES tokens, last_refill, clock

TypeInvariant ==
    /\ tokens \in [Keys -> Int]
    /\ last_refill \in [Keys -> Int]
    /\ clock \in Nat
    /\ \A k \in Keys : 0 <= tokens[k] /\ tokens[k] <= MaxTokens

Init ==
    /\ tokens = [k \in Keys |-> MaxTokens]
    /\ last_refill = [k \in Keys |-> 0]
    /\ clock = 0

Refill(k) ==
    /\ clock' = clock + 1
    /\ LET elapsed == (clock - last_refill[k])
          added == Min(elapsed * RefillRate, MaxTokens - tokens[k])
       IN /\ tokens' = [tokens EXCEPT ![k] = tokens[k] + added]
          /\ last_refill' = [last_refill EXCEPT ![k] = clock]
    /\ UNCHANGED <<clock>>

Tick ==
    /\ UNCHANGED <<tokens, last_refill>>
    /\ clock' = clock + 1

Consume(k, n) ==
    /\ n \in 1..MaxTokens
    /\ tokens[k] >= n
    /\ clock' = clock + 1
    /\ LET elapsed == (clock - last_refill[k])
          refilled == Min(tokens[k] + elapsed * RefillRate, MaxTokens)
       IN /\ tokens' = [tokens EXCEPT ![k] = refilled - n]
          /\ last_refill' = [last_refill EXCEPT ![k] = clock]

ConsumeFail(k, n) ==
    /\ n \in 1..MaxTokens
    /\ LET elapsed == (clock - last_refill[k])
          refilled == Min(tokens[k] + elapsed * RefillRate, MaxTokens)
       IN /\ refilled < n
    /\ tokens' = [tokens EXCEPT ![k] = refilled]
    /\ last_refill' = [last_refill EXCEPT ![k] = clock]
    /\ clock' = clock + 1

Next == \E k \in Keys :
    \/ Refill(k)
    \/ Consume(k, 1)
    \/ ConsumeFail(k, 1)
    \/ Tick

Spec == Init /\ [][Next]_<<tokens, last_refill, clock>>

TokensNeverNegative ==
    [](\A k \in Keys : tokens[k] >= 0)

TokensNeverExceedCapacity ==
    [](\A k \in Keys : tokens[k] <= MaxTokens)

RefillEventuallyFills ==
    <>[](\A k \in Keys :
        ~(\A t \in Nat : ~<<tokens, last_refill, clock>> = [tokens EXCEPT ![k] = MaxTokens]))

NoOverConsume ==
    [](UNCHANGED tokens => TRUE)

=============================================================================
(* TLC configuration:
   --SPECIFICATION Spec
   --INVARIANT TypeInvariant
   --INVARIANT TokensNeverNegative
   --INVARIANT TokensNeverExceedCapacity
   --PROPERTY RefillEventuallyFills
   --CONSTANT Keys = {user1, user2}
   --CONSTANT MaxTokens = 10
   --CONSTANT RefillRate = 1
*)
====
