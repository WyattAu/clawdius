/-
  Lean4 Proof: Host Kernel Lifecycle Safety
  Component: COMP-HOST-001
  Blue Paper: BP-HOST-KERNEL-001
  Yellow Paper: YP-HOST-KERNEL-001
-/

import Std.Data.HashMap

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

def validComponentTransition : ComponentState → ComponentState → Bool
  | ComponentState.uninitialized, ComponentState.initialized => true
  | ComponentState.initialized, ComponentState.running => true
  | ComponentState.running, ComponentState.stopped => true
  | ComponentState.running, ComponentState.error => true
  | ComponentState.error, ComponentState.stopped => true
  | _, _ => false

def validKernelTransition : KernelState → KernelState → Bool
  | KernelState.uninitialized, KernelState.initializing => true
  | KernelState.initializing, KernelState.running => true
  | KernelState.initializing, KernelState.stopped => true
  | KernelState.running, KernelState.shuttingDown => true
  | KernelState.shuttingDown, KernelState.stopped => true
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
    validKernelTransition KernelState.shuttingDown KernelState.stopped = true :=
  ⟨rfl, ⟨rfl, ⟨rfl, ⟨rfl, rfl⟩⟩⟩⟩

theorem stopped_is_terminal (s : KernelState) :
    validKernelTransition KernelState.stopped s = false := by
  cases s <;> rfl

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

theorem component_no_double_init (s : ComponentState) :
    validComponentTransition s ComponentState.initialized = true →
    s = ComponentState.uninitialized := by
  intro h
  cases s with
  | uninitialized => rfl
  | _ => simp only [validComponentTransition] at h; exact absurd h (by decide)

theorem init_failure_valid :
    validKernelTransition KernelState.initializing KernelState.stopped = true := rfl

theorem shutdown_only_from_active (c : ComponentRegistry) :
    kernelShutdown { state := KernelState.uninitialized, components := c } =
      Except.error "not running or shutting down" ∧
    kernelShutdown { state := KernelState.stopped, components := c } =
      Except.error "not running or shutting down" :=
  ⟨by simp [kernelShutdown], by simp [kernelShutdown]⟩
