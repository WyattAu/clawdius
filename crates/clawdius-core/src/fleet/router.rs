//! The central agent's router: fair dev-cycle dispatch across the fleet.
//!
//! One dispatch lane per repository — at most one cycle is in flight per repo
//! at a time (the lane model keeps a repo's worktree single-writer). Among
//! repos with queued work and a free lane, the router dispatches the repo
//! least recently dispatched (never-dispatched repos first), which yields
//! round-robin fairness without timers.

use std::collections::{BTreeMap, VecDeque};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::cycle::{CyclePhase, DevCycle};
use super::gates::{GateDecision, GateId, GatePolicy, ReviewQueue};

/// A dispatch handed to an executor by [`FleetRouter::dispatch_next`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchDecision {
    /// Repo to work on.
    pub repo: String,
    /// Task text for the agent.
    pub task: String,
}

/// Central-agent routing state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FleetRouter {
    /// Queued task texts per repo (front = next to run).
    queues: BTreeMap<String, VecDeque<String>>,
    /// Current cycle phase per repo (the lane).
    lanes: BTreeMap<String, CyclePhase>,
    /// Last dispatch time per repo, for fairness ordering.
    last_dispatched: BTreeMap<String, DateTime<Utc>>,
}

impl FleetRouter {
    /// Create an empty router.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            queues: BTreeMap::new(),
            lanes: BTreeMap::new(),
            last_dispatched: BTreeMap::new(),
        }
    }

    /// Enqueue a dev-cycle task for a repo.
    pub fn enqueue(&mut self, repo: &str, task: &str) {
        self.queues
            .entry(repo.to_string())
            .or_default()
            .push_back(task.to_string());
        self.lanes
            .entry(repo.to_string())
            .or_insert(CyclePhase::Queued);
    }

    /// Number of queued tasks for a repo.
    #[must_use]
    pub fn queue_len(&self, repo: &str) -> usize {
        self.queues.get(repo).map_or(0, VecDeque::len)
    }

    /// Current lane phase for a repo.
    #[must_use]
    pub fn lane(&self, repo: &str) -> Option<CyclePhase> {
        self.lanes.get(repo).copied()
    }

    /// Pick the next dispatch: active repos only, free lanes only,
    /// least-recently-dispatched first.
    #[must_use]
    pub fn dispatch_next(&mut self, active_repos: &[String]) -> Option<DispatchDecision> {
        let mut candidates: Vec<&String> = active_repos
            .iter()
            .filter(|repo| {
                self.queue_len(repo) > 0 && !self.lane(repo).is_some_and(CyclePhase::occupies_lane)
            })
            .collect();
        candidates.sort_by_key(|repo| self.last_dispatched.get(*repo));
        let repo: String = candidates.first().copied()?.clone();
        let queue = self.queues.get_mut(repo.as_str())?;
        let task = queue.pop_front()?;
        self.lanes.insert(repo.clone(), CyclePhase::Dispatched);
        self.last_dispatched.insert(repo.clone(), Utc::now());
        Some(DispatchDecision { repo, task })
    }

    /// Mark the repo's in-flight cycle as running by the executor.
    pub fn mark_running(&mut self, repo: &str) -> bool {
        self.transition_lane(repo, CyclePhase::Running)
    }

    /// Report a finish from the agent; applies `policy`, appends any gate
    /// flags to `review`, and returns the gate decision.
    pub fn report_finish(
        &mut self,
        repo: &str,
        passed: &[GateId],
        reason: &str,
        policy: &GatePolicy,
        review: &mut ReviewQueue,
    ) -> Option<GateDecision> {
        if !self.transition_lane(repo, CyclePhase::FinishReported) {
            return None;
        }
        let decision = policy.on_finish(repo, passed, reason);
        let next = match &decision {
            GateDecision::Complete => CyclePhase::Completed,
            GateDecision::SkippedAndFlagged(records) => {
                if policy.skip_on_finish {
                    review.extend(records.clone());
                    CyclePhase::GatesSkipped
                } else {
                    // Blocking policy: a rejected record means the cycle
                    // failed its gates — requeue for another pass.
                    CyclePhase::Requeued
                }
            },
        };
        self.transition_lane(repo, next);
        Some(decision)
    }

    /// Report an execution failure; the lane returns to requeued state.
    pub fn report_failure(&mut self, repo: &str) -> bool {
        self.transition_lane(repo, CyclePhase::Requeued)
    }

    /// Repos with at least one queued task.
    #[must_use]
    pub fn pending_repos(&self) -> Vec<String> {
        self.queues
            .iter()
            .filter(|(_, q)| !q.is_empty())
            .map(|(repo, _)| repo.clone())
            .collect()
    }

    fn transition_lane(&mut self, repo: &str, next: CyclePhase) -> bool {
        let Some(lane) = self.lanes.get_mut(repo) else {
            return false;
        };
        let mut probe = DevCycle {
            repo: repo.to_string(),
            task: String::new(),
            phase: *lane,
            updated_at: Utc::now(),
            note: None,
        };
        if probe.transition(next) {
            *lane = next;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    use crate::fleet::gates::GateStatus;

    fn active(names: &[&str]) -> Vec<String> {
        names.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn dispatches_round_robin_across_repos() {
        let mut router = FleetRouter::new();
        router.enqueue("a", "task-a1");
        router.enqueue("a", "task-a2");
        router.enqueue("b", "task-b1");
        let repos = active(&["a", "b"]);

        let first = router.dispatch_next(&repos).unwrap();
        assert_eq!(first.repo, "a");
        let second = router.dispatch_next(&repos).unwrap();
        assert_eq!(second.repo, "b", "a's lane is busy; b goes next");
        assert!(router.dispatch_next(&repos).is_none(), "both lanes busy");
    }

    #[test]
    fn inactive_repos_are_skipped() {
        let mut router = FleetRouter::new();
        router.enqueue("a", "task-a");
        assert!(router.dispatch_next(&[]).is_none());
    }

    #[test]
    fn finish_with_skip_policy_moves_to_gates_skipped() {
        let mut router = FleetRouter::new();
        let mut review = ReviewQueue::new();
        let policy = GatePolicy {
            skip_on_finish: true,
            ..GatePolicy::default()
        };
        router.enqueue("r", "task");
        router.dispatch_next(&active(&["r"])).unwrap();
        router.mark_running("r");
        let decision = router
            .report_finish("r", &[], "done", &policy, &mut review)
            .unwrap();
        assert!(matches!(decision, GateDecision::SkippedAndFlagged(_)));
        assert_eq!(router.lane("r"), Some(CyclePhase::GatesSkipped));
        assert_eq!(review.pending().len(), GateId::all().len());
    }

    #[test]
    fn blocking_policy_requeues_on_unmet_gates() {
        let mut router = FleetRouter::new();
        let mut review = ReviewQueue::new();
        let policy = GatePolicy::default();
        router.enqueue("r", "task");
        router.dispatch_next(&active(&["r"])).unwrap();
        let decision = router
            .report_finish("r", &[], "done", &policy, &mut review)
            .unwrap();
        assert_eq!(router.lane("r"), Some(CyclePhase::Requeued));
        assert!(
            review.pending().is_empty(),
            "blocking policy does not queue"
        );
        let GateDecision::SkippedAndFlagged(records) = decision else {
            panic!("expected a rejected record");
        };
        assert_eq!(records[0].status, GateStatus::Rejected);
    }

    #[test]
    fn free_lane_after_terminal_allows_next_dispatch() {
        let mut router = FleetRouter::new();
        let mut review = ReviewQueue::new();
        let policy = GatePolicy {
            skip_on_finish: true,
            ..GatePolicy::default()
        };
        router.enqueue("r", "task-1");
        router.enqueue("r", "task-2");
        let repos = active(&["r"]);
        router.dispatch_next(&repos).unwrap();
        router.mark_running("r");
        router
            .report_finish("r", &GateId::all(), "clean", &policy, &mut review)
            .unwrap();
        let next = router.dispatch_next(&repos).unwrap();
        assert_eq!(next.task, "task-2");
    }

    proptest! {
        /// Round-robin fairness: with equal queue depths, the first `n`
        /// dispatches hit every repo exactly once (no repo repeats before
        /// all others have had a turn), and a full drain dispatches each
        /// repo exactly `depth` times.
        #[test]
        fn prop_dispatch_round_robin_fair(n in 2usize..=6, depth in 1usize..=3) {
            let mut router = FleetRouter::new();
            let repos: Vec<String> = (0..n).map(|i| format!("repo-{i}")).collect();
            for repo in &repos {
                for j in 0..depth {
                    router.enqueue(repo, &format!("task-{repo}-{j}"));
                }
            }
            let policy = GatePolicy {
                skip_on_finish: true,
                ..GatePolicy::default()
            };
            let mut review = ReviewQueue::new();
            let mut dispatched: Vec<String> = Vec::new();

            for _ in 0..(n * depth) {
                let decision = router
                    .dispatch_next(&repos)
                    .expect("drain: some repo has queued work and a free lane");
                dispatched.push(decision.repo.clone());
                // Free the lane (all gates passed -> Completed) so the next
                // dispatch can rotate to the least-recently-dispatched repo.
                router.mark_running(&decision.repo);
                router
                    .report_finish(&decision.repo, &GateId::all(), "clean", &policy, &mut review)
                    .unwrap();
            }

            let mut first_round = dispatched[..n].to_vec();
            first_round.sort();
            assert_eq!(
                first_round, repos,
                "first n dispatches must be one turn per repo"
            );
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for repo in &dispatched {
                *counts.entry(repo.as_str()).or_insert(0) += 1;
            }
            for repo in &repos {
                assert_eq!(
                    counts.get(repo.as_str()).copied(),
                    Some(depth),
                    "repo {repo} dispatched the wrong number of times"
                );
            }
        }

        /// FIFO under interleaved enqueue/dispatch: however enqueues are
        /// interleaved with dispatch cycles, tasks leave the queue in
        /// exactly the order they entered.
        #[test]
        fn prop_queue_fifo_under_interleaving(
            tasks in proptest::collection::vec("[a-z]{1,12}", 0usize..20),
            coins in proptest::collection::vec(any::<bool>(), 0usize..64),
        ) {
            let mut router = FleetRouter::new();
            let repo = "r";
            let policy = GatePolicy {
                skip_on_finish: true,
                ..GatePolicy::default()
            };
            let mut review = ReviewQueue::new();
            let mut next_enq = 0usize;
            let mut out: Vec<String> = Vec::new();
            let mut k = 0usize;

            while next_enq < tasks.len() || router.queue_len(repo) > 0 {
                let can_dispatch = router.queue_len(repo) > 0
                    && !router.lane(repo).is_some_and(CyclePhase::occupies_lane);
                let coin = coins.get(k % coins.len().max(1)).copied().unwrap_or(false);
                if next_enq < tasks.len() && (coin || !can_dispatch) {
                    router.enqueue(repo, &tasks[next_enq]);
                    next_enq += 1;
                } else {
                    let decision = router
                        .dispatch_next(&[repo.to_string()])
                        .expect("dispatchable by construction");
                    out.push(decision.task);
                    router.mark_running(repo);
                    router
                        .report_finish(repo, &GateId::all(), "clean", &policy, &mut review)
                        .unwrap();
                }
                k += 1;
            }
            assert_eq!(out, tasks, "dequeued order must equal enqueue order");
        }

        /// dispatch_next never returns a repo whose lane occupies: every
        /// returned repo had a free lane, queued work, and was active
        /// immediately before the dispatch; when it returns None, no
        /// active repo was dispatchable.
        #[test]
        fn prop_dispatch_never_takes_occupied_lane(
            n in 2usize..=4,
            ops in proptest::collection::vec((any::<u8>(), any::<u8>(), any::<bool>()), 0usize..60),
        ) {
            let repos: Vec<String> = (0..n).map(|i| format!("repo-{i}")).collect();
            let mut router = FleetRouter::new();
            let mut review = ReviewQueue::new();
            let policy = GatePolicy {
                skip_on_finish: true,
                ..GatePolicy::default()
            };

            for (k, (raw_op, raw_repo, coin)) in ops.iter().enumerate() {
                let repo = &repos[(*raw_repo as usize) % n];
                match raw_op % 4 {
                    0 => router.enqueue(repo, &format!("task-{k}")),
                    1 => {
                        let pre: BTreeMap<String, (Option<CyclePhase>, usize)> = repos
                            .iter()
                            .map(|r| (r.clone(), (router.lane(r), router.queue_len(r))))
                            .collect();
                        let decision = router.dispatch_next(&repos);
                        match &decision {
                            Some(d) => {
                                let (pre_lane, pre_queue) = pre[&d.repo];
                                assert!(repos.contains(&d.repo), "dispatched an inactive repo");
                                assert!(
                                    !pre_lane.unwrap_or(CyclePhase::Queued).occupies_lane(),
                                    "dispatched {} whose lane was occupied ({pre_lane:?})",
                                    d.repo
                                );
                                assert!(
                                    pre_queue > 0,
                                    "dispatched {} whose queue was empty",
                                    d.repo
                                );
                                assert_eq!(router.lane(&d.repo), Some(CyclePhase::Dispatched));
                            },
                            None => {
                                for r in &repos {
                                    let (lane, queue) = pre[r];
                                    assert!(
                                        queue == 0 || lane.is_some_and(CyclePhase::occupies_lane),
                                        "dispatch_next returned None though {r} was dispatchable"
                                    );
                                }
                            },
                        }
                    },
                    2 => {
                        if *coin {
                            router.mark_running(repo);
                        } else {
                            router.report_failure(repo);
                        }
                    },
                    _ => {
                        let passed: &[GateId] = if *coin { &GateId::all() } else { &[] };
                        let _ = router.report_finish(repo, passed, "op", &policy, &mut review);
                    },
                }
            }
        }
    }
}
