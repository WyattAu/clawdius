#![deny(unsafe_code)]

//! Usage Metering for Multi-Tenant Messaging
//!
//! Records each `process_message()` invocation with tenant, user, platform,
//! command category, outcome, and latency. Exposes counters and histograms
//! through the existing `MetricsStore` for Prometheus scraping.
//!
//! # Design
//!
//! - In-memory ring buffer of recent events (bounded, O(1) insert).
//! - Prometheus counters are incremented synchronously (lock-free atomics via
//!   `MetricsStore::messaging_counter_inc`).
//! - Optional persistence via `StateStore` for billing/audit trail.
//!
//! # Prometheus Metrics Emitted
//!
//! | Name | Type | Labels |
//! |------|------|--------|
//! | `clawdius_usage_messages_total` | counter | `tenant`, `platform`, `category`, `outcome` |
//! | `clawdius_usage_message_duration_ms` | histogram | `tenant`, `platform`, `category` |
//! | `clawdius_usage_active_tenants` | gauge | — |

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use super::state_store::StateStore;
use super::types::{CommandCategory, Platform};

/// Maximum number of events kept in the in-memory ring buffer.
const RING_BUFFER_CAPACITY: usize = 10_000;

/// StateStore table name for persisted usage events.
const USAGE_TABLE: &str = "usage_events";

/// Outcome of a processed message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Success,
    Error,
    RateLimited,
    Unauthorized,
    InvalidCommand,
}

impl std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => write!(f, "success"),
            Self::Error => write!(f, "error"),
            Self::RateLimited => write!(f, "rate_limited"),
            Self::Unauthorized => write!(f, "unauthorized"),
            Self::InvalidCommand => write!(f, "invalid_command"),
        }
    }
}

/// A single usage event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEvent {
    /// Tenant ID, or `"default"` if multi-tenancy is disabled.
    pub tenant_id: String,
    /// User ID on the messaging platform.
    pub user_id: String,
    /// Messaging platform.
    pub platform: Platform,
    /// Command category (status, help, generate, etc.).
    pub category: CommandCategory,
    /// Outcome of the message processing.
    pub outcome: Outcome,
    /// Processing latency in milliseconds.
    pub latency_ms: u64,
    /// Unix timestamp (seconds) when the event was recorded.
    pub timestamp: u64,
}

impl UsageEvent {
    /// Create a new usage event.
    #[must_use]
    pub fn new(
        tenant_id: String,
        user_id: String,
        platform: Platform,
        category: CommandCategory,
        outcome: Outcome,
        latency_ms: u64,
    ) -> Self {
        Self {
            tenant_id,
            user_id,
            platform,
            category,
            outcome,
            latency_ms,
            timestamp: now_unix(),
        }
    }
}

/// Tracks usage across all tenants and users.
///
/// Thread-safe, designed to be shared via `Arc` across the gateway.
pub struct UsageTracker {
    /// In-memory ring buffer of recent events.
    events: RwLock<VecDeque<UsageEvent>>,
    /// Optional persistent store for billing/audit.
    store: Option<Arc<dyn StateStore>>,
    /// Count of distinct tenants seen (for gauge).
    active_tenants: RwLock<std::collections::HashSet<String>>,
    /// Prometheus metrics callback.
    metrics: Option<Arc<dyn UsageMetricsSink>>,
}

/// Trait for emitting Prometheus-compatible metrics from usage events.
///
/// Implemented by the server's `MetricsStore` to bridge usage tracking
/// into the existing Prometheus exposition endpoint.
pub trait UsageMetricsSink: Send + Sync {
    /// Increment a counter.
    fn counter_inc(&self, name: &str, labels: &str);
    /// Record a histogram observation.
    fn histogram_observe(&self, name: &str, labels: &str, value_ms: f64);
    /// Set a gauge value.
    fn gauge_set(&self, name: &str, value: i64);
}

/// Prometheus metric name constants for usage metering.
pub const USAGE_MESSAGES_TOTAL: &str = "clawdius_usage_messages_total";
pub const USAGE_MESSAGE_DURATION_MS: &str = "clawdius_usage_message_duration_ms";
pub const USAGE_ACTIVE_TENANTS: &str = "clawdius_usage_active_tenants";

impl UsageTracker {
    /// Create a new usage tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            events: RwLock::new(VecDeque::with_capacity(RING_BUFFER_CAPACITY)),
            store: None,
            active_tenants: RwLock::new(std::collections::HashSet::new()),
            metrics: None,
        }
    }

    /// Attach a persistent state store.
    pub fn with_store(mut self, store: Arc<dyn StateStore>) -> Self {
        self.store = Some(store);
        self
    }

    /// Attach a Prometheus metrics sink.
    pub fn with_metrics(mut self, sink: Arc<dyn UsageMetricsSink>) -> Self {
        self.metrics = Some(sink);
        self
    }

    /// Record a usage event.
    ///
    /// Increments Prometheus counters, appends to the ring buffer,
    /// and optionally persists to the state store (fire-and-forget).
    pub async fn record(&self, event: UsageEvent) {
        // Update Prometheus metrics
        if let Some(sink) = &self.metrics {
            let tenant = &event.tenant_id;
            let platform = format!("{:?}", event.platform).to_lowercase();
            let category = format!("{:?}", event.category).to_lowercase();
            let outcome = event.outcome.to_string();

            sink.counter_inc(
                USAGE_MESSAGES_TOTAL,
                &format!(
                    "tenant=\"{}\",platform=\"{platform}\",category=\"{category}\",outcome=\"{outcome}\"",
                    tenant
                ),
            );

            if event.outcome == Outcome::Success {
                sink.histogram_observe(
                    USAGE_MESSAGE_DURATION_MS,
                    &format!(
                        "tenant=\"{}\",platform=\"{platform}\",category=\"{category}\"",
                        tenant
                    ),
                    event.latency_ms as f64,
                );
            }
        }

        // Update active tenants set
        {
            let mut tenants = self.active_tenants.write().await;
            tenants.insert(event.tenant_id.clone());
            if let Some(sink) = &self.metrics {
                sink.gauge_set(USAGE_ACTIVE_TENANTS, tenants.len() as i64);
            }
        }

        // Append to ring buffer (evicts oldest if full)
        {
            let mut events = self.events.write().await;
            if events.len() >= RING_BUFFER_CAPACITY {
                events.pop_front();
            }
            events.push_back(event.clone());
        }

        // Persist to state store (fire-and-forget)
        if let Some(store) = &self.store {
            let store = Arc::clone(store);
            let event_for_store = event.clone();
            tokio::spawn(async move {
                if let Err(e) = persist_event(&store, &event_for_store).await {
                    warn!(error = %e, "Failed to persist usage event");
                }
            });
        }

        debug!(
            tenant = %event.tenant_id,
            user = %event.user_id,
            platform = ?event.platform,
            category = ?event.category,
            outcome = %event.outcome,
            latency_ms = event.latency_ms,
            "Usage event recorded"
        );
    }

    /// Query recent events from the in-memory ring buffer.
    ///
    /// Returns up to `limit` most recent events. If `tenant_id` is `Some`,
    /// filters to that tenant only.
    pub async fn recent_events(&self, limit: usize, tenant_id: Option<&str>) -> Vec<UsageEvent> {
        let events = self.events.read().await;
        let mut result: Vec<&UsageEvent> = events
            .iter()
            .filter(|e| tenant_id.is_none_or(|tid| e.tenant_id == tid))
            .collect();
        result.reverse(); // most recent first
        result.truncate(limit);
        result.into_iter().cloned().collect()
    }

    /// Get usage summary for a tenant in the current ring buffer window.
    pub async fn tenant_summary(&self, tenant_id: &str) -> TenantUsageSummary {
        let events = self.events.read().await;
        let mut total = 0u64;
        let mut success = 0u64;
        let mut errors = 0u64;
        let mut total_latency_ms = 0u64;
        let mut users = std::collections::HashSet::new();

        for event in events.iter().filter(|e| e.tenant_id == tenant_id) {
            total += 1;
            users.insert(event.user_id.clone());
            match event.outcome {
                Outcome::Success => {
                    success += 1;
                    total_latency_ms += event.latency_ms;
                },
                Outcome::Error => errors += 1,
                _ => {},
            }
        }

        let avg_latency_ms = if success > 0 {
            total_latency_ms / success
        } else {
            0
        };

        TenantUsageSummary {
            tenant_id: tenant_id.to_string(),
            total_messages: total,
            successful: success,
            errors,
            avg_latency_ms,
            unique_users: users.len(),
        }
    }

    /// Get the count of distinct active tenants.
    pub async fn active_tenant_count(&self) -> usize {
        self.active_tenants.read().await.len()
    }

    /// Get total events in the ring buffer.
    pub async fn event_count(&self) -> usize {
        self.events.read().await.len()
    }
}

impl Default for UsageTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of usage for a single tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantUsageSummary {
    pub tenant_id: String,
    pub total_messages: u64,
    pub successful: u64,
    pub errors: u64,
    pub avg_latency_ms: u64,
    pub unique_users: usize,
}

async fn persist_event(
    store: &Arc<dyn StateStore>,
    event: &UsageEvent,
) -> super::types::Result<()> {
    // Ensure the table exists
    if !store.table_exists(USAGE_TABLE).await? {
        store.create_table(USAGE_TABLE).await?;
    }

    let key = format!(
        "{}:{}:{}:{:?}",
        event.tenant_id, event.user_id, event.timestamp, event.category
    );
    let value = serde_json::to_vec(event).map_err(|e| {
        super::types::MessagingError::ParseError(format!("Failed to serialize usage event: {e}"))
    })?;

    // TTL of 30 days
    store
        .set(USAGE_TABLE, &key, &value, Some(30 * 24 * 3600))
        .await
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MockMetricsSink {
        counters: std::sync::Mutex<HashMap<String, u64>>,
        gauges: std::sync::Mutex<HashMap<String, i64>>,
    }

    impl MockMetricsSink {
        fn new() -> Self {
            Self {
                counters: std::sync::Mutex::new(HashMap::new()),
                gauges: std::sync::Mutex::new(HashMap::new()),
            }
        }

        fn counter_value(&self, key: &str) -> u64 {
            self.counters.lock().unwrap().get(key).copied().unwrap_or(0)
        }

        fn gauge_value(&self, key: &str) -> i64 {
            self.gauges.lock().unwrap().get(key).copied().unwrap_or(0)
        }
    }

    impl UsageMetricsSink for MockMetricsSink {
        fn counter_inc(&self, name: &str, labels: &str) {
            let key = format!("{}{{{}}}", name, labels);
            let mut counters = self.counters.lock().unwrap();
            *counters.entry(key).or_insert(0) += 1;
        }

        fn histogram_observe(&self, _name: &str, _labels: &str, _value_ms: f64) {
            // No-op for tests
        }

        fn gauge_set(&self, name: &str, value: i64) {
            let mut gauges = self.gauges.lock().unwrap();
            gauges.insert(name.to_string(), value);
        }
    }

    fn make_event(
        tenant: &str,
        user: &str,
        category: CommandCategory,
        outcome: Outcome,
    ) -> UsageEvent {
        UsageEvent::new(
            tenant.to_string(),
            user.to_string(),
            Platform::Telegram,
            category,
            outcome,
            42,
        )
    }

    #[tokio::test]
    async fn test_record_and_recent_events() {
        let tracker = UsageTracker::new();
        tracker
            .record(make_event(
                "t1",
                "u1",
                CommandCategory::Help,
                Outcome::Success,
            ))
            .await;
        tracker
            .record(make_event(
                "t1",
                "u2",
                CommandCategory::Generate,
                Outcome::Error,
            ))
            .await;
        tracker
            .record(make_event(
                "t2",
                "u1",
                CommandCategory::Status,
                Outcome::Success,
            ))
            .await;

        let recent = tracker.recent_events(10, None).await;
        assert_eq!(recent.len(), 3);

        let t1_events = tracker.recent_events(10, Some("t1")).await;
        assert_eq!(t1_events.len(), 2);

        let t2_events = tracker.recent_events(10, Some("t2")).await;
        assert_eq!(t2_events.len(), 1);
    }

    #[tokio::test]
    async fn test_ring_buffer_eviction() {
        let tracker = UsageTracker::new();

        for i in 0..(RING_BUFFER_CAPACITY + 100) {
            tracker
                .record(make_event(
                    "t1",
                    &format!("u{}", i),
                    CommandCategory::Help,
                    Outcome::Success,
                ))
                .await;
        }

        assert_eq!(tracker.event_count().await, RING_BUFFER_CAPACITY);
    }

    #[tokio::test]
    async fn test_recent_events_limit() {
        let tracker = UsageTracker::new();
        for _ in 0..5 {
            tracker
                .record(make_event(
                    "t1",
                    "u1",
                    CommandCategory::Help,
                    Outcome::Success,
                ))
                .await;
        }

        let recent = tracker.recent_events(2, None).await;
        assert_eq!(recent.len(), 2);
    }

    #[tokio::test]
    async fn test_tenant_summary() {
        let tracker = UsageTracker::new();
        tracker
            .record(make_event(
                "t1",
                "u1",
                CommandCategory::Help,
                Outcome::Success,
            ))
            .await;
        tracker
            .record(make_event(
                "t1",
                "u2",
                CommandCategory::Generate,
                Outcome::Success,
            ))
            .await;
        tracker
            .record(make_event(
                "t1",
                "u1",
                CommandCategory::Status,
                Outcome::Error,
            ))
            .await;

        let summary = tracker.tenant_summary("t1").await;
        assert_eq!(summary.total_messages, 3);
        assert_eq!(summary.successful, 2);
        assert_eq!(summary.errors, 1);
        assert_eq!(summary.unique_users, 2);
    }

    #[tokio::test]
    async fn test_active_tenant_count() {
        let tracker = UsageTracker::new();
        tracker
            .record(make_event(
                "t1",
                "u1",
                CommandCategory::Help,
                Outcome::Success,
            ))
            .await;
        tracker
            .record(make_event(
                "t2",
                "u1",
                CommandCategory::Help,
                Outcome::Success,
            ))
            .await;
        tracker
            .record(make_event(
                "t1",
                "u2",
                CommandCategory::Help,
                Outcome::Success,
            ))
            .await;

        assert_eq!(tracker.active_tenant_count().await, 2);
    }

    #[tokio::test]
    async fn test_metrics_sink_counter() {
        let sink = Arc::new(MockMetricsSink::new());
        let tracker = UsageTracker::new().with_metrics(sink.clone());

        tracker
            .record(make_event(
                "t1",
                "u1",
                CommandCategory::Help,
                Outcome::Success,
            ))
            .await;
        tracker
            .record(make_event(
                "t1",
                "u1",
                CommandCategory::Help,
                Outcome::Success,
            ))
            .await;
        tracker
            .record(make_event(
                "t1",
                "u1",
                CommandCategory::Help,
                Outcome::Error,
            ))
            .await;

        let success_key = format!(
            "{}{{{}}}",
            USAGE_MESSAGES_TOTAL,
            "tenant=\"t1\",platform=\"telegram\",category=\"help\",outcome=\"success\""
        );
        assert_eq!(sink.counter_value(&success_key), 2);

        let error_key = format!(
            "{}{{{}}}",
            USAGE_MESSAGES_TOTAL,
            "tenant=\"t1\",platform=\"telegram\",category=\"help\",outcome=\"error\""
        );
        assert_eq!(sink.counter_value(&error_key), 1);
    }

    #[tokio::test]
    async fn test_metrics_sink_active_tenants_gauge() {
        let sink = Arc::new(MockMetricsSink::new());
        let tracker = UsageTracker::new().with_metrics(sink.clone());

        tracker
            .record(make_event(
                "t1",
                "u1",
                CommandCategory::Help,
                Outcome::Success,
            ))
            .await;
        assert_eq!(sink.gauge_value(USAGE_ACTIVE_TENANTS), 1);

        tracker
            .record(make_event(
                "t2",
                "u1",
                CommandCategory::Help,
                Outcome::Success,
            ))
            .await;
        assert_eq!(sink.gauge_value(USAGE_ACTIVE_TENANTS), 2);

        // Same tenant again — gauge should still be 2
        tracker
            .record(make_event(
                "t1",
                "u2",
                CommandCategory::Help,
                Outcome::Success,
            ))
            .await;
        assert_eq!(sink.gauge_value(USAGE_ACTIVE_TENANTS), 2);
    }

    #[test]
    fn test_outcome_display() {
        assert_eq!(Outcome::Success.to_string(), "success");
        assert_eq!(Outcome::Error.to_string(), "error");
        assert_eq!(Outcome::RateLimited.to_string(), "rate_limited");
        assert_eq!(Outcome::Unauthorized.to_string(), "unauthorized");
        assert_eq!(Outcome::InvalidCommand.to_string(), "invalid_command");
    }

    #[test]
    fn test_usage_event_new() {
        let event = UsageEvent::new(
            "t1".into(),
            "u1".into(),
            Platform::Discord,
            CommandCategory::Generate,
            Outcome::Success,
            100,
        );
        assert_eq!(event.tenant_id, "t1");
        assert_eq!(event.user_id, "u1");
        assert_eq!(event.platform, Platform::Discord);
        assert!(event.timestamp > 0);
    }

    #[test]
    fn test_tenant_summary_serialization() {
        let summary = TenantUsageSummary {
            tenant_id: "t1".to_string(),
            total_messages: 100,
            successful: 90,
            errors: 10,
            avg_latency_ms: 50,
            unique_users: 5,
        };
        let json = serde_json::to_string(&summary).expect("serialize ok");
        assert!(json.contains("\"total_messages\":100"));
    }

    #[tokio::test]
    async fn test_tenant_summary_empty() {
        let tracker = UsageTracker::new();
        let summary = tracker.tenant_summary("nonexistent").await;
        assert_eq!(summary.total_messages, 0);
        assert_eq!(summary.successful, 0);
        assert_eq!(summary.unique_users, 0);
    }

    // -----------------------------------------------------------------------
    // Integration tests: UsageTracker with real InMemoryStateStore backend
    // -----------------------------------------------------------------------

    use super::super::state_store::InMemoryStateStore;

    fn make_event_full(
        tenant: &str,
        user: &str,
        category: CommandCategory,
        outcome: Outcome,
        latency_ms: u64,
    ) -> UsageEvent {
        UsageEvent::new(
            tenant.to_string(),
            user.to_string(),
            Platform::Telegram,
            category,
            outcome,
            latency_ms,
        )
    }

    #[tokio::test]
    async fn integration_usage_tracker_with_store_backend() {
        let store: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::new());
        let tracker = UsageTracker::new().with_store(store);

        // Record several events across tenants
        for i in 0..5 {
            tracker
                .record(make_event_full(
                    "tenant-a",
                    "user-1",
                    CommandCategory::Generate,
                    Outcome::Success,
                    50 + i,
                ))
                .await;
        }
        tracker
            .record(make_event_full(
                "tenant-a",
                "user-2",
                CommandCategory::Generate,
                Outcome::Error,
                200,
            ))
            .await;
        tracker
            .record(make_event_full(
                "tenant-b",
                "user-1",
                CommandCategory::Analyze,
                Outcome::Success,
                300,
            ))
            .await;

        // Verify in-memory summaries
        let summary_a = tracker.tenant_summary("tenant-a").await;
        assert_eq!(summary_a.total_messages, 6);
        assert_eq!(summary_a.successful, 5);
        assert_eq!(summary_a.errors, 1);
        assert_eq!(summary_a.unique_users, 2);
        assert!(summary_a.avg_latency_ms > 0);

        let summary_b = tracker.tenant_summary("tenant-b").await;
        assert_eq!(summary_b.total_messages, 1);
        assert_eq!(summary_b.successful, 1);
        assert_eq!(summary_b.unique_users, 1);
    }

    #[tokio::test]
    async fn integration_usage_tracker_ring_buffer_bound() {
        let store: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::new());
        let tracker = UsageTracker::new().with_store(store);

        // Fill well beyond the ring buffer capacity
        for i in 0..15_000 {
            tracker
                .record(make_event_full(
                    "tenant-x",
                    &format!("user-{}", i % 100),
                    CommandCategory::Status,
                    Outcome::Success,
                    10,
                ))
                .await;
        }

        // Ring buffer should be bounded
        let count = tracker.event_count().await;
        assert!(
            count <= 10_000,
            "ring buffer should not exceed capacity, got {count}"
        );

        // Summary should still work
        let summary = tracker.tenant_summary("tenant-x").await;
        assert!(summary.total_messages > 0);
        assert_eq!(summary.successful, summary.total_messages);
    }

    #[tokio::test]
    async fn integration_usage_tracker_recent_events_filtered() {
        let store: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::new());
        let tracker = UsageTracker::new().with_store(store);

        tracker
            .record(make_event_full(
                "t1",
                "u1",
                CommandCategory::Help,
                Outcome::Success,
                10,
            ))
            .await;
        tracker
            .record(make_event_full(
                "t2",
                "u1",
                CommandCategory::Help,
                Outcome::Success,
                20,
            ))
            .await;
        tracker
            .record(make_event_full(
                "t1",
                "u2",
                CommandCategory::Generate,
                Outcome::Error,
                30,
            ))
            .await;

        // Filter by tenant
        let t1_events = tracker.recent_events(100, Some("t1")).await;
        assert_eq!(t1_events.len(), 2);
        assert!(t1_events.iter().all(|e| e.tenant_id == "t1"));

        // No filter
        let all_events = tracker.recent_events(100, None).await;
        assert_eq!(all_events.len(), 3);

        // Limit
        let limited = tracker.recent_events(1, None).await;
        assert_eq!(limited.len(), 1);
    }
}
