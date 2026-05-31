//! Cluster node representation and cluster-wide state management.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

/// A single node in the LLM routing cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    /// Unique node identifier.
    pub id: String,
    /// Network address (host:port).
    pub address: String,
    /// Current health status.
    pub health: NodeHealth,
    /// Current load level (0.0 – 1.0).
    pub load: f64,
    /// Timestamp of the last successful heartbeat.
    pub last_heartbeat: u64,
}

/// Health status of a cluster node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeHealthStatus {
    /// Node is fully operational.
    Healthy,
    /// Node is degraded but still serving requests.
    Degraded,
    /// Node is unreachable or has failed.
    Unhealthy,
}

/// Health details for a cluster node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHealth {
    /// Overall health status.
    pub status: NodeHealthStatus,
    /// Consecutive successful heartbeats.
    pub consecutive_successes: u32,
    /// Consecutive missed heartbeats.
    pub consecutive_failures: u32,
    /// Total requests served.
    pub total_requests: u64,
    /// Total errors encountered.
    pub total_errors: u64,
}

/// Cluster-wide state: membership, leadership, and epoch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterState {
    /// All known nodes keyed by id.
    pub nodes: HashMap<String, ClusterNode>,
    /// Current leader node id, if known.
    pub leader: Option<String>,
    /// Logical epoch — incremented on leader change.
    pub epoch: u64,
}

impl ClusterNode {
    /// Create a new node with the given id and address.
    pub fn new(id: impl Into<String>, address: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            address: address.into(),
            health: NodeHealth::default(),
            load: 0.0,
            last_heartbeat: 0,
        }
    }

    /// Returns `true` if the node is considered healthy enough to serve requests.
    pub fn is_healthy(&self) -> bool {
        self.health.status == NodeHealthStatus::Healthy
            || (self.health.status == NodeHealthStatus::Degraded
                && self.health.consecutive_failures < 3)
    }

    /// Record a successful heartbeat.
    pub fn record_heartbeat(&mut self, now: u64) {
        self.last_heartbeat = now;
        self.health.consecutive_successes += 1;
        self.health.consecutive_failures = 0;
        if self.health.status == NodeHealthStatus::Unhealthy
            && self.health.consecutive_successes >= 2
        {
            self.health.status = NodeHealthStatus::Degraded;
        }
        if self.health.status == NodeHealthStatus::Degraded
            && self.health.consecutive_successes >= 5
        {
            self.health.status = NodeHealthStatus::Healthy;
        }
    }

    /// Record a missed / failed heartbeat.
    pub fn record_failure(&mut self) {
        self.health.consecutive_failures += 1;
        self.health.consecutive_successes = 0;
        if self.health.consecutive_failures >= 3
            && self.health.status != NodeHealthStatus::Unhealthy
        {
            self.health.status = NodeHealthStatus::Unhealthy;
        }
    }

    /// Increment the request counter.
    pub fn increment_requests(&mut self) {
        self.health.total_requests += 1;
    }

    /// Increment the error counter.
    pub fn increment_errors(&mut self) {
        self.health.total_errors += 1;
    }
}

impl Default for NodeHealth {
    fn default() -> Self {
        Self {
            status: NodeHealthStatus::Healthy,
            consecutive_successes: 0,
            consecutive_failures: 0,
            total_requests: 0,
            total_errors: 0,
        }
    }
}

impl ClusterState {
    /// Create a new empty cluster state.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            leader: None,
            epoch: 0,
        }
    }

    /// Add a node to the cluster.
    pub fn add_node(&mut self, node: ClusterNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// Remove a node from the cluster. Returns the removed node if present.
    pub fn remove_node(&mut self, id: &str) -> Option<ClusterNode> {
        let removed = self.nodes.remove(id);
        if self.leader.as_deref() == Some(id) {
            self.leader = None;
            self.epoch += 1;
        }
        removed
    }

    /// Set the cluster leader and bump the epoch.
    pub fn set_leader(&mut self, id: impl Into<String>) {
        let new_leader = id.into();
        if self.leader.as_ref() != Some(&new_leader) {
            self.epoch += 1;
        }
        self.leader = Some(new_leader);
    }

    /// Get a reference to the leader node, if present.
    pub fn leader_node(&self) -> Option<&ClusterNode> {
        self.leader.as_ref().and_then(|id| self.nodes.get(id))
    }

    /// Get all healthy nodes.
    pub fn healthy_nodes(&self) -> Vec<&ClusterNode> {
        self.nodes.values().filter(|n| n.is_healthy()).collect()
    }
}

impl Default for ClusterState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_node_is_healthy() {
        let node = ClusterNode::new("n1", "127.0.0.1:9001");
        assert!(node.is_healthy());
    }

    #[test]
    fn unhealthy_node_is_not_healthy() {
        let mut node = ClusterNode::new("n1", "127.0.0.1:9001");
        for _ in 0..3 {
            node.record_failure();
        }
        assert_eq!(node.health.status, NodeHealthStatus::Unhealthy);
        assert!(!node.is_healthy());
    }

    #[test]
    fn degraded_node_with_few_failures_is_healthy() {
        let mut node = ClusterNode::new("n1", "127.0.0.1:9001");
        node.health.status = NodeHealthStatus::Degraded;
        node.health.consecutive_failures = 2;
        assert!(node.is_healthy());
    }

    #[test]
    fn degraded_node_with_many_failures_is_not_healthy() {
        let mut node = ClusterNode::new("n1", "127.0.0.1:9001");
        node.health.status = NodeHealthStatus::Degraded;
        node.health.consecutive_failures = 3;
        assert!(!node.is_healthy());
    }

    #[test]
    fn heartbeat_recovers_unhealthy_to_degraded() {
        let mut node = ClusterNode::new("n1", "127.0.0.1:9001");
        for _ in 0..3 {
            node.record_failure();
        }
        assert_eq!(node.health.status, NodeHealthStatus::Unhealthy);
        node.record_heartbeat(1);
        node.record_heartbeat(2);
        assert_eq!(node.health.status, NodeHealthStatus::Degraded);
    }

    #[test]
    fn heartbeat_recovers_degraded_to_healthy() {
        let mut node = ClusterNode::new("n1", "127.0.0.1:9001");
        node.health.status = NodeHealthStatus::Degraded;
        for i in 0..5 {
            node.record_heartbeat(i as u64);
        }
        assert_eq!(node.health.status, NodeHealthStatus::Healthy);
    }

    #[test]
    fn three_failures_mark_unhealthy() {
        let mut node = ClusterNode::new("n1", "127.0.0.1:9001");
        node.record_failure();
        node.record_failure();
        node.record_failure();
        assert_eq!(node.health.status, NodeHealthStatus::Unhealthy);
    }

    #[test]
    fn cluster_add_and_remove_node() {
        let mut cluster = ClusterState::new();
        let node = ClusterNode::new("n1", "127.0.0.1:9001");
        cluster.add_node(node);
        assert!(cluster.nodes.contains_key("n1"));
        let removed = cluster.remove_node("n1");
        assert!(removed.is_some());
        assert!(!cluster.nodes.contains_key("n1"));
    }

    #[test]
    fn cluster_set_leader() {
        let mut cluster = ClusterState::new();
        cluster.add_node(ClusterNode::new("n1", "127.0.0.1:9001"));
        cluster.add_node(ClusterNode::new("n2", "127.0.0.1:9002"));
        cluster.set_leader("n1");
        assert_eq!(cluster.leader.as_deref(), Some("n1"));
        assert_eq!(cluster.epoch, 1);
    }

    #[test]
    fn removing_leader_clears_and_bumps_epoch() {
        let mut cluster = ClusterState::new();
        cluster.add_node(ClusterNode::new("n1", "127.0.0.1:9001"));
        cluster.set_leader("n1");
        assert_eq!(cluster.epoch, 1);
        cluster.remove_node("n1");
        assert!(cluster.leader.is_none());
        assert_eq!(cluster.epoch, 2);
    }

    #[test]
    fn same_leader_does_not_bump_epoch() {
        let mut cluster = ClusterState::new();
        cluster.set_leader("n1");
        cluster.set_leader("n1");
        assert_eq!(cluster.epoch, 1);
    }

    #[test]
    fn healthy_nodes_filters_correctly() {
        let mut cluster = ClusterState::new();
        cluster.add_node(ClusterNode::new("n1", "127.0.0.1:9001"));
        let mut n2 = ClusterNode::new("n2", "127.0.0.1:9002");
        for _ in 0..3 {
            n2.record_failure();
        }
        cluster.add_node(n2);
        assert_eq!(cluster.healthy_nodes().len(), 1);
    }
}
