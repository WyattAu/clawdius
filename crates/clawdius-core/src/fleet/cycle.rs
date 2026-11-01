//! The per-repo dev-cycle state machine.
//!
//! A dev cycle is one unit of agent-driven work on a repository (a sprint, an
//! auto-fix pass, a generation job). The phase machine is deliberately small:
//!
//! ```text
//! Queued → Dispatched → Running → FinishReported → Completed | GatesSkipped
//!    ↑                                                    │
//!    └────────────── requeue on failure ◄─────────────────┘
//! ```
//!
//! `FinishReported` is where the [`GatePolicy`](crate::fleet::GatePolicy)
//! decides between `Completed` (gates passed / none required) and
//! `GatesSkipped` (hard gates skipped, each flagged for human review).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Lifecycle phase of one dev cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CyclePhase {
    /// Waiting for the central agent to dispatch it.
    Queued,
    /// Handed to an executor; the repo's single lane is now busy.
    Dispatched,
    /// Agent actively working the cycle.
    Running,
    /// The agent reported finish; gates are being evaluated.
    FinishReported,
    /// Finished with all required gates satisfied.
    Completed,
    /// Finished with hard gates skipped — flagged for human review.
    GatesSkipped,
    /// Failed and returned to the queue for a retry.
    Requeued,
}

impl CyclePhase {
    /// Whether the repo's dispatch lane is occupied by this phase.
    #[must_use]
    pub const fn occupies_lane(self) -> bool {
        matches!(
            self,
            Self::Dispatched | Self::Running | Self::FinishReported
        )
    }

    /// Whether the cycle reached a terminal phase.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::GatesSkipped)
    }
}

/// One dev cycle on one repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevCycle {
    /// Repo this cycle targets (registry name).
    pub repo: String,
    /// What the agent was asked to do.
    pub task: String,
    /// Current phase.
    pub phase: CyclePhase,
    /// Last phase transition.
    pub updated_at: DateTime<Utc>,
    /// Human-readable result/notes from the agent or gate evaluation.
    pub note: Option<String>,
}

impl DevCycle {
    /// Create a queued cycle.
    #[must_use]
    pub fn queued(repo: impl Into<String>, task: impl Into<String>) -> Self {
        Self {
            repo: repo.into(),
            task: task.into(),
            phase: CyclePhase::Queued,
            updated_at: Utc::now(),
            note: None,
        }
    }

    /// Advance the phase; rejects illegal transitions by returning `false`.
    pub fn transition(&mut self, next: CyclePhase) -> bool {
        let legal = match (self.phase, next) {
            (CyclePhase::Queued | CyclePhase::Requeued, CyclePhase::Dispatched)
            | (CyclePhase::Dispatched, CyclePhase::Running)
            // A synchronous executor may finish straight from dispatch.
            | (
                CyclePhase::Dispatched | CyclePhase::Running,
                CyclePhase::FinishReported,
            )
            | (
                CyclePhase::FinishReported,
                CyclePhase::Completed | CyclePhase::GatesSkipped | CyclePhase::Requeued,
            )
            // Terminal cycles may be restarted into the queue.
            | (
                CyclePhase::Completed | CyclePhase::GatesSkipped,
                CyclePhase::Queued,
            ) => true,
            _ => false,
        };
        if legal {
            self.phase = next;
            self.updated_at = Utc::now();
        }
        legal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_transitions() {
        let mut cycle = DevCycle::queued("r", "task");
        assert!(cycle.transition(CyclePhase::Dispatched));
        assert!(cycle.transition(CyclePhase::Running));
        assert!(cycle.transition(CyclePhase::FinishReported));
        assert!(cycle.transition(CyclePhase::Completed));
        assert!(cycle.phase.is_terminal());
    }

    #[test]
    fn illegal_transitions_rejected() {
        let mut cycle = DevCycle::queued("r", "task");
        assert!(
            !cycle.transition(CyclePhase::Running),
            "must dispatch first"
        );
        assert_eq!(cycle.phase, CyclePhase::Queued);
    }

    #[test]
    fn terminal_cycles_can_restart() {
        let mut cycle = DevCycle::queued("r", "task");
        assert!(cycle.transition(CyclePhase::Dispatched));
        assert!(cycle.transition(CyclePhase::Running));
        assert!(cycle.transition(CyclePhase::FinishReported));
        assert!(cycle.transition(CyclePhase::GatesSkipped));
        assert!(cycle.transition(CyclePhase::Queued));
    }

    #[test]
    fn lane_occupied_only_while_in_flight() {
        assert!(!CyclePhase::Queued.occupies_lane());
        assert!(CyclePhase::Running.occupies_lane());
        assert!(CyclePhase::FinishReported.occupies_lane());
        assert!(!CyclePhase::Completed.occupies_lane());
    }
}
