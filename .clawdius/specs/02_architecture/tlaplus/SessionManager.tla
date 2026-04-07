---- MODULE SessionManager ----
EXTENDS Naturals, Sequences, FiniteSets
CONSTANT Sessions, MaxRetries

(* Corresponds to: crates/clawdius-core/src/session/manager.rs
   - SessionManager uses Arc<RwLock<Option<SessionId>>> for active session
   - SessionStore uses SQLite for persistence (blocking I/O)
   - Compactor triggers when token usage exceeds threshold_percent
   - States modeled after messaging/types.rs SessionState:
     Active, Idle, Compacted, Closed *)

VARIABLES session_state, active_session, messages, compacting, pc

ValidStates == {"active", "idle", "compacting", "closed"}

TypeInvariant ==
    /\ session_state \in [Sessions -> ValidStates]
    /\ active_session \in Sessions \union {None}
    /\ messages \in [Sessions -> Seq(Msg)]
    /\ compacting \subseteq Sessions
    /\ domain(messages) = Sessions

Init ==
    /\ session_state = [s \in Sessions |-> "idle"]
    /\ active_session = None
    /\ messages = [s \in Sessions |-> <<>>]
    /\ compacting = {}
    /\ pc = [s \in Sessions |-> "init"]

CreateSession(s) ==
    /\ pc[s] = "init"
    /\ session_state[s] = "idle"
    /\ active_session' = s
    /\ session_state' = [session_state EXCEPT ![s] = "active"]
    /\ pc' = [pc EXCEPT ![s] = "created"]

ActivateSession(s) ==
    /\ pc[s] \in {"init", "created"}
    /\ session_state[s] = "idle"
    /\ active_session' = s
    /\ session_state' = [session_state EXCEPT ![s] = "active"]
    /\ pc' = [pc EXCEPT ![s] = "active"]

AddMessage(s, msg) ==
    /\ session_state[s] = "active"
    /\ active_session = s
    /\ s \notin compacting
    /\ session_state' = session_state
    /\ messages' = [messages EXCEPT ![s] = Append(@, msg)]
    /\ active_session' = active_session
    /\ pc' = [pc EXCEPT ![s] = "active"]

BeginCompact(s) ==
    /\ session_state[s] = "active"
    /\ s \notin compacting
    /\ Len(messages[s]) >= MaxRetries
    /\ session_state' = [session_state EXCEPT ![s] = "compacting"]
    /\ compacting' = compacting \union {s}
    /\ pc' = [pc EXCEPT ![s] = "compacting"]

EndCompact(s) ==
    /\ session_state[s] = "compacting"
    /\ s \in compacting
    /\ session_state' = [session_state EXCEPT ![s] = "active"]
    /\ compacting' = compacting \ {s}
    /\ pc' = [pc EXCEPT ![s] = "active"]

CloseSession(s) ==
    /\ session_state[s] \in {"active", "idle"}
    /\ s \notin compacting
    /\ session_state' = [session_state EXCEPT ![s] = "closed"]
    /\ active_session' = IF active_session = s THEN None ELSE active_session
    /\ pc' = [pc EXCEPT ![s] = "closed"]

SetIdle(s) ==
    /\ session_state[s] = "active"
    /\ s \notin compacting
    /\ session_state' = [session_state EXCEPT ![s] = "idle"]
    /\ active_session' = IF active_session = s THEN None ELSE active_session
    /\ pc' = [pc EXCEPT ![s] = "idle"]

Next == \E s \in Sessions :
    \/ CreateSession(s)
    \/ ActivateSession(s)
    \/ AddMessage(s, <<"msg">>)
    \/ BeginCompact(s)
    \/ EndCompact(s)
    \/ CloseSession(s)
    \/ SetIdle(s)

Spec == Init /\ [][Next]_<<session_state, active_session, messages, compacting, pc>>

NoConcurrentWriters ==
    [](\A s \in Sessions :
        /\ session_state[s] = "active"
        => ~(s \in compacting /\ active_session = s))

ValidTransitions ==
    [](\A s \in Sessions :
        \/ session_state[s] = "idle" /\ session_state'[s] = "active"
        \/ session_state[s] = "active" /\ session_state'[s] \in {"idle", "compacting", "closed"}
        \/ session_state[s] = "compacting" /\ session_state'[s] = "active"
        \/ session_state[s] = "closed" /\ UNCHANGED session_state[s]
        \/ UNCHANGED session_state[s])

CompactionCompletes ==
    <>[](\A s \in Sessions :
        [](session_state[s] = "compacting" ~> <>session_state[s] = "active"))

SingleActiveSession ==
    [](\A s1, s2 \in Sessions :
        active_session = s1 /\ active_session = s2 => s1 = s2)

=============================================================================
(* TLC configuration:
   --SPECIFICATION Spec
   --INVARIANT TypeInvariant
   --INVARIANT NoConcurrentWriters
   --INVARIANT ValidTransitions
   --INVARIANT SingleActiveSession
   --PROPERTY CompactionCompletes
   --CONSTANT Sessions = {s1, s2}
   --CONSTANT MaxRetries = 3
*)
====
