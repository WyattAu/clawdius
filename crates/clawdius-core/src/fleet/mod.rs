//! Fleet — multi-repo dev-cycle management for the central agent.
//!
//! The fleet module lets one Clawdius instance supervise a whole tree of
//! repositories: it discovers repos under configured roots, keeps an
//! active/inactive registry, routes dev-cycle tasks to repos through a
//! single-lane-per-repo scheduler, and applies the hard-gate policy when a
//! cycle reports finish.
//!
//! # Gate policy
//!
//! Hard gates normally block a cycle from completing. With
//! [`GatePolicy::skip_on_finish`] enabled, a cycle that reports finish skips
//! its hard gates and every skipped gate is flagged into the review queue —
//! the dashboard surfaces those flags for human review instead of blocking
//! the fleet.
//!
//! # Persistence
//!
//! The registry and review queue serialize to one JSON document
//! (see [`FleetState::save`]/[`FleetState::load`]) so the TUI can restore the
//! fleet view across restarts.
//!
//! # Module layout
//!
//! - [`registry`] — repo discovery + active/inactive registry
//! - [`cycle`] — the per-repo dev-cycle state machine
//! - [`gates`] — hard-gate policy, skip-and-flag, review queue
//! - [`router`] — the central agent's task routing across repos

pub mod cycle;
pub mod gates;
pub mod registry;
pub mod router;

pub use cycle::{CyclePhase, DevCycle};
pub use gates::{GateDecision, GateId, GatePolicy, GateRecord, GateStatus, ReviewQueue};
pub use registry::{RepoEntry, RepoRegistry};
pub use router::{DispatchDecision, FleetRouter};

use serde::{Deserialize, Serialize};

/// The complete, persistable fleet state: registry + router queues + review
/// queue.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FleetState {
    /// Known repositories and their active/inactive flags.
    pub registry: RepoRegistry,
    /// Central-agent routing state.
    pub router: FleetRouter,
    /// Hard-gate policy applied when a cycle reports finish.
    pub gate_policy: GatePolicy,
}

impl FleetState {
    /// Create an empty fleet state with the default gate policy.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Serialize the fleet state to a JSON writer.
    ///
    /// # Errors
    ///
    /// Returns the underlying `serde_json` / IO error on failure.
    pub fn save<W: std::io::Write>(&self, writer: W) -> std::io::Result<()> {
        serde_json::to_writer_pretty(writer, self).map_err(std::io::Error::other)
    }

    /// Deserialize the fleet state from a JSON reader.
    ///
    /// # Errors
    ///
    /// Returns the underlying `serde_json` / IO error on failure.
    pub fn load<R: std::io::Read>(reader: R) -> std::io::Result<Self> {
        serde_json::from_reader(reader).map_err(std::io::Error::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fleet_state_roundtrips_through_json() {
        let mut state = FleetState::new();
        state
            .registry
            .upsert(RepoEntry::new("clawdius", "/dev/clawdius"));
        state.router.enqueue("clawdius", "fix the tests");
        let json = {
            let mut buf: Vec<u8> = Vec::new();
            state.save(&mut buf).unwrap();
            String::from_utf8(buf).unwrap()
        };
        let loaded = FleetState::load(json.as_bytes()).unwrap();
        assert_eq!(loaded.registry.entries.len(), 1);
        assert_eq!(loaded.router.queue_len("clawdius"), 1);
    }
}
