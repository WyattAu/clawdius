//! Host Kernel - Central orchestrator for Clawdius
//!
//! The Host Kernel is the trusted computing base (TCB) that coordinates all
//! subsystems, enforces the Nexus FSM lifecycle, and provides the runtime
//! environment for monoio-based asynchronous execution.
//!
//! # Architecture
//! - Runtime: monoio (`io_uring`, thread-per-core)
//! - Components: FSM, Sentinel, Brain, Graph-RAG, Broker
//! - Config: TOML-based
//! - Shutdown: Graceful with timeout

use crate::component::{ComponentId, ComponentInfo, ComponentState};
use crate::config::Config;
use crate::error::{ClawdiusError, Result};
use crate::fsm::StateMachine;
use monoio::time::TimeDriver;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Host kernel state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelState {
    /// Kernel is uninitialized
    Uninitialized,
    /// Kernel is initialized but not running
    Initialized,
    /// Kernel is running
    Running,
    /// Kernel is shutting down
    ShuttingDown,
    /// Kernel has stopped
    Stopped,
}

impl std::fmt::Display for KernelState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Uninitialized => write!(f, "uninitialized"),
            Self::Initialized => write!(f, "initialized"),
            Self::Running => write!(f, "running"),
            Self::ShuttingDown => write!(f, "shutting_down"),
            Self::Stopped => write!(f, "stopped"),
        }
    }
}

/// Metadata about the running kernel
#[derive(Debug)]
pub struct KernelMetadata {
    /// Unique session identifier
    pub session_id: Uuid,
    /// Kernel start time
    pub started_at: Instant,
    /// Current kernel state
    pub state: KernelState,
}

impl KernelMetadata {
    /// Create new kernel metadata
    #[must_use]
    pub fn new() -> Self {
        Self {
            session_id: Uuid::new_v4(),
            started_at: Instant::now(),
            state: KernelState::Uninitialized,
        }
    }

    /// Get uptime duration
    #[must_use]
    pub fn uptime(&self) -> Duration {
        self.started_at.elapsed()
    }
}

impl Default for KernelMetadata {
    fn default() -> Self {
        Self::new()
    }
}

/// Component registry for managing subsystems
#[derive(Debug)]
pub struct ComponentRegistry {
    /// Registered components by ID
    components: HashMap<ComponentId, ComponentInfo>,
    /// FSM component
    fsm: Option<StateMachine>,
}

impl ComponentRegistry {
    /// Create a new empty component registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
            fsm: None,
        }
    }

    /// Register a component
    pub fn register(&mut self, info: ComponentInfo) {
        self.components.insert(info.id, info);
    }

    /// Get component info by ID
    #[must_use]
    pub fn get(&self, id: ComponentId) -> Option<&ComponentInfo> {
        self.components.get(&id)
    }

    /// Get mutable component info by ID
    pub fn get_mut(&mut self, id: ComponentId) -> Option<&mut ComponentInfo> {
        self.components.get_mut(&id)
    }

    /// List all registered components
    pub fn list(&self) -> impl Iterator<Item = &ComponentInfo> {
        self.components.values()
    }

    /// Set the FSM component
    pub fn set_fsm(&mut self, fsm: StateMachine) {
        self.fsm = Some(fsm);
        self.register(ComponentInfo::new(
            ComponentId::FSM,
            "FSM",
            env!("CARGO_PKG_VERSION"),
        ));
    }

    /// Get the FSM component
    #[must_use]
    pub fn fsm(&self) -> Option<&StateMachine> {
        self.fsm.as_ref()
    }

    /// Get mutable FSM component
    pub fn fsm_mut(&mut self) -> Option<&mut StateMachine> {
        self.fsm.as_mut()
    }
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// The Host Kernel - central orchestrator for Clawdius
#[derive(Debug)]
pub struct Host {
    /// Configuration
    config: Config,
    /// Kernel metadata
    metadata: KernelMetadata,
    /// Component registry
    components: ComponentRegistry,
    /// Shutdown flag
    shutdown_requested: Arc<AtomicBool>,
}

impl Host {
    /// Create a new Host Kernel
    ///
    /// # Errors
    /// Returns an error if initialization fails.
    pub fn new(config: Config) -> Result<Self> {
        config.validate()?;

        let metadata = KernelMetadata::new();
        let components = ComponentRegistry::new();

        tracing::info!(
            session_id = %metadata.session_id,
            "Host kernel created"
        );

        Ok(Self {
            config,
            metadata,
            components,
            shutdown_requested: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Get the kernel configuration
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get the kernel metadata
    #[must_use]
    pub fn metadata(&self) -> &KernelMetadata {
        &self.metadata
    }

    /// Get the current kernel state
    #[must_use]
    pub fn state(&self) -> KernelState {
        self.metadata.state
    }

    /// Get the component registry
    #[must_use]
    pub fn components(&self) -> &ComponentRegistry {
        &self.components
    }

    /// Initialize the Host Kernel
    ///
    /// This initializes all registered components.
    ///
    /// # Errors
    /// Returns an error if initialization fails.
    pub fn initialize(&mut self) -> Result<()> {
        if self.metadata.state != KernelState::Uninitialized {
            return Err(ClawdiusError::Config(
                "Kernel already initialized".into(),
            ));
        }

        tracing::info!("Initializing host kernel...");

        // Initialize FSM component
        let fsm = StateMachine::new()?;
        self.components.set_fsm(fsm);

        // Update component info state
        if let Some(info) = self.components.get_mut(ComponentId::FSM) {
            info.state = ComponentState::Initialized;
        }

        self.metadata.state = KernelState::Initialized;

        tracing::info!("Host kernel initialized");
        Ok(())
    }

    /// Run the Host Kernel event loop
    ///
    /// This method blocks until shutdown is requested or an error occurs.
    ///
    /// # Errors
    /// Returns an error if the event loop encounters a fatal error.
    pub async fn run(&mut self) -> Result<()> {
        if self.metadata.state == KernelState::Uninitialized {
            return Err(ClawdiusError::Config(
                "Kernel not initialized. Call initialize() first.".into(),
            ));
        }

        if self.metadata.state == KernelState::Running {
            return Err(ClawdiusError::Config("Kernel already running".into()));
        }

        self.metadata.state = KernelState::Running;

        // FSM is already started when created
        if let Some(info) = self.components.get_mut(ComponentId::FSM) {
            info.state = ComponentState::Running;
        }

        tracing::info!(
            session_id = %self.metadata.session_id,
            "Host kernel running"
        );

        // Main event loop
        self.event_loop().await
    }

    /// Main event loop
    async fn event_loop(&mut self) -> Result<()> {
        loop {
            // Check for shutdown
            if self.shutdown_requested.load(Ordering::Relaxed) {
                tracing::info!("Shutdown requested");
                break;
            }

            // Process FSM tick
            if let Some(fsm) = self.components.fsm_mut() {
                use crate::fsm::TransitionResult;

                match fsm.tick() {
                    TransitionResult::Continue => {
                        // Continue processing
                    }
                    TransitionResult::Transition(new_phase) => {
                        tracing::info!("Phase transition: {}", new_phase);
                    }
                    TransitionResult::Complete => {
                        tracing::info!("State machine completed all phases");
                        break;
                    }
                    TransitionResult::Error(e) => {
                        tracing::error!("State machine error: {}", e);
                        return Err(e);
                    }
                }
            }

            // Small sleep to yield to other tasks
            monoio::time::sleep(std::time::Duration::from_millis(1)).await;
        }

        Ok(())
    }

    /// Request shutdown of the Host Kernel
    pub fn request_shutdown(&self) {
        tracing::info!("Shutdown requested");
        self.shutdown_requested.store(true, Ordering::Relaxed);
    }

    /// Shutdown the Host Kernel gracefully
    ///
    /// # Errors
    /// Returns an error if shutdown fails.
    pub fn shutdown(&mut self) -> Result<()> {
        if self.metadata.state != KernelState::Running {
            return Err(ClawdiusError::Config(
                "Kernel not running".into(),
            ));
        }

        self.metadata.state = KernelState::ShuttingDown;
        tracing::info!("Shutting down host kernel...");

        // Stop FSM component
        if let Some(info) = self.components.get_mut(ComponentId::FSM) {
            info.state = ComponentState::Stopping;
        }

        // FSM doesn't have explicit stop, just mark as stopped
        if let Some(info) = self.components.get_mut(ComponentId::FSM) {
            info.state = ComponentState::Stopped;
        }

        self.metadata.state = KernelState::Stopped;

        tracing::info!(
            uptime_secs = %self.metadata.uptime().as_secs(),
            "Host kernel shutdown complete"
        );

        Ok(())
    }
}

/// Build the monoio runtime for the Host Kernel
///
/// # Errors
/// Returns an error if runtime creation fails.
pub fn build_runtime() -> Result<monoio::Runtime<TimeDriver<monoio::IoUringDriver>>> {
    monoio::RuntimeBuilder::<monoio::IoUringDriver>::new()
        .enable_timer()
        .build()
        .map_err(|e| ClawdiusError::Config(format!("Failed to create monoio runtime: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kernel_metadata() {
        let meta = KernelMetadata::new();
        assert_eq!(meta.state, KernelState::Uninitialized);
        assert!(meta.uptime() < Duration::from_secs(1));
    }

    #[test]
    fn test_component_registry() {
        let mut registry = ComponentRegistry::new();

        let info = ComponentInfo::new(ComponentId::HOST, "Test", "1.0.0");
        registry.register(info);

        assert!(registry.get(ComponentId::HOST).is_some());
        assert!(registry.get(ComponentId::FSM).is_none());
    }

    #[test]
    fn test_host_creation() {
        let config = Config::default();
        let host = Host::new(config);
        assert!(host.is_ok());

        let host = host.expect("Host creation failed");
        assert_eq!(host.state(), KernelState::Uninitialized);
    }

    #[test]
    fn test_host_initialize() {
        let config = Config::default();
        let mut host = Host::new(config).expect("Host creation failed");

        let result = host.initialize();
        assert!(result.is_ok());
        assert_eq!(host.state(), KernelState::Initialized);
    }

    #[test]
    fn test_host_double_initialize() {
        let config = Config::default();
        let mut host = Host::new(config).expect("Host creation failed");

        host.initialize().expect("First init failed");
        let result = host.initialize();
        assert!(result.is_err());
    }

    #[test]
    fn test_kernel_state_display() {
        assert_eq!(format!("{}", KernelState::Running), "running");
        assert_eq!(format!("{}", KernelState::ShuttingDown), "shutting_down");
    }
}
