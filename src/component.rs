//! Component trait and types for Clawdius subsystems
//!
//! Defines the interface that all major components (FSM, Sentinel, Brain, Graph)
//! must implement for lifecycle management by the Host Kernel.

use crate::error::Result;
use std::fmt::Debug;

/// Unique identifier for a component
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentId(pub u16);

impl ComponentId {
    /// Host kernel component ID
    pub const HOST: Self = Self(0x0001);
    /// FSM component ID
    pub const FSM: Self = Self(0x0002);
    /// Sentinel sandbox component ID
    pub const SENTINEL: Self = Self(0x0003);
    /// Brain WASM runtime component ID
    pub const BRAIN: Self = Self(0x0004);
    /// Graph-RAG component ID
    pub const GRAPH: Self = Self(0x0005);
    /// Broker HFT component ID
    pub const BROKER: Self = Self(0x0006);
}

impl std::fmt::Display for ComponentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match *self {
            Self::HOST => "Host",
            Self::FSM => "FSM",
            Self::SENTINEL => "Sentinel",
            Self::BRAIN => "Brain",
            Self::GRAPH => "Graph",
            Self::BROKER => "Broker",
            _ => "Unknown",
        };
        write!(f, "{}({:#06x})", name, self.0)
    }
}

/// Current state of a component
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComponentState {
    /// Component is uninitialized
    #[default]
    Uninitialized,
    /// Component is initialized but not running
    Initialized,
    /// Component is running
    Running,
    /// Component is stopping
    Stopping,
    /// Component has stopped
    Stopped,
    /// Component encountered an error
    Error,
}

impl std::fmt::Display for ComponentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Uninitialized => write!(f, "uninitialized"),
            Self::Initialized => write!(f, "initialized"),
            Self::Running => write!(f, "running"),
            Self::Stopping => write!(f, "stopping"),
            Self::Stopped => write!(f, "stopped"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Trait that all Clawdius components must implement
///
/// Components are managed by the Host Kernel and follow a strict lifecycle:
/// 1. `new()` - Create the component
/// 2. `initialize()` - Perform initialization (load config, allocate resources)
/// 3. `start()` - Begin operation
/// 4. `stop()` - Gracefully shutdown
pub trait Component: Send + Sync + Debug {
    /// Returns the component's unique identifier
    fn id(&self) -> ComponentId;

    /// Returns the component's name for logging
    fn name(&self) -> &'static str;

    /// Returns the current state of the component
    fn state(&self) -> ComponentState;

    /// Initialize the component
    ///
    /// Called once after construction. Should allocate resources
    /// and prepare for operation but not start processing.
    ///
    /// # Errors
    /// Returns an error if initialization fails.
    fn initialize(&mut self) -> Result<()>;

    /// Start the component
    ///
    /// Called after initialization. Should begin processing.
    ///
    /// # Errors
    /// Returns an error if startup fails.
    fn start(&mut self) -> Result<()>;

    /// Stop the component gracefully
    ///
    /// Should complete in-flight operations and release resources.
    ///
    /// # Errors
    /// Returns an error if shutdown fails.
    fn stop(&mut self) -> Result<()>;
}

/// Metadata about a component
#[derive(Debug, Clone)]
pub struct ComponentInfo {
    /// Component identifier
    pub id: ComponentId,
    /// Component name
    pub name: &'static str,
    /// Current state
    pub state: ComponentState,
    /// Component version
    pub version: &'static str,
}

impl ComponentInfo {
    /// Create new component info
    #[must_use]
    pub const fn new(id: ComponentId, name: &'static str, version: &'static str) -> Self {
        Self {
            id,
            name,
            state: ComponentState::Uninitialized,
            version,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_id_display() {
        assert_eq!(format!("{}", ComponentId::HOST), "Host(0x0001)");
        assert_eq!(format!("{}", ComponentId::FSM), "FSM(0x0002)");
    }

    #[test]
    fn test_component_state_default() {
        let state = ComponentState::default();
        assert_eq!(state, ComponentState::Uninitialized);
    }

    #[test]
    fn test_component_state_display() {
        assert_eq!(format!("{}", ComponentState::Running), "running");
        assert_eq!(format!("{}", ComponentState::Error), "error");
    }
}
