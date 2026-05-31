//! LLM request routing strategies.
//!
//! Provides pluggable routing algorithms that select a cluster node for each
//! incoming LLM request. Each strategy implements the [`Router`] trait.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::distributed::node::{ClusterNode, ClusterState, NodeHealthStatus};

/// Outcome of a routing decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingDecision {
    /// A specific node was selected.
    RouteTo(String),
    /// No healthy nodes available.
    NoNodesAvailable,
    /// Request should be retried later.
    RetryLater,
}

/// Abstract routing strategy for selecting a cluster node.
pub trait Router: Send + Sync {
    /// Select the best node for an incoming request.
    fn route(&self, cluster: &ClusterState) -> RoutingDecision;
}

/// Round-robin router: cycles through healthy nodes sequentially.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoundRobinRouter {
    last_index: usize,
}

impl RoundRobinRouter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Router for RoundRobinRouter {
    fn route(&self, cluster: &ClusterState) -> RoutingDecision {
        let healthy: Vec<&ClusterNode> = cluster.healthy_nodes();
        if healthy.is_empty() {
            return RoutingDecision::NoNodesAvailable;
        }
        let index = self.last_index % healthy.len();
        RoutingDecision::RouteTo(healthy[index].id.clone())
    }
}

/// Least-connections router: picks the node with the lowest load.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LeastConnectionsRouter;

impl LeastConnectionsRouter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Router for LeastConnectionsRouter {
    fn route(&self, cluster: &ClusterState) -> RoutingDecision {
        let healthy: Vec<&ClusterNode> = cluster.healthy_nodes();
        if healthy.is_empty() {
            return RoutingDecision::NoNodesAvailable;
        }
        let best = healthy
            .iter()
            .min_by(|a, b| a.load.partial_cmp(&b.load).unwrap_or(std::cmp::Ordering::Equal));
        match best {
            Some(node) => RoutingDecision::RouteTo(node.id.clone()),
            None => RoutingDecision::NoNodesAvailable,
        }
    }
}

/// Latency-aware router: picks the node with the lowest recorded latency.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyAwareRouter {
    /// Per-node latency samples (ms). Updated externally.
    pub latencies: HashMap<String, f64>,
}

impl LatencyAwareRouter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Router for LatencyAwareRouter {
    fn route(&self, cluster: &ClusterState) -> RoutingDecision {
        let healthy: Vec<&ClusterNode> = cluster.healthy_nodes();
        if healthy.is_empty() {
            return RoutingDecision::NoNodesAvailable;
        }
        let best = healthy
            .iter()
            .min_by(|a, b| {
                let la = self.latencies.get(&a.id).copied().unwrap_or(f64::MAX);
                let lb = self.latencies.get(&b.id).copied().unwrap_or(f64::MAX);
                la.partial_cmp(&lb).unwrap_or(std::cmp::Ordering::Equal)
            });
        match best {
            Some(node) => RoutingDecision::RouteTo(node.id.clone()),
            None => RoutingDecision::NoNodesAvailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributed::node::ClusterNode;

    fn make_cluster(node_ids: &[&str]) -> ClusterState {
        let mut cluster = ClusterState::new();
        for id in node_ids {
            let port = 9001 + node_ids.iter().position(|&x| x == *id).unwrap_or(0);
            cluster.add_node(ClusterNode::new(*id, format!("127.0.0.1:{port}")));
        }
        cluster
    }

    fn make_unhealthy_cluster(node_ids: &[&str]) -> ClusterState {
        let mut cluster = ClusterState::new();
        for id in node_ids {
            let mut node = ClusterNode::new(*id, "127.0.0.1:9001");
            for _ in 0..3 {
                node.record_failure();
            }
            cluster.add_node(node);
        }
        cluster
    }

    #[test]
    fn round_robin_picks_first_node() {
        let router = RoundRobinRouter::new();
        let cluster = make_cluster(&["n1", "n2", "n3"]);
        let decision = router.route(&cluster);
        assert_eq!(decision, RoutingDecision::RouteTo("n1".to_string()));
    }

    #[test]
    fn round_robin_no_healthy_returns_none() {
        let router = RoundRobinRouter::new();
        let cluster = make_unhealthy_cluster(&["n1"]);
        let decision = router.route(&cluster);
        assert_eq!(decision, RoutingDecision::NoNodesAvailable);
    }

    #[test]
    fn least_connections_picks_lowest_load() {
        let router = LeastConnectionsRouter::new();
        let mut cluster = make_cluster(&["n1", "n2", "n3"]);
        if let Some(n) = cluster.nodes.get_mut("n1") {
            n.load = 0.8;
        }
        if let Some(n) = cluster.nodes.get_mut("n2") {
            n.load = 0.2;
        }
        if let Some(n) = cluster.nodes.get_mut("n3") {
            n.load = 0.5;
        }
        let decision = router.route(&cluster);
        assert_eq!(decision, RoutingDecision::RouteTo("n2".to_string()));
    }

    #[test]
    fn least_connections_no_healthy_returns_none() {
        let router = LeastConnectionsRouter::new();
        let cluster = ClusterState::new();
        let decision = router.route(&cluster);
        assert_eq!(decision, RoutingDecision::NoNodesAvailable);
    }

    #[test]
    fn latency_aware_picks_lowest_latency() {
        let mut router = LatencyAwareRouter::new();
        router.latencies.insert("n1".to_string(), 120.0);
        router.latencies.insert("n2".to_string(), 30.0);
        router.latencies.insert("n3".to_string(), 80.0);
        let cluster = make_cluster(&["n1", "n2", "n3"]);
        let decision = router.route(&cluster);
        assert_eq!(decision, RoutingDecision::RouteTo("n2".to_string()));
    }

    #[test]
    fn latency_aware_no_latency_data_uses_healthy() {
        let router = LatencyAwareRouter::new();
        let cluster = make_cluster(&["n1"]);
        let decision = router.route(&cluster);
        assert_eq!(decision, RoutingDecision::RouteTo("n1".to_string()));
    }

    #[test]
    fn latency_aware_no_healthy_returns_none() {
        let router = LatencyAwareRouter::new();
        let cluster = ClusterState::new();
        let decision = router.route(&cluster);
        assert_eq!(decision, RoutingDecision::NoNodesAvailable);
    }

    #[test]
    fn routing_decision_equality() {
        let a = RoutingDecision::RouteTo("n1".to_string());
        let b = RoutingDecision::RouteTo("n1".to_string());
        assert_eq!(a, b);
    }

    #[test]
    fn routing_decision_serialization() {
        let d = RoutingDecision::RetryLater;
        let json = serde_json::to_string(&d).expect("serialize");
        let back: RoutingDecision = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(d, back);
    }
}
