//! Distributed LLM routing with multi-node support.
//!
//! Routes LLM requests across a cluster of Clawdius nodes for load
//! balancing, failover, and horizontal scaling.

use anyhow::{Context, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A node in the Clawdius cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    /// Unique node identifier.
    pub id: String,
    /// Node endpoint URL (e.g., "http://10.0.1.5:8080").
    pub endpoint: String,
    /// Whether this node is currently healthy.
    pub healthy: bool,
    /// Current load (0.0 = idle, 1.0 = saturated).
    pub load: f64,
    /// Last health check timestamp.
    pub last_health_check: Option<u64>,
    /// Node region/zone for locality-aware routing.
    pub region: Option<String>,
}

/// Load balancing strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadBalanceStrategy {
    /// Route to the node with the lowest load.
    LeastLoaded,
    /// Distribute requests in round-robin order.
    RoundRobin,
    /// Route to a random healthy node.
    Random,
    /// Route to the first available node (primary/failover).
    Priority,
}

/// Health check configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Interval between health checks.
    pub interval_secs: u64,
    /// Request timeout for health checks.
    pub timeout_secs: u64,
    /// Number of consecutive failures before marking unhealthy.
    pub failure_threshold: u32,
    /// Number of consecutive successes before marking healthy.
    pub success_threshold: u32,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            interval_secs: 10,
            timeout_secs: 5,
            failure_threshold: 3,
            success_threshold: 2,
        }
    }
}

/// Circuit breaker state for a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerState {
    Closed,   // Normal operation
    Open,     // Failing, requests blocked
    HalfOpen, // Testing if node recovered
}

/// Per-node circuit breaker.
#[derive(Debug)]
struct NodeCircuitBreaker {
    state: CircuitBreakerState,
    consecutive_failures: u32,
    consecutive_successes: u32,
    last_failure_time: Option<Instant>,
}

impl Default for NodeCircuitBreaker {
    fn default() -> Self {
        Self {
            state: CircuitBreakerState::Closed,
            consecutive_failures: 0,
            consecutive_successes: 0,
            last_failure_time: None,
        }
    }
}

/// Cluster router that manages multi-node LLM request routing.
pub struct ClusterRouter {
    nodes: RwLock<Vec<ClusterNode>>,
    circuit_breakers: RwLock<HashMap<String, NodeCircuitBreaker>>,
    strategy: LoadBalanceStrategy,
    health_config: HealthCheckConfig,
    round_robin_idx: RwLock<usize>,
    http_client: reqwest::Client,
}

impl ClusterRouter {
    /// Create a new cluster router with the given nodes and strategy.
    pub fn new(
        nodes: Vec<ClusterNode>,
        strategy: LoadBalanceStrategy,
        health_config: HealthCheckConfig,
    ) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            circuit_breakers: RwLock::new(
                nodes
                    .iter()
                    .map(|n| (n.id.clone(), NodeCircuitBreaker::default()))
                    .collect(),
            ),
            nodes: RwLock::new(nodes),
            strategy,
            health_config,
            round_robin_idx: RwLock::new(0),
            http_client,
        }
    }

    /// Create a single-node router (no clustering, but interface-compatible).
    pub fn single_node(endpoint: String) -> Self {
        Self::new(
            vec![ClusterNode {
                id: "local".to_string(),
                endpoint,
                healthy: true,
                load: 0.0,
                last_health_check: None,
                region: None,
            }],
            LoadBalanceStrategy::Priority,
            HealthCheckConfig::default(),
        )
    }

    /// Select the best node for routing a request.
    pub fn select_node(&self) -> Option<ClusterNode> {
        let nodes = self.nodes.read();
        let breakers = self.circuit_breakers.read();

        let healthy_nodes: Vec<_> = nodes
            .iter()
            .filter(|n| {
                n.healthy
                    && breakers
                        .get(&n.id)
                        .map(|cb| cb.state != CircuitBreakerState::Open)
                        .unwrap_or(true)
            })
            .collect();

        if healthy_nodes.is_empty() {
            return None;
        }

        match self.strategy {
            LoadBalanceStrategy::LeastLoaded => healthy_nodes
                .into_iter()
                .min_by(|a, b| {
                    a.load
                        .partial_cmp(&b.load)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .cloned(),
            LoadBalanceStrategy::RoundRobin => {
                let mut idx = self.round_robin_idx.write();
                let selected = healthy_nodes[*idx % healthy_nodes.len()].clone();
                *idx = (*idx + 1) % healthy_nodes.len().max(1);
                Some(selected)
            },
            LoadBalanceStrategy::Random => {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                Instant::now().hash(&mut hasher);
                let idx = (hasher.finish() as usize) % healthy_nodes.len();
                Some(healthy_nodes[idx].clone())
            },
            LoadBalanceStrategy::Priority => healthy_nodes.into_iter().next().cloned(),
        }
    }

    /// Route a request to a selected node.
    pub async fn route_request(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let node = self.select_node().context("No healthy nodes available")?;

        let url = format!("{}{}", node.endpoint.trim_end_matches('/'), path);

        let result = self.http_client.post(&url).json(body).send().await;

        match result {
            Ok(resp) if resp.status().is_success() => {
                self.record_success(&node.id);
                resp.json()
                    .await
                    .context("Failed to parse response from node")
            },
            Ok(resp) => {
                self.record_failure(&node.id);
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!("Node {} returned {}: {}", node.id, status, body)
            },
            Err(e) => {
                self.record_failure(&node.id);
                anyhow::bail!("Request to node {} failed: {}", node.id, e)
            },
        }
    }

    /// Record a successful request to a node.
    fn record_success(&self, node_id: &str) {
        let mut breakers = self.circuit_breakers.write();
        if let Some(cb) = breakers.get_mut(node_id) {
            cb.consecutive_failures = 0;
            cb.consecutive_successes += 1;
            if cb.state == CircuitBreakerState::HalfOpen
                && cb.consecutive_successes >= self.health_config.success_threshold
            {
                cb.state = CircuitBreakerState::Closed;
            }
        }

        // Update node load (simplified)
        let mut nodes = self.nodes.write();
        if let Some(node) = nodes.iter_mut().find(|n| n.id == node_id) {
            node.load = (node.load + 0.01).min(1.0);
        }
    }

    /// Record a failed request to a node.
    fn record_failure(&self, node_id: &str) {
        let mut breakers = self.circuit_breakers.write();
        if let Some(cb) = breakers.get_mut(node_id) {
            cb.consecutive_successes = 0;
            cb.consecutive_failures += 1;
            cb.last_failure_time = Some(Instant::now());

            if cb.consecutive_failures >= self.health_config.failure_threshold {
                cb.state = CircuitBreakerState::Open;
                tracing::warn!(
                    "Circuit breaker opened for node {} after {} failures",
                    node_id,
                    cb.consecutive_failures
                );
            }
        }
    }

    /// Run a health check on all nodes.
    pub async fn health_check_all(&self) -> Vec<String> {
        let nodes = self.nodes.read().clone();
        let mut unhealthy = Vec::new();

        for node in &nodes {
            match self.check_node_health(node).await {
                true => {
                    let mut nodes = self.nodes.write();
                    if let Some(n) = nodes.iter_mut().find(|n| n.id == node.id) {
                        n.healthy = true;
                        n.last_health_check = Some(
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        );
                    }
                },
                false => {
                    let mut nodes = self.nodes.write();
                    if let Some(n) = nodes.iter_mut().find(|n| n.id == node.id) {
                        n.healthy = false;
                    }
                    unhealthy.push(node.id.clone());
                },
            }
        }

        unhealthy
    }

    /// Check health of a single node.
    async fn check_node_health(&self, node: &ClusterNode) -> bool {
        let url = format!("{}/api/v1/health", node.endpoint.trim_end_matches('/'));

        match self
            .http_client
            .get(&url)
            .timeout(Duration::from_secs(self.health_config.timeout_secs))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Get all nodes (for monitoring).
    pub fn nodes(&self) -> Vec<ClusterNode> {
        self.nodes.read().clone()
    }

    /// Get circuit breaker states (for monitoring).
    pub fn circuit_breaker_states(&self) -> HashMap<String, CircuitBreakerState> {
        self.circuit_breakers
            .read()
            .iter()
            .map(|(k, v)| (k.clone(), v.state))
            .collect()
    }

    /// Add a new node to the cluster.
    pub fn add_node(&self, node: ClusterNode) {
        let id = node.id.clone();
        self.nodes.write().push(node);
        self.circuit_breakers
            .write()
            .insert(id, NodeCircuitBreaker::default());
    }

    /// Remove a node from the cluster.
    pub fn remove_node(&self, node_id: &str) {
        self.nodes.write().retain(|n| n.id != node_id);
        self.circuit_breakers.write().remove(node_id);
    }
}

/// Cluster configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// List of seed nodes.
    pub nodes: Vec<ClusterNodeConfig>,
    /// Load balancing strategy.
    #[serde(default = "default_strategy")]
    pub strategy: LoadBalanceStrategy,
    /// Health check configuration.
    #[serde(default)]
    pub health_check: HealthCheckConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNodeConfig {
    pub id: String,
    pub endpoint: String,
    pub region: Option<String>,
}

fn default_strategy() -> LoadBalanceStrategy {
    LoadBalanceStrategy::LeastLoaded
}

impl ClusterRouter {
    /// Create from cluster configuration.
    pub fn from_config(config: ClusterConfig) -> Self {
        let nodes: Vec<ClusterNode> = config
            .nodes
            .into_iter()
            .map(|n| ClusterNode {
                id: n.id,
                endpoint: n.endpoint,
                healthy: true,
                load: 0.0,
                last_health_check: None,
                region: n.region,
            })
            .collect();

        Self::new(nodes, config.strategy, config.health_check)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_node_least_loaded() {
        let router = ClusterRouter::new(
            vec![
                ClusterNode {
                    id: "node-1".into(),
                    endpoint: "http://localhost:8081".into(),
                    healthy: true,
                    load: 0.8,
                    last_health_check: None,
                    region: None,
                },
                ClusterNode {
                    id: "node-2".into(),
                    endpoint: "http://localhost:8082".into(),
                    healthy: true,
                    load: 0.2,
                    last_health_check: None,
                    region: None,
                },
            ],
            LoadBalanceStrategy::LeastLoaded,
            HealthCheckConfig::default(),
        );

        let selected = router.select_node().unwrap();
        assert_eq!(selected.id, "node-2");
    }

    #[test]
    fn test_select_node_round_robin() {
        let router = ClusterRouter::new(
            vec![
                ClusterNode {
                    id: "a".into(),
                    endpoint: "http://a".into(),
                    healthy: true,
                    load: 0.0,
                    last_health_check: None,
                    region: None,
                },
                ClusterNode {
                    id: "b".into(),
                    endpoint: "http://b".into(),
                    healthy: true,
                    load: 0.0,
                    last_health_check: None,
                    region: None,
                },
                ClusterNode {
                    id: "c".into(),
                    endpoint: "http://c".into(),
                    healthy: true,
                    load: 0.0,
                    last_health_check: None,
                    region: None,
                },
            ],
            LoadBalanceStrategy::RoundRobin,
            HealthCheckConfig::default(),
        );

        let first = router.select_node().unwrap();
        let second = router.select_node().unwrap();
        let third = router.select_node().unwrap();
        let fourth = router.select_node().unwrap();

        assert_eq!(first.id, "a");
        assert_eq!(second.id, "b");
        assert_eq!(third.id, "c");
        assert_eq!(fourth.id, "a"); // wraps around
    }

    #[test]
    fn test_circuit_breaker_opens_on_failures() {
        let router = ClusterRouter::new(
            vec![ClusterNode {
                id: "node-1".into(),
                endpoint: "http://localhost:8081".into(),
                healthy: true,
                load: 0.0,
                last_health_check: None,
                region: None,
            }],
            LoadBalanceStrategy::Priority,
            HealthCheckConfig {
                failure_threshold: 3,
                success_threshold: 2,
                interval_secs: 10,
                timeout_secs: 5,
            },
        );

        router.record_failure("node-1");
        router.record_failure("node-1");
        assert_eq!(
            router.circuit_breaker_states().get("node-1"),
            Some(&CircuitBreakerState::Closed)
        );

        router.record_failure("node-1"); // 3rd failure
        assert_eq!(
            router.circuit_breaker_states().get("node-1"),
            Some(&CircuitBreakerState::Open)
        );

        // Node should not be selected when breaker is open
        assert!(router.select_node().is_none());
    }

    #[test]
    fn test_circuit_breaker_half_open_recovery() {
        let router = ClusterRouter::new(
            vec![ClusterNode {
                id: "node-1".into(),
                endpoint: "http://localhost:8081".into(),
                healthy: true,
                load: 0.0,
                last_health_check: None,
                region: None,
            }],
            LoadBalanceStrategy::Priority,
            HealthCheckConfig {
                failure_threshold: 1,
                success_threshold: 1,
                interval_secs: 1,
                timeout_secs: 1,
            },
        );

        // Open the breaker
        router.record_failure("node-1");
        assert_eq!(
            router.circuit_breaker_states().get("node-1"),
            Some(&CircuitBreakerState::Open)
        );

        // Manually set to half-open for testing
        {
            let mut breakers = router.circuit_breakers.write();
            breakers.get_mut("node-1").unwrap().state = CircuitBreakerState::HalfOpen;
        }

        // Success should close the breaker
        router.record_success("node-1");
        assert_eq!(
            router.circuit_breaker_states().get("node-1"),
            Some(&CircuitBreakerState::Closed)
        );
    }

    #[test]
    fn test_add_remove_node() {
        let router = ClusterRouter::single_node("http://localhost:8080".into());
        assert_eq!(router.nodes().len(), 1);

        router.add_node(ClusterNode {
            id: "node-2".into(),
            endpoint: "http://localhost:8082".into(),
            healthy: true,
            load: 0.0,
            last_health_check: None,
            region: None,
        });
        assert_eq!(router.nodes().len(), 2);

        router.remove_node("node-2");
        assert_eq!(router.nodes().len(), 1);
    }

    #[test]
    fn test_from_config() {
        let config = ClusterConfig {
            nodes: vec![
                ClusterNodeConfig {
                    id: "n1".into(),
                    endpoint: "http://10.0.0.1:8080".into(),
                    region: Some("us-east".into()),
                },
                ClusterNodeConfig {
                    id: "n2".into(),
                    endpoint: "http://10.0.0.2:8080".into(),
                    region: Some("us-east".into()),
                },
            ],
            strategy: LoadBalanceStrategy::RoundRobin,
            health_check: HealthCheckConfig::default(),
        };

        let router = ClusterRouter::from_config(config);
        assert_eq!(router.nodes().len(), 2);
    }
}
