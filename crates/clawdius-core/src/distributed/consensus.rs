//! Consensus protocol for distributed leader election.
//!
//! Implements a simplified Raft-inspired protocol with heartbeat-based leader
//! election. This is **scaffolding** — no network transport is included.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Role of a node in the consensus protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsensusState {
    /// Following the current leader.
    Follower,
    /// Contesting for leadership.
    Candidate,
    /// Currently the cluster leader.
    Leader,
}

/// A single entry in the replicated log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Monotonically increasing log index.
    pub index: u64,
    /// Term in which this entry was created.
    pub term: u64,
    /// Command payload (opaque bytes / JSON).
    pub command: Vec<u8>,
}

/// AppendEntries RPC payload (simplified Raft).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntries {
    /// Leader's current term.
    pub term: u64,
    /// Leader id so followers can redirect clients.
    pub leader_id: String,
    /// Index of log entry immediately preceding new ones.
    pub prev_log_index: u64,
    /// Term of prev_log_index entry.
    pub prev_log_term: u64,
    /// Log entries to store (empty for heartbeat).
    pub entries: Vec<LogEntry>,
    /// Leader's commit index.
    pub leader_commit: u64,
}

/// Heartbeat-based leader election.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderElection {
    /// Current term.
    pub current_term: u64,
    /// Node role.
    pub state: ConsensusState,
    /// Id of the node running this election state machine.
    pub node_id: String,
    /// Who we voted for in this term, if anyone.
    pub voted_for: Option<String>,
    /// Votes received in the current election (node id -> granted).
    pub votes_received: HashMap<String, bool>,
    /// Heartbeat timeout in milliseconds.
    pub heartbeat_timeout_ms: u64,
    /// Time of last received heartbeat (logical tick).
    pub last_heartbeat_tick: u64,
}

impl LeaderElection {
    /// Create a new election state machine starting as a follower.
    pub fn new(node_id: impl Into<String>) -> Self {
        Self {
            current_term: 0,
            state: ConsensusState::Follower,
            node_id: node_id.into(),
            voted_for: None,
            votes_received: HashMap::new(),
            heartbeat_timeout_ms: 150,
            last_heartbeat_tick: 0,
        }
    }

    /// Receive a heartbeat from a leader. Returns `true` if accepted.
    pub fn receive_heartbeat(&mut self, term: u64, leader_id: &str, tick: u64) -> bool {
        if term >= self.current_term {
            self.current_term = term;
            self.state = ConsensusState::Follower;
            self.last_heartbeat_tick = tick;
            if term > 0 {
                self.voted_for = Some(leader_id.to_string());
            }
            return true;
        }
        false
    }

    /// Start an election: transition to Candidate, increment term, vote for self.
    pub fn start_election(&mut self) {
        self.current_term += 1;
        self.state = ConsensusState::Candidate;
        self.voted_for = Some(self.node_id.clone());
        self.votes_received.clear();
        self.votes_received.insert(self.node_id.clone(), true);
    }

    /// Record a vote from a peer. Returns `true` if the vote grants a majority.
    pub fn receive_vote(&mut self, from: &str, granted: bool) -> bool {
        if self.state != ConsensusState::Candidate {
            return false;
        }
        self.votes_received.insert(from.to_string(), granted);
        let total_granted = self.votes_received.values().filter(|&&g| g).count();
        total_granted * 2 > self.votes_received.len()
    }

    /// Become leader (called after winning election).
    pub fn become_leader(&mut self) {
        self.state = ConsensusState::Leader;
    }

    /// Check if the heartbeat timeout has elapsed.
    pub fn is_heartbeat_timeout(&self, tick: u64) -> bool {
        tick.saturating_sub(self.last_heartbeat_tick) > self.heartbeat_timeout_ms
    }

    /// Process a tick: check for heartbeat timeout and trigger election if needed.
    /// Returns `true` if an election was started.
    pub fn tick(&mut self, tick: u64) -> bool {
        if self.state == ConsensusState::Follower && self.is_heartbeat_timeout(tick) {
            self.start_election();
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_election_starts_as_follower() {
        let election = LeaderElection::new("n1");
        assert_eq!(election.state, ConsensusState::Follower);
        assert_eq!(election.current_term, 0);
    }

    #[test]
    fn heartbeat_accepted_higher_term() {
        let mut election = LeaderElection::new("n1");
        let accepted = election.receive_heartbeat(1, "n2", 10);
        assert!(accepted);
        assert_eq!(election.current_term, 1);
        assert_eq!(election.state, ConsensusState::Follower);
        assert_eq!(election.last_heartbeat_tick, 10);
    }

    #[test]
    fn heartbeat_rejected_lower_term() {
        let mut election = LeaderElection::new("n1");
        election.current_term = 5;
        let accepted = election.receive_heartbeat(3, "n2", 10);
        assert!(!accepted);
        assert_eq!(election.current_term, 5);
    }

    #[test]
    fn start_election_increments_term() {
        let mut election = LeaderElection::new("n1");
        election.start_election();
        assert_eq!(election.current_term, 1);
        assert_eq!(election.state, ConsensusState::Candidate);
        assert_eq!(election.voted_for.as_deref(), Some("n1"));
    }

    #[test]
    fn receive_vote_grants_majority() {
        let mut election = LeaderElection::new("n1");
        election.start_election();
        let won = election.receive_vote("n2", true);
        assert!(won);
    }

    #[test]
    fn receive_vote_no_majority_yet() {
        let mut election = LeaderElection::new("n1");
        election.start_election();
        election.receive_vote("n2", true);
        election.receive_vote("n3", false);
        let won = election.receive_vote("n4", true);
        assert!(won);
    }

    #[test]
    fn receive_vote_ignored_when_not_candidate() {
        let mut election = LeaderElection::new("n1");
        let won = election.receive_vote("n2", true);
        assert!(!won);
    }

    #[test]
    fn become_leader_changes_state() {
        let mut election = LeaderElection::new("n1");
        election.start_election();
        election.become_leader();
        assert_eq!(election.state, ConsensusState::Leader);
    }

    #[test]
    fn tick_triggers_election_on_timeout() {
        let mut election = LeaderElection::new("n1");
        election.heartbeat_timeout_ms = 5;
        let started = election.tick(10);
        assert!(started);
        assert_eq!(election.state, ConsensusState::Candidate);
    }

    #[test]
    fn tick_no_timeout_yet() {
        let mut election = LeaderElection::new("n1");
        election.last_heartbeat_tick = 0;
        election.heartbeat_timeout_ms = 100;
        let started = election.tick(50);
        assert!(!started);
        assert_eq!(election.state, ConsensusState::Follower);
    }

    #[test]
    fn tick_candidate_does_not_re_elect() {
        let mut election = LeaderElection::new("n1");
        election.start_election();
        let started = election.tick(1000);
        assert!(!started);
        assert_eq!(election.state, ConsensusState::Candidate);
    }

    #[test]
    fn log_entry_serialization() {
        let entry = LogEntry {
            index: 1,
            term: 2,
            command: vec![1, 2, 3],
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let back: LogEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(entry.index, back.index);
        assert_eq!(entry.term, back.term);
        assert_eq!(entry.command, back.command);
    }

    #[test]
    fn append_entries_serialization() {
        let ae = AppendEntries {
            term: 1,
            leader_id: "n1".to_string(),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![],
            leader_commit: 0,
        };
        let json = serde_json::to_string(&ae).expect("serialize");
        let back: AppendEntries = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(ae.term, back.term);
        assert_eq!(ae.leader_id, back.leader_id);
    }

    #[test]
    fn consensus_state_serialization_roundtrip() {
        let states = vec![
            ConsensusState::Follower,
            ConsensusState::Candidate,
            ConsensusState::Leader,
        ];
        for s in states {
            let json = serde_json::to_string(&s).expect("serialize");
            let back: ConsensusState = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(s, back);
        }
    }
}
