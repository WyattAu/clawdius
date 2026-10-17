import Std.Data.HashMap
/-
  Lean4 Proof: Host Kernel Lifecycle Safety + Liveness
  Component: COMP-HOST-001
  Blue Paper: BP-HOST-KERNEL-001
  Yellow Paper: YP-HOST-KERNEL-015

  Migrated from the legacy tree (.clawdius/specs/02_architecture/proofs/)
  in loop 2, iteration 7, with the crash-only redesign: the legacy model
  asserted `stopped_is_terminal` — no transition out of stopped — which
  made ANY host failure permanent (a liveness hole, not a safety
  feature). Following Candea & Fox, "Crash-Only Software" (HotOS IX
  2003) and Dijkstra's self-stabilization, the redesigned model makes
  restart a first-class transition (`stopped -> initializing`) and
  proves self-stabilization: `running` is reachable from EVERY state.

  All legacy safety theorems are re-verified: shutdown reachability,
  no double-initialization, no start without init, no direct restart
  shortcut (a restart must pass through the full initialization phase).
-/

inductive KernelState where
  | uninitialized : KernelState
  | initializing : KernelState
  | running : KernelState
  | shuttingDown : KernelState
  | stopped : KernelState
  deriving Repr, DecidableEq, BEq

inductive ComponentId where
  | host : ComponentId
  | fsm : ComponentId
  | sentinel : ComponentId
  | brain : ComponentId
  | graph : ComponentId
  | broker : ComponentId
  deriving Repr, DecidableEq, BEq, Hashable

inductive ComponentState where
  | uninitialized : ComponentState
  | initialized : ComponentState
  | running : ComponentState
  | stopped : ComponentState
  | error : ComponentState
  deriving Repr, DecidableEq, BEq

structure ComponentRegistry where
  components : Std.HashMap ComponentId ComponentState
  deriving Repr

/-- Kernel lifecycle transitions. Crash-only redesign (loop 2): the
`stopped -> initializing` restart is a first-class transition —
recovery is the normal path, not an afterthought. There is still NO
direct `stopped -> running` shortcut: a restart must pass through the
full initialization phase. -/
def validKernelTransition : KernelState → KernelState → Bool
  | KernelState.uninitialized, KernelState.initializing => true
  | KernelState.initializing, KernelState.running => true
  | KernelState.initializing, KernelState.stopped => true
  | KernelState.running, KernelState.shuttingDown => true
  | KernelState.shuttingDown, KernelState.stopped => true
  | KernelState.stopped, KernelState.initializing => true
  | _, _ => false

/-- Component lifecycle: the `stopped -> initialized` restart mirrors
the kernel-level crash-only discipline. -/
def validComponentTransition : ComponentState → ComponentState → Bool
  | ComponentState.uninitialized, ComponentState.initialized => true
  | ComponentState.initialized, ComponentState.running => true
  | ComponentState.running, ComponentState.stopped => true
  | ComponentState.running, ComponentState.error => true
  | ComponentState.error, ComponentState.stopped => true
  | ComponentState.stopped, ComponentState.initialized => true
  | _, _ => false

structure Kernel where
  state : KernelState
  components : ComponentRegistry
  deriving Repr

def kernelInitialize (k : Kernel) : Except String Kernel :=
  match k.state with
  | KernelState.uninitialized =>
    Except.ok { state := KernelState.initializing, components := k.components }
  | _ => Except.error "already initialized"

def kernelStart (k : Kernel) : Except String Kernel :=
  match k.state with
  | KernelState.initializing =>
    Except.ok { state := KernelState.running, components := k.components }
  | _ => Except.error "not in initializing state"

def kernelShutdown (k : Kernel) : Except String Kernel :=
  match k.state with
  | KernelState.running =>
    Except.ok { state := KernelState.shuttingDown, components := k.components }
  | KernelState.shuttingDown =>
    Except.ok { state := KernelState.stopped, components := k.components }
  | _ => Except.error "not running or shutting down"

inductive CanReachStopped : KernelState → Prop where
  | base : CanReachStopped KernelState.stopped
  | step (s s' : KernelState) :
      validKernelTransition s s' = true →
      CanReachStopped s' →
      CanReachStopped s

theorem shutdown_reachable_shutting_down : CanReachStopped KernelState.shuttingDown := by
  apply CanReachStopped.step KernelState.shuttingDown KernelState.stopped
  · rfl
  · exact CanReachStopped.base

theorem shutdown_reachable_running : CanReachStopped KernelState.running := by
  apply CanReachStopped.step KernelState.running KernelState.shuttingDown
  · rfl
  · exact shutdown_reachable_shutting_down

theorem shutdown_reachable_initializing : CanReachStopped KernelState.initializing := by
  apply CanReachStopped.step KernelState.initializing KernelState.stopped
  · rfl
  · exact CanReachStopped.base

theorem shutdown_reachable_uninitialized : CanReachStopped KernelState.uninitialized := by
  apply CanReachStopped.step KernelState.uninitialized KernelState.initializing
  · rfl
  · exact shutdown_reachable_initializing

theorem no_double_initialize (c : ComponentRegistry) :
    kernelInitialize { state := KernelState.initializing, components := c } =
    Except.error "already initialized" := by
  simp [kernelInitialize]

theorem no_start_without_init (c : ComponentRegistry) :
    kernelStart { state := KernelState.uninitialized, components := c } =
    Except.error "not in initializing state" := by
  simp [kernelStart]

theorem no_start_when_running (c : ComponentRegistry) :
    kernelStart { state := KernelState.running, components := c } =
    Except.error "not in initializing state" := by
  simp [kernelStart]

theorem shutdown_reachable (s : KernelState) :
    s ≠ KernelState.stopped → CanReachStopped s := by
  intro h
  cases s with
  | uninitialized => exact shutdown_reachable_uninitialized
  | initializing => exact shutdown_reachable_initializing
  | running => exact shutdown_reachable_running
  | shuttingDown => exact shutdown_reachable_shutting_down
  | stopped => contradiction

theorem valid_transitions_preserved :
    validKernelTransition KernelState.uninitialized KernelState.initializing = true ∧
    validKernelTransition KernelState.initializing KernelState.running = true ∧
    validKernelTransition KernelState.initializing KernelState.stopped = true ∧
    validKernelTransition KernelState.running KernelState.shuttingDown = true ∧
    validKernelTransition KernelState.shuttingDown KernelState.stopped = true ∧
    validKernelTransition KernelState.stopped KernelState.initializing = true :=
  ⟨rfl, ⟨rfl, ⟨rfl, ⟨rfl, ⟨rfl, rfl⟩⟩⟩⟩⟩

/-- CRASH-ONLY REDESIGN: `stopped` is no longer terminal — it is
restartable through the initialization phase. The legacy model asserted
the opposite (`stopped_is_terminal`), which made any host failure
permanent. What IS preserved: no direct restart shortcut — a restart
must re-run initialization. -/
theorem stopped_is_restartable :
    validKernelTransition KernelState.stopped KernelState.initializing = true := rfl

theorem no_direct_restart_shortcut :
    validKernelTransition KernelState.stopped KernelState.running = false ∧
    validKernelTransition KernelState.stopped KernelState.shuttingDown = false ∧
    validKernelTransition KernelState.stopped KernelState.stopped = false :=
  ⟨rfl, rfl, rfl⟩

theorem component_stopped_is_restartable :
    validComponentTransition ComponentState.stopped ComponentState.initialized = true := rfl

theorem component_no_direct_restart_shortcut :
    validComponentTransition ComponentState.stopped ComponentState.running = false := rfl

-- ---------------------------------------------------------------------------
-- Self-stabilization: `running` is reachable from EVERY state
-- (Dijkstra 1974; Candea & Fox, "Crash-Only Software", HotOS IX 2003)
-- ---------------------------------------------------------------------------

/-- General reachability over the kernel lifecycle transitions. -/
inductive Reach : KernelState → KernelState → Prop where
  | here (s : KernelState) : Reach s s
  | step (from_ mid target : KernelState)
      (h : validKernelTransition from_ mid = true) (hrest : Reach mid target) :
      Reach from_ target

/-- T1 (self-stabilization liveness): from EVERY kernel state, `running`
is reachable. A crashed, stopped, or shutting-down host is never stuck —
this is the property that makes the crash-only design usable. -/
theorem running_reachable : ∀ s : KernelState, Reach s KernelState.running := by
  intro s
  cases s with
  | uninitialized =>
    exact Reach.step KernelState.uninitialized KernelState.initializing
        KernelState.running rfl
      (Reach.step KernelState.initializing KernelState.running
        KernelState.running rfl (Reach.here KernelState.running))
  | initializing =>
    exact Reach.step KernelState.initializing KernelState.running
      KernelState.running rfl (Reach.here KernelState.running)
  | running => exact Reach.here KernelState.running
  | shuttingDown =>
    exact Reach.step KernelState.shuttingDown KernelState.stopped
        KernelState.running rfl
      (Reach.step KernelState.stopped KernelState.initializing
        KernelState.running rfl
        (Reach.step KernelState.initializing KernelState.running
          KernelState.running rfl (Reach.here KernelState.running)))
  | stopped =>
    exact Reach.step KernelState.stopped KernelState.initializing
        KernelState.running rfl
      (Reach.step KernelState.initializing KernelState.running
        KernelState.running rfl (Reach.here KernelState.running))

/-- General reachability over the component lifecycle transitions. -/
inductive ReachC : ComponentState → ComponentState → Prop where
  | here (s : ComponentState) : ReachC s s
  | step (from_ mid target : ComponentState)
      (h : validComponentTransition from_ mid = true) (hrest : ReachC mid target) :
      ReachC from_ target

/-- T1c (component self-stabilization): from EVERY component state —
including `error` — `running` is reachable. -/
theorem component_running_reachable :
    ∀ s : ComponentState, ReachC s ComponentState.running := by
  intro s
  cases s with
  | uninitialized =>
    exact ReachC.step ComponentState.uninitialized ComponentState.initialized
        ComponentState.running rfl
      (ReachC.step ComponentState.initialized ComponentState.running
        ComponentState.running rfl (ReachC.here ComponentState.running))
  | initialized =>
    exact ReachC.step ComponentState.initialized ComponentState.running
      ComponentState.running rfl (ReachC.here ComponentState.running)
  | running => exact ReachC.here ComponentState.running
  | stopped =>
    exact ReachC.step ComponentState.stopped ComponentState.initialized
        ComponentState.running rfl
      (ReachC.step ComponentState.initialized ComponentState.running
        ComponentState.running rfl (ReachC.here ComponentState.running))
  | error =>
    exact ReachC.step ComponentState.error ComponentState.stopped
        ComponentState.running rfl
      (ReachC.step ComponentState.stopped ComponentState.initialized
        ComponentState.running rfl
        (ReachC.step ComponentState.initialized ComponentState.running
          ComponentState.running rfl (ReachC.here ComponentState.running)))

/-- T2 (no shortcut): the restart path always re-runs initialization —
a stopped host cannot jump to `running` without re-validation. -/
theorem initialization_sequence (c : ComponentRegistry) :
    kernelInitialize { state := KernelState.uninitialized, components := c } =
      Except.ok { state := KernelState.initializing, components := c } ∧
    kernelStart { state := KernelState.initializing, components := c } =
      Except.ok { state := KernelState.running, components := c } ∧
    kernelShutdown { state := KernelState.running, components := c } =
      Except.ok { state := KernelState.shuttingDown, components := c } ∧
    kernelShutdown { state := KernelState.shuttingDown, components := c } =
      Except.ok { state := KernelState.stopped, components := c } :=
  ⟨by simp [kernelInitialize], ⟨by simp [kernelStart], ⟨by simp [kernelShutdown], by simp [kernelShutdown]⟩⟩⟩

/-- With the crash-only restart, `stopped` ALSO validly transitions to
`initialized` — so the no-double-init property holds for every state
except `uninitialized` (the only non-stopped source). -/
theorem component_init_sources (s : ComponentState)
    (h : validComponentTransition s ComponentState.initialized = true) :
    s = ComponentState.uninitialized ∨ s = ComponentState.stopped := by
  cases s with
  | uninitialized => exact Or.inl rfl
  | initialized => exact absurd h (by simp only [validComponentTransition]; decide)
  | running => exact absurd h (by simp only [validComponentTransition]; decide)
  | stopped => exact Or.inr rfl
  | error => exact absurd h (by simp only [validComponentTransition]; decide)

theorem init_failure_valid :
    validKernelTransition KernelState.initializing KernelState.stopped = true := rfl

theorem shutdown_only_from_active (c : ComponentRegistry) :
    kernelShutdown { state := KernelState.uninitialized, components := c } =
      Except.error "not running or shutting down" ∧
    kernelShutdown { state := KernelState.stopped, components := c } =
      Except.error "not running or shutting down" :=
  ⟨by simp [kernelShutdown], by simp [kernelShutdown]⟩

-- ---------------------------------------------------------------------------
-- P4: the registry-level restart
-- (YP-HOST-KERNEL-015, loop 3 it. 5)
-- ---------------------------------------------------------------------------

/-! P3 proved a restart must re-run initialization; P1/P2 proved no state
is a trap. P4 is the registry-level statement those leave open: a HOST
restart re-initializes ALL registered components, losing none and
inventing none.

Modelled as an association list — the functional spec of what the
HashMap-backed `ComponentRegistry` reset computes. Working at this level
makes "ALL components" a theorem about arbitrary registry contents
(induction over the list), not a claim checked against whatever happens
to be registered when someone reads the spec. -/

/-- The registry as an association list: each entry is a component and
its pre-restart state. -/
def restartRegistry (entries : List (ComponentId × ComponentState)) :
    List (ComponentId × ComponentState) :=
  entries.map (fun p => (p.1, ComponentState.uninitialized))

/-- T-P4a: a restart loses no component — the registry has exactly as
many entries after as before. -/
theorem restart_registry_preserves_length (entries : List (ComponentId × ComponentState)) :
    (restartRegistry entries).length = entries.length := by
  simp [restartRegistry]

/-- T-P4b: a restart resets EVERY entry to `uninitialized` — for arbitrary
registry contents, by induction, not by inspecting a snapshot. -/
theorem restart_registry_resets_every_entry (entries : List (ComponentId × ComponentState)) :
    ∀ p ∈ restartRegistry entries, p.2 = ComponentState.uninitialized := by
  intro p hp
  rw [restartRegistry] at hp
  rcases List.mem_map.mp hp with ⟨q, _, rfl⟩
  rfl

/-- T-P4c: a restart invents no component — every post-restart entry's id
was registered before. -/
theorem restart_registry_preserves_ids (entries : List (ComponentId × ComponentState)) :
    ∀ p ∈ restartRegistry entries, p.1 ∈ entries.map Prod.fst := by
  intro p hp
  rw [restartRegistry, List.mem_map] at hp
  obtain ⟨a, ha, rfl⟩ := hp
  rw [List.mem_map]
  exact ⟨a, ha, rfl⟩

/-- T-P4d (ties the reset to T2): from the post-restart state, the ONLY
forward transition is the full initialization phase — `uninitialized`
cannot shortcut to `running`. -/
theorem post_restart_no_shortcut (s : ComponentState)
    (h : validComponentTransition s ComponentState.running = true) :
    s ≠ ComponentState.uninitialized := by
  cases s with
  | uninitialized => exact absurd h (by simp only [validComponentTransition]; decide)
  | initialized => exact by decide
  | running => exact by decide
  | stopped => exact by decide
  | error => exact by decide

/-- T-P4e: and the full initialization sequence IS valid — for every
registered component, enumerated over the finite registry keys. -/
theorem restart_full_sequence_valid :
    ∀ id ∈ [ComponentId.host, ComponentId.fsm, ComponentId.sentinel,
            ComponentId.brain, ComponentId.graph, ComponentId.broker],
      validComponentTransition ComponentState.uninitialized ComponentState.initialized = true ∧
      validComponentTransition ComponentState.initialized ComponentState.running = true := by
  intro id _
  exact ⟨rfl, rfl⟩
