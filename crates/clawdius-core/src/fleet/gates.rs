//! Hard-gate policy: skip-and-flag on finish, with a human review queue.
//!
//! A *hard gate* is a check that normally must pass before a dev cycle may
//! complete (build, full test suite, clippy, proof build, audit). With the
//! skip policy enabled, a cycle that reports finish does not block on its
//! hard gates: each gate is recorded as [`GateStatus::AwaitingReview`] and
//! appended to the review queue, and the cycle ends in
//! [`CyclePhase::GatesSkipped`](crate::fleet::CyclePhase::GatesSkipped).
//! The dashboard surfaces the queue; a human resolves each record.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Identifier of a fleet hard gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GateId {
    /// Workspace compiles.
    Build,
    /// Full test suite green.
    Tests,
    /// Clippy with the workspace deny set.
    Clippy,
    /// Formal proof estate builds (`lake build`, 0 sorries).
    Proofs,
    /// Supply-chain audit (`cargo audit` / vet) clean.
    Audit,
}

impl GateId {
    /// Short human-readable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Tests => "tests",
            Self::Clippy => "clippy",
            Self::Proofs => "proofs",
            Self::Audit => "audit",
        }
    }

    /// Every gate known to the fleet, in dashboard order.
    #[must_use]
    pub const fn all() -> [Self; 5] {
        [
            Self::Build,
            Self::Tests,
            Self::Clippy,
            Self::Proofs,
            Self::Audit,
        ]
    }
}

/// Review status of one gate record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateStatus {
    /// Skipped by policy; waiting for human review.
    AwaitingReview,
    /// A human reviewed and accepted the skip.
    Approved,
    /// A human reviewed and rejected the skip (cycle should be requeued).
    Rejected,
}

/// One flagged gate on one repo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateRecord {
    /// Repo the flag belongs to.
    pub repo: String,
    /// Which gate was skipped.
    pub gate: GateId,
    /// Review status.
    pub status: GateStatus,
    /// Why the gate was skipped.
    pub reason: String,
    /// When the record entered the queue.
    pub flagged_at: DateTime<Utc>,
}

/// What the policy decided for a finished cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateDecision {
    /// No hard gates pending; cycle completes normally.
    Complete,
    /// Hard gates were skipped; records were appended to the review queue.
    SkippedAndFlagged(Vec<GateRecord>),
}

/// The fleet's hard-gate policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatePolicy {
    /// Gates that count as hard for the fleet.
    pub hard_gates: Vec<GateId>,
    /// When true, a finish report skips hard gates and flags them for
    /// review instead of failing the cycle.
    pub skip_on_finish: bool,
}

impl Default for GatePolicy {
    fn default() -> Self {
        Self {
            hard_gates: GateId::all().to_vec(),
            skip_on_finish: false,
        }
    }
}

impl GatePolicy {
    /// Evaluate a finish report for a repo.
    ///
    /// `passed` lists the hard gates the cycle already satisfied. Without
    /// skip-on-finish, any unmet hard gate fails the cycle. With it, every
    /// unmet hard gate is flagged into the review queue and the fleet keeps
    /// moving.
    #[must_use]
    pub fn on_finish(&self, repo: &str, passed: &[GateId], reason: &str) -> GateDecision {
        let unmet: Vec<GateId> = self
            .hard_gates
            .iter()
            .copied()
            .filter(|g| !passed.contains(g))
            .collect();
        if unmet.is_empty() {
            return GateDecision::Complete;
        }
        if !self.skip_on_finish {
            let labels = unmet
                .iter()
                .map(|g| g.label())
                .collect::<Vec<_>>()
                .join(", ");
            return GateDecision::SkippedAndFlagged(vec![GateRecord {
                repo: repo.to_string(),
                gate: unmet[0],
                status: GateStatus::Rejected,
                reason: format!("hard gates not satisfied: {labels}"),
                flagged_at: Utc::now(),
            }]);
        }
        let records = unmet
            .into_iter()
            .map(|gate| GateRecord {
                repo: repo.to_string(),
                gate,
                status: GateStatus::AwaitingReview,
                reason: reason.to_string(),
                flagged_at: Utc::now(),
            })
            .collect();
        GateDecision::SkippedAndFlagged(records)
    }
}

/// The dashboard's review queue: every flagged hard gate awaiting a human.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReviewQueue {
    /// Flagged records, oldest first.
    pub records: Vec<GateRecord>,
}

impl ReviewQueue {
    /// Create an empty queue.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Append records produced by a finish report.
    pub fn extend(&mut self, records: impl IntoIterator<Item = GateRecord>) {
        self.records.extend(records);
    }

    /// Records still awaiting review (skipped-only view for the dashboard).
    #[must_use]
    pub fn pending(&self) -> Vec<&GateRecord> {
        self.records
            .iter()
            .filter(|r| r.status == GateStatus::AwaitingReview)
            .collect()
    }

    /// Resolve one pending record for `(repo, gate)`; returns true if found.
    pub fn resolve(&mut self, repo: &str, gate: GateId, approved: bool) -> bool {
        let status = if approved {
            GateStatus::Approved
        } else {
            GateStatus::Rejected
        };
        let Some(record) = self
            .records
            .iter_mut()
            .find(|r| r.status == GateStatus::AwaitingReview && r.repo == repo && r.gate == gate)
        else {
            return false;
        };
        record.status = status;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn complete_when_all_hard_gates_pass() {
        let policy = GatePolicy::default();
        let decision = policy.on_finish("r", &GateId::all(), "clean run");
        assert_eq!(decision, GateDecision::Complete);
    }

    #[test]
    fn blocking_policy_flags_rejected_when_skip_disabled() {
        let policy = GatePolicy::default();
        let decision = policy.on_finish("r", &[GateId::Build], "agent says done");
        let GateDecision::SkippedAndFlagged(records) = decision else {
            panic!("expected flags");
        };
        assert_eq!(records[0].status, GateStatus::Rejected);
    }

    #[test]
    fn skip_policy_flags_every_unmet_gate_for_review() {
        let policy = GatePolicy {
            skip_on_finish: true,
            ..GatePolicy::default()
        };
        let decision = policy.on_finish("r", &[GateId::Build], "agent says done");
        let GateDecision::SkippedAndFlagged(records) = decision else {
            panic!("expected flags");
        };
        assert_eq!(records.len(), 4, "tests/clippy/proofs/audit flagged");
        assert!(records
            .iter()
            .all(|r| r.status == GateStatus::AwaitingReview));
    }

    #[test]
    fn review_queue_resolve_targets_pending_record() {
        let policy = GatePolicy {
            skip_on_finish: true,
            ..GatePolicy::default()
        };
        let mut queue = ReviewQueue::new();
        let GateDecision::SkippedAndFlagged(records) = policy.on_finish("r", &[], "done") else {
            panic!("expected flags");
        };
        queue.extend(records);
        assert_eq!(queue.pending().len(), GateId::all().len());
        assert!(queue.resolve("r", GateId::Proofs, true));
        assert_eq!(queue.pending().len(), GateId::all().len() - 1);
        assert!(
            !queue.resolve("r", GateId::Proofs, true),
            "already resolved"
        );
    }

    proptest! {
        /// skip_on_finish flags exactly the hard gates not in `passed` —
        /// nothing more, nothing less — in hard-gate order. A fully-passed
        /// report is Complete under either policy; with skip disabled the
        /// single unmet-gate record is Rejected, not AwaitingReview.
        #[test]
        fn prop_skip_on_finish_flags_exactly_unmet(
            hard in proptest::collection::vec(any::<bool>(), 5usize),
            passed in proptest::collection::vec(any::<bool>(), 5usize),
            skip in any::<bool>(),
        ) {
            let hard_gates: Vec<GateId> = GateId::all()
                .iter()
                .zip(&hard)
                .filter(|(_, h)| **h)
                .map(|(g, _)| *g)
                .collect();
            let passed_gates: Vec<GateId> = GateId::all()
                .iter()
                .zip(&passed)
                .filter(|(_, p)| **p)
                .map(|(g, _)| *g)
                .collect();
            let policy = GatePolicy {
                hard_gates: hard_gates.clone(),
                skip_on_finish: skip,
            };
            let unmet: Vec<GateId> = hard_gates
                .iter()
                .copied()
                .filter(|g| !passed_gates.contains(g))
                .collect();

            let decision = policy.on_finish("r", &passed_gates, "why");
            if unmet.is_empty() {
                assert_eq!(decision, GateDecision::Complete);
            } else {
                let GateDecision::SkippedAndFlagged(records) = decision else {
                    panic!("expected flags for unmet gates");
                };
                assert!(!records.is_empty());
                assert!(records.iter().all(|r| r.repo == "r"));
                if skip {
                    let flagged: Vec<GateId> = records.iter().map(|r| r.gate).collect();
                    assert_eq!(flagged, unmet, "flagged set must equal the unmet set");
                    assert!(records
                        .iter()
                        .all(|r| r.reason == "why" && r.status == GateStatus::AwaitingReview));
                } else {
                    assert_eq!(records.len(), 1, "blocking policy flags one record");
                    assert_eq!(records[0].gate, unmet[0]);
                    assert_eq!(records[0].status, GateStatus::Rejected);
                }
            }
        }

        /// resolve flips only the first pending record matching
        /// (repo, gate): every other record is untouched, repeated
        /// resolves walk duplicate matches one at a time, and resolving
        /// an exhausted pair changes nothing and returns false.
        #[test]
        fn prop_resolve_only_matching_record(
            entries in proptest::collection::vec((0usize..2, 0usize..5), 1usize..15),
            target_repo in 0usize..2,
            target_gate in 0usize..5,
            approved in any::<bool>(),
        ) {
            let repos = ["alpha", "beta"];
            let gates = GateId::all();
            let mut queue = ReviewQueue::new();
            for (r, g) in &entries {
                queue.extend(vec![GateRecord {
                    repo: repos[*r].to_string(),
                    gate: gates[*g],
                    status: GateStatus::AwaitingReview,
                    reason: "x".to_string(),
                    flagged_at: Utc::now(),
                }]);
            }

            let repo = repos[target_repo];
            let gate = gates[target_gate];
            let matching: Vec<usize> = queue
                .records
                .iter()
                .enumerate()
                .filter(|(_, rec)| {
                    rec.status == GateStatus::AwaitingReview
                        && rec.repo == repo
                        && rec.gate == gate
                })
                .map(|(i, _)| i)
                .collect();
            let expected_status = if approved {
                GateStatus::Approved
            } else {
                GateStatus::Rejected
            };

            let mut flips = 0usize;
            loop {
                let before: Vec<GateStatus> =
                    queue.records.iter().map(|rec| rec.status).collect();
                let found = queue.resolve(repo, gate, approved);
                assert_eq!(found, flips < matching.len());
                let after: Vec<GateStatus> =
                    queue.records.iter().map(|rec| rec.status).collect();
                if found {
                    let idx = matching[flips];
                    for i in 0..before.len() {
                        if i == idx {
                            assert_eq!(after[i], expected_status);
                        } else {
                            assert_eq!(
                                after[i], before[i],
                                "resolve touched non-matching record {i}"
                            );
                        }
                    }
                    flips += 1;
                } else {
                    assert_eq!(after, before, "unsuccessful resolve changed records");
                    break;
                }
            }
            assert_eq!(flips, matching.len());
        }
    }
}
