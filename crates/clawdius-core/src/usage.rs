//! Usage metering and billing support.
//!
//! Tracks token usage per tenant/session for SaaS billing.
//! Provides per-cycle aggregation and quota enforcement.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A usage record for a single API call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    /// Unique record ID.
    pub id: String,
    /// Tenant/org ID.
    pub tenant_id: String,
    /// Session ID.
    pub session_id: String,
    /// LLM provider used.
    pub provider: String,
    /// LLM model used.
    pub model: String,
    /// Input tokens consumed.
    pub input_tokens: u64,
    /// Output tokens generated.
    pub output_tokens: u64,
    /// Total tokens (input + output).
    pub total_tokens: u64,
    /// Timestamp of the API call.
    pub timestamp: DateTime<Utc>,
    /// User identifier.
    pub user_id: String,
    /// Platform the request came from.
    pub platform: Option<String>,
    /// Cost in USD (if calculated).
    pub cost_usd: Option<f64>,
}

impl UsageRecord {
    /// Create a new usage record.
    #[must_use]
    pub fn new(
        tenant_id: impl Into<String>,
        session_id: impl Into<String>,
        provider: impl Into<String>,
        model: impl Into<String>,
        input_tokens: u64,
        output_tokens: u64,
    ) -> Self {
        let total = input_tokens.saturating_add(output_tokens);
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            tenant_id: tenant_id.into(),
            session_id: session_id.into(),
            provider: provider.into(),
            model: model.into(),
            input_tokens,
            output_tokens,
            total_tokens: total,
            timestamp: Utc::now(),
            user_id: String::new(),
            platform: None,
            cost_usd: None,
        }
    }

    /// Set the user ID.
    #[must_use]
    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = user_id.into();
        self
    }

    /// Set the platform.
    #[must_use]
    pub fn with_platform(mut self, platform: impl Into<String>) -> Self {
        self.platform = Some(platform.into());
        self
    }

    /// Set the cost in USD.
    #[must_use]
    pub fn with_cost(mut self, cost: f64) -> Self {
        self.cost_usd = Some(cost);
        self
    }
}

/// Aggregated usage for a billing cycle.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageAggregation {
    /// Tenant ID.
    pub tenant_id: String,
    /// Billing cycle identifier (e.g., "2026-04").
    pub cycle: String,
    /// Total input tokens.
    pub input_tokens: u64,
    /// Total output tokens.
    pub output_tokens: u64,
    /// Total tokens.
    pub total_tokens: u64,
    /// Total cost in USD.
    pub total_cost_usd: f64,
    /// Number of API calls.
    pub api_calls: u64,
    /// Per-model breakdown.
    pub by_model: HashMap<String, ModelUsage>,
    /// Per-session breakdown.
    pub by_session: HashMap<String, u64>,
    /// First activity in cycle.
    pub first_activity: Option<DateTime<Utc>>,
    /// Last activity in cycle.
    pub last_activity: Option<DateTime<Utc>>,
}

/// Per-model usage breakdown.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub api_calls: u64,
    pub cost_usd: f64,
}

/// Tenant quota configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quota {
    /// Monthly token limit.
    pub monthly_token_limit: u64,
    /// Per-request token limit.
    pub per_request_token_limit: u64,
    /// Monthly cost limit in USD.
    pub monthly_cost_limit: f64,
    /// Rate limit (requests per minute).
    pub rate_limit_per_minute: u32,
}

impl Default for Quota {
    fn default() -> Self {
        Self {
            monthly_token_limit: 10_000_000,   // 10M tokens/month
            per_request_token_limit: 100_000,     // 100K tokens/request
            monthly_cost_limit: 100.0,            // $100/month
            rate_limit_per_minute: 60,
        }
    }
}

/// In-memory usage meter for fast token tracking.
pub struct UsageMeter {
    /// Current cycle token counts (atomic for lock-free reads).
    current_tokens: AtomicU64,
    /// Current cycle cost.
    current_cost: std::sync::atomic::AtomicU64,
    /// Current cycle API call count.
    current_calls: AtomicU64,
}

impl UsageMeter {
    /// Create a new usage meter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current_tokens: AtomicU64::new(0),
            current_cost: std::sync::atomic::AtomicU64::new(0),
            current_calls: AtomicU64::new(0),
        }
    }

    /// Record a usage event.
    pub fn record(&self, tokens: u64, cost_cents: u64) {
        self.current_tokens.fetch_add(tokens, Ordering::Relaxed);
        self.current_cost.fetch_add(cost_cents, Ordering::Relaxed);
        self.current_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current token count.
    #[must_use]
    pub fn tokens(&self) -> u64 {
        self.current_tokens.load(Ordering::Relaxed)
    }

    /// Get current cost in cents.
    #[must_use]
    pub fn cost_cents(&self) -> u64 {
        self.current_cost.load(Ordering::Relaxed)
    }

    /// Get current cost in USD.
    #[must_use]
    pub fn cost_usd(&self) -> f64 {
        self.cost_cents() as f64 / 100.0
    }

    /// Get current API call count.
    #[must_use]
    pub fn calls(&self) -> u64 {
        self.current_calls.load(Ordering::Relaxed)
    }

    /// Check if adding tokens would exceed the quota.
    #[must_use]
    pub fn would_exceed_tokens(&self, additional: u64, limit: u64) -> bool {
        self.tokens().saturating_add(additional) > limit
    }

    /// Reset the meter (called at billing cycle boundary).
    pub fn reset(&self) {
        self.current_tokens.store(0, Ordering::Relaxed);
        self.current_cost.store(0, Ordering::Relaxed);
        self.current_calls.store(0, Ordering::Relaxed);
    }
}

impl Default for UsageMeter {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-tenant usage tracker.
pub struct TenantUsageTracker {
    /// Per-tenant meters.
    meters: parking_lot::RwLock<HashMap<String, UsageMeter>>,
    /// Tenant quotas.
    quotas: parking_lot::RwLock<HashMap<String, Quota>>,
}

impl TenantUsageTracker {
    /// Create a new tenant usage tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            meters: parking_lot::RwLock::new(HashMap::new()),
            quotas: parking_lot::RwLock::new(HashMap::new()),
        }
    }

    /// Register a tenant with a quota.
    pub fn register_tenant(&self, tenant_id: impl Into<String>, quota: Quota) {
        let id = tenant_id.into();
        self.meters.write().insert(id.clone(), UsageMeter::new());
        self.quotas.write().insert(id, quota);
    }

    /// Record usage for a tenant.
    ///
    /// Returns Ok(()) if within quota, Err(QuotaExceeded) otherwise.
    pub fn record_usage(
        &self,
        tenant_id: &str,
        tokens: u64,
        cost_cents: u64,
    ) -> Result<(), QuotaExceeded> {
        // Ensure tenant exists (drop write guard before acquiring read)
        {
            let meters = self.meters.read();
            if !meters.contains_key(tenant_id) {
                drop(meters);
                let mut meters = self.meters.write();
                meters.insert(tenant_id.to_string(), UsageMeter::new());
                let mut quotas = self.quotas.write();
                quotas.insert(tenant_id.to_string(), Quota::default());
            }
        }

        let meters = self.meters.read();
        let meter = meters.get(tenant_id).unwrap();

        // Check token quota
        let quotas = self.quotas.read();
        let quota = quotas.get(tenant_id).cloned().unwrap_or_default();
        drop(quotas);

        if meter.would_exceed_tokens(tokens, quota.monthly_token_limit) {
            return Err(QuotaExceeded {
                tenant_id: tenant_id.to_string(),
                current_tokens: meter.tokens(),
                requested_tokens: tokens,
                limit: quota.monthly_token_limit,
                kind: QuotaExceededKind::Tokens,
            });
        }

        meter.record(tokens, cost_cents);
        Ok(())
    }

    /// Get usage stats for a tenant.
    #[must_use]
    pub fn get_usage(&self, tenant_id: &str) -> Option<(u64, u64, u64)> {
        let meters = self.meters.read();
        let meter = meters.get(tenant_id)?;
        Some((meter.tokens(), meter.cost_cents(), meter.calls()))
    }

    /// Reset all meters (billing cycle boundary).
    pub fn reset_all(&self) {
        for meter in self.meters.read().values() {
            meter.reset();
        }
    }

    /// List all registered tenant IDs.
    #[must_use]
    pub fn list_tenants(&self) -> Vec<String> {
        self.meters.read().keys().cloned().collect()
    }
}

impl Default for TenantUsageTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Quota exceeded error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaExceeded {
    /// Tenant that exceeded quota.
    pub tenant_id: String,
    /// Current usage.
    pub current_tokens: u64,
    /// Requested amount.
    pub requested_tokens: u64,
    /// The limit that was exceeded.
    pub limit: u64,
    /// Kind of quota exceeded.
    pub kind: QuotaExceededKind,
}

/// Kind of quota exceeded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum QuotaExceededKind {
    /// Monthly token limit exceeded.
    Tokens,
    /// Per-request token limit exceeded.
    PerRequestTokens,
    /// Monthly cost limit exceeded.
    Cost,
    /// Rate limit exceeded.
    RateLimit,
}

// ─────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_record_creation() {
        let record = UsageRecord::new(
            "tenant1",
            "session1",
            "anthropic",
            "claude-sonnet-4",
            100,
            50,
        )
        .with_user("user1")
        .with_platform("telegram");

        assert_eq!(record.input_tokens, 100);
        assert_eq!(record.output_tokens, 50);
        assert_eq!(record.total_tokens, 150);
        assert_eq!(record.user_id, "user1");
        assert_eq!(record.platform, Some("telegram".to_string()));
    }

    #[test]
    fn test_usage_meter_record() {
        let meter = UsageMeter::new();
        meter.record(1000, 50); // 1000 tokens, $0.50

        assert_eq!(meter.tokens(), 1000);
        assert_eq!(meter.cost_cents(), 50);
        assert_eq!(meter.cost_usd(), 0.50);
        assert_eq!(meter.calls(), 1);
    }

    #[test]
    fn test_usage_meter_accumulates() {
        let meter = UsageMeter::new();
        meter.record(100, 10);
        meter.record(200, 20);
        meter.record(300, 30);

        assert_eq!(meter.tokens(), 600);
        assert_eq!(meter.cost_cents(), 60);
        assert_eq!(meter.calls(), 3);
    }

    #[test]
    fn test_usage_meter_quota_check() {
        let meter = UsageMeter::new();
        meter.record(900, 0);

        assert!(!meter.would_exceed_tokens(50, 1000));
        assert!(meter.would_exceed_tokens(200, 1000));
    }

    #[test]
    fn test_usage_meter_reset() {
        let meter = UsageMeter::new();
        meter.record(500, 0);
        assert_eq!(meter.tokens(), 500);

        meter.reset();
        assert_eq!(meter.tokens(), 0);
        assert_eq!(meter.calls(), 0);
    }

    #[test]
    fn test_tenant_tracker_register() {
        let tracker = TenantUsageTracker::new();
        tracker.register_tenant("org1", Quota::default());

        assert!(tracker.list_tenants().contains(&"org1".to_string()));
    }

    #[test]
    fn test_tenant_tracker_record_within_quota() {
        let tracker = TenantUsageTracker::new();
        tracker.register_tenant("org1", Quota {
            monthly_token_limit: 1000,
            ..Default::default()
        });

        assert!(tracker.record_usage("org1", 500, 10).is_ok());
        assert!(tracker.record_usage("org1", 400, 5).is_ok());
    }

    #[test]
    fn test_tenant_tracker_quota_exceeded() {
        let tracker = TenantUsageTracker::new();
        tracker.register_tenant("org1", Quota {
            monthly_token_limit: 1000,
            ..Default::default()
        });

        tracker.record_usage("org1", 600, 0).unwrap();

        let result = tracker.record_usage("org1", 500, 0);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.tenant_id, "org1");
        assert_eq!(err.current_tokens, 600);
        assert_eq!(err.requested_tokens, 500);
        assert_eq!(err.limit, 1000);
        assert_eq!(err.kind, QuotaExceededKind::Tokens);
    }

    #[test]
    fn test_tenant_tracker_auto_register() {
        let tracker = TenantUsageTracker::new();
        // No explicit registration — auto-registers with default quota
        assert!(tracker.record_usage("new-org", 100, 5).is_ok());
        assert!(tracker.list_tenants().contains(&"new-org".to_string()));
    }

    #[test]
    fn test_tenant_tracker_get_usage() {
        let tracker = TenantUsageTracker::new();
        tracker.register_tenant("org1", Quota::default());

        tracker.record_usage("org1", 1000, 100).unwrap();

        let (tokens, cost_cents, calls) = tracker.get_usage("org1").unwrap();
        assert_eq!(tokens, 1000);
        assert_eq!(cost_cents, 100);
        assert_eq!(calls, 1);
    }

    #[test]
    fn test_tenant_tracker_reset_all() {
        let tracker = TenantUsageTracker::new();
        tracker.register_tenant("org1", Quota::default());
        tracker.register_tenant("org2", Quota::default());

        tracker.record_usage("org1", 500, 0).unwrap();
        tracker.record_usage("org2", 300, 0).unwrap();

        tracker.reset_all();

        let (t1, _, _) = tracker.get_usage("org1").unwrap();
        let (t2, _, _) = tracker.get_usage("org2").unwrap();
        assert_eq!(t1, 0);
        assert_eq!(t2, 0);
    }

    #[test]
    fn test_quota_default() {
        let quota = Quota::default();
        assert_eq!(quota.monthly_token_limit, 10_000_000);
        assert_eq!(quota.per_request_token_limit, 100_000);
        assert!((quota.monthly_cost_limit - 100.0).abs() < 0.01);
    }
}
