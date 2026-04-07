---- MODULE WasmPluginRuntime ----
EXTENDS Naturals, FiniteSets
CONSTANT Plugins

(* Corresponds to: crates/clawdius-core/src/plugin/wasm.rs
   - PluginState enum: Loaded, Initializing, Active, Paused, Error, Unloading
   - WasmPlugin: metadata, config, state, engine, module, instance, linker, stats
   - WasmRuntime: Arc<RwLock<Vec<Arc<RwLock<WasmPlugin>>>>>
   - State transitions from wasm.rs:
     Loaded -> Initializing (initialize())
     Initializing -> Active (after _initialize + clawdius_init succeed)
     Active -> Unloading (shutdown())
     Unloading -> Loaded (after clawdius_shutdown, instance = None)
     Active -> Error (on_hook execution failure)
     Error -> Loaded (recovery: reload module)
   - Safety: WasmRuntime uses RwLock per plugin, preventing concurrent mutation
   - dispatch_hook reads plugin list lock, then per-plugin read lock *)

VARIABLES plugin_state, executing, hook_queue, error_count

ValidStates == {"unloaded", "loading", "initializing", "active", "paused", "error", "unloading"}

TypeInvariant ==
    /\ plugin_state \in [Plugins -> ValidStates]
    /\ executing \subseteq Plugins
    /\ \A p \in executing : plugin_state[p] = "active"
    /\ hook_queue \in [Plugins -> Nat]
    /\ error_count \in [Plugins -> Nat]

Init ==
    /\ plugin_state = [p \in Plugins |-> "unloaded"]
    /\ executing = {}
    /\ hook_queue = [p \in Plugins |-> 0]
    /\ error_count = [p \in Plugins |-> 0]

Load(p) ==
    /\ plugin_state[p] = "unloaded"
    /\ p \notin executing
    /\ plugin_state' = [plugin_state EXCEPT ![p] = "loading"]
    /\ UNCHANGED <<executing, hook_queue, error_count>>

Initialize(p) ==
    /\ plugin_state[p] = "loading"
    /\ plugin_state' = [plugin_state EXCEPT ![p] = "initializing"]
    /\ UNCHANGED <<executing, hook_queue, error_count>>

InitSuccess(p) ==
    /\ plugin_state[p] = "initializing"
    /\ plugin_state' = [plugin_state EXCEPT ![p] = "active"]
    /\ UNCHANGED <<executing, hook_queue, error_count>>

InitFail(p) ==
    /\ plugin_state[p] = "initializing"
    /\ plugin_state' = [plugin_state EXCEPT ![p] = "error"]
    /\ error_count' = [error_count EXCEPT ![p] = @ + 1]
    /\ UNCHANGED <<executing, hook_queue>>

ExecuteHook(p) ==
    /\ plugin_state[p] = "active"
    /\ p \notin executing
    /\ executing' = executing \union {p}
    /\ hook_queue' = [hook_queue EXCEPT ![p] = @ + 1]
    /\ UNCHANGED <<plugin_state, error_count>>

HookSuccess(p) ==
    /\ p \in executing
    /\ executing' = executing \ {p}
    /\ UNCHANGED <<plugin_state, hook_queue, error_count>>

HookError(p) ==
    /\ p \in executing
    /\ executing' = executing \ {p}
    /\ plugin_state' = [plugin_state EXCEPT ![p] = "error"]
    /\ error_count' = [error_count EXCEPT ![p] = @ + 1]
    /\ UNCHANGED <<hook_queue>>

Pause(p) ==
    /\ plugin_state[p] = "active"
    /\ p \notin executing
    /\ plugin_state' = [plugin_state EXCEPT ![p] = "paused"]
    /\ UNCHANGED <<executing, hook_queue, error_count>>

Resume(p) ==
    /\ plugin_state[p] = "paused"
    /\ plugin_state' = [plugin_state EXCEPT ![p] = "active"]
    /\ UNCHANGED <<executing, hook_queue, error_count>>

Shutdown(p) ==
    /\ plugin_state[p] = "active"
    /\ p \notin executing
    /\ plugin_state' = [plugin_state EXCEPT ![p] = "unloading"]
    /\ UNCHANGED <<executing, hook_queue, error_count>>

Unload(p) ==
    /\ plugin_state[p] = "unloading"
    /\ executing' = executing \ {p}
    /\ plugin_state' = [plugin_state EXCEPT ![p] = "unloaded"]
    /\ UNCHANGED <<hook_queue, error_count>>

RecoverFromError(p) ==
    /\ plugin_state[p] = "error"
    /\ p \notin executing
    /\ plugin_state' = [plugin_state EXCEPT ![p] = "unloaded"]
    /\ UNCHANGED <<executing, hook_queue, error_count>>

Next == \E p \in Plugins :
    \/ Load(p)
    \/ Initialize(p)
    \/ InitSuccess(p)
    \/ InitFail(p)
    \/ ExecuteHook(p)
    \/ HookSuccess(p)
    \/ HookError(p)
    \/ Pause(p)
    \/ Resume(p)
    \/ Shutdown(p)
    \/ Unload(p)
    \/ RecoverFromError(p)

Spec == Init /\ [][Next]_<<plugin_state, executing, hook_queue, error_count>>

NoConcurrentExecution ==
    [](Cardinality(executing) <= 1 \/ \A p1, p2 \in executing : p1 = p2)

SingleExecutorPerPlugin ==
    [](\A p \in Plugins : p \in executing => executing \cap {p} = {p})

ValidTransitions ==
    [](\A p \in Plugins :
        \/ plugin_state[p] = "unloaded" /\ plugin_state'[p] = "loading"
        \/ plugin_state[p] = "loading" /\ plugin_state'[p] = "initializing"
        \/ plugin_state[p] = "initializing" /\ plugin_state'[p] \in {"active", "error"}
        \/ plugin_state[p] = "active" /\ plugin_state'[p] \in {"paused", "error", "unloading"}
        \/ plugin_state[p] = "paused" /\ plugin_state'[p] = "active"
        \/ plugin_state[p] = "unloading" /\ plugin_state'[p] = "unloaded"
        \/ plugin_state[p] = "error" /\ plugin_state'[p] = "unloaded"
        \/ UNCHANGED plugin_state[p])

ErrorRecoverable ==
    [](\A p \in Plugins :
        plugin_state[p] = "error" ~> <>(plugin_state[p] = "unloaded"))

ShutdownCleanlyUnloads ==
    [](\A p \in Plugins :
        plugin_state[p] = "unloading" ~> <>(plugin_state[p] = "unloaded"))

=============================================================================
(* TLC configuration:
   --SPECIFICATION Spec
   --INVARIANT TypeInvariant
   --INVARIANT NoConcurrentExecution
   --INVARIANT ValidTransitions
   --PROPERTY ErrorRecoverable
   --PROPERTY ShutdownCleanlyUnloads
   --CONSTANT Plugins = {p1, p2}
*)
====
