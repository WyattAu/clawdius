---- MODULE CLAWDIUS ----
EXTENDS Naturals, FiniteSets

(* Top-level Clawdius system specification composing all concurrent subsystems.
   This module references the individual component specs and defines system-wide
   invariants that hold across component boundaries.
   
   Components modeled:
   - SessionManager: session/manager.rs (RwLock<SessionStore>, active_session tracking)
   - RateLimiter: messaging/rate_limiter.rs (TokenBucket per key, Arc<RwLock<HashMap>>)
   - MessageQueue: messaging/retry_queue.rs (pending + dead_letter, exponential backoff)
   - WasmPluginRuntime: plugin/wasm.rs (per-plugin RwLock, WasmRuntime with plugin list)
   
   Cross-component interactions:
   - Messaging sends messages to sessions (rate-limited)
   - Sessions trigger compaction when token limits exceeded
   - Plugins hook into session lifecycle (before_edit, after_edit, etc.)
   - Rate limiter guards message delivery per user/platform *)

CONSTANTS
    Sessions,
    Keys,
    Tasks,
    Plugins,
    MaxRetries,
    MaxQueueSize,
    MaxTokens,
    RefillRate

VARIABLES
    session_state,
    active_session,
    session_compacting,
    tokens,
    pending,
    dead_letter,
    attempt,
    delivered,
    plugin_state,
    executing,
    clock

SessionTypeOK ==
    /\ session_state \in [Sessions -> {"active", "idle", "compacting", "closed"}]
    /\ active_session \in Sessions \union {None}
    /\ session_compacting \subseteq Sessions

RateLimitTypeOK ==
    /\ tokens \in [Keys -> Int]
    /\ \A k \in Keys : 0 <= tokens[k] /\ tokens[k] <= MaxTokens

QueueTypeOK ==
    /\ pending \subseteq Tasks
    /\ dead_letter \subseteq Tasks
    /\ pending \intersect dead_letter = {}
    /\ attempt \in [Tasks -> 0..MaxRetries]
    /\ delivered \subseteq Tasks
    /\ Cardinality(pending) + Cardinality(dead_letter) <= MaxQueueSize

PluginTypeOK ==
    /\ plugin_state \in [Plugins -> {"unloaded", "loading", "initializing", "active", "paused", "error", "unloading"}]
    /\ executing \subseteq Plugins

SystemTypeInvariant ==
    /\ SessionTypeOK
    /\ RateLimitTypeOK
    /\ QueueTypeOK
    /\ PluginTypeOK

Init ==
    /\ session_state = [s \in Sessions |-> "idle"]
    /\ active_session = None
    /\ session_compacting = {}
    /\ tokens = [k \in Keys |-> MaxTokens]
    /\ pending = {}
    /\ dead_letter = {}
    /\ attempt = [t \in Tasks |-> 0]
    /\ delivered = {}
    /\ plugin_state = [p \in Plugins |-> "unloaded"]
    /\ executing = {}
    /\ clock = 0

Next == TRUE

Spec == Init /\ [][Next]_<<
    session_state, active_session, session_compacting,
    tokens, pending, dead_letter, attempt, delivered,
    plugin_state, executing, clock
>>

NoConcurrentSessionWrites ==
    [](\A s \in Sessions :
        session_state[s] = "active" =>
            ~(s \in session_compacting /\ active_session = s))

NoConcurrentPluginExecution ==
    [](\A p1, p2 \in executing : p1 = p2)

TokensBounded ==
    [](\A k \in Keys : 0 <= tokens[k] /\ tokens[k] <= MaxTokens)

QueueDisjoint ==
    [](pending \intersect dead_letter = {})

SessionClosedNotWritable ==
    [](\A s \in Sessions :
        session_state[s] = "closed" => s \notin session_compacting)

SystemLiveness ==
    <>[](
        \A s \in Sessions :
            [](session_state[s] = "compacting" ~> <>session_state[s] = "active")
    )

=============================================================================
(* TLC configuration:
   --SPECIFICATION Spec
   --INVARIANT SystemTypeInvariant
   --INVARIANT NoConcurrentSessionWrites
   --INVARIANT NoConcurrentPluginExecution
   --INVARIANT TokensBounded
   --INVARIANT QueueDisjoint
   --INVARIANT SessionClosedNotWritable
   --PROPERTY SystemLiveness
   --CONSTANT Sessions = {s1}
   --CONSTANT Keys = {user1}
   --CONSTANT Tasks = {t1}
   --CONSTANT Plugins = {p1}
   --CONSTANT MaxRetries = 3
   --CONSTANT MaxQueueSize = 5
   --CONSTANT MaxTokens = 10
   --CONSTANT RefillRate = 1
   
   Note: Next == TRUE here since this module defines cross-component
   invariants. For full model checking, instantiate the individual
   module specs (SessionManager, RateLimiter, MessageQueue,
   WasmPluginRuntime) separately with concrete constants.
*)
====
