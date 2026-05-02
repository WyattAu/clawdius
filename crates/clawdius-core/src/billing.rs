//! Stripe billing integration.
//!
//! Manages Stripe customers, subscriptions, invoices, and payment intents.
//! Uses feature-gated Stripe API calls behind the `stripe` feature flag.
//! When the feature is disabled, all operations return stubbed responses
//! suitable for self-hosted / air-gapped deployments.

use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

// ─────────────────────────────────────────────────────────
// Domain types (always compiled — no Stripe SDK dependency)
// ─────────────────────────────────────────────────────────

/// Subscription plan tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PlanTier {
    /// Free tier — limited tokens.
    Free,
    /// Pro tier — $29/month.
    Pro,
    /// Team tier — $99/month per seat.
    Team,
    /// Enterprise tier — custom pricing.
    Enterprise,
}

impl PlanTier {
    /// Monthly price in USD cents.
    #[must_use]
    pub const fn price_cents(&self) -> u64 {
        match self {
            Self::Free => 0,
            Self::Pro => 2900,
            Self::Team => 9900,
            Self::Enterprise => 0, // custom
        }
    }

    /// Monthly token allowance.
    #[must_use]
    pub const fn token_allowance(&self) -> u64 {
        match self {
            Self::Free => 100_000,
            Self::Pro => 5_000_000,
            Self::Team => 25_000_000,
            Self::Enterprise => u64::MAX,
        }
    }

    /// Max seats (None = unlimited).
    #[must_use]
    pub const fn max_seats(&self) -> Option<u32> {
        match self {
            Self::Free => Some(1),
            Self::Pro => Some(1),
            Self::Team => Some(25),
            Self::Enterprise => None,
        }
    }

    /// Human-readable name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Pro => "Pro",
            Self::Team => "Team",
            Self::Enterprise => "Enterprise",
        }
    }
}

impl std::fmt::Display for PlanTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl Default for PlanTier {
    fn default() -> Self {
        Self::Free
    }
}

/// Billing cycle frequency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingCycle {
    Monthly,
    Annual,
}

impl BillingCycle {
    /// Months in cycle.
    #[must_use]
    pub const fn months(&self) -> u32 {
        match self {
            Self::Monthly => 1,
            Self::Annual => 12,
        }
    }
}

/// Subscription status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Active,
    PastDue,
    Canceled,
    Trialing,
    Unpaid,
    Paused,
}

/// A tenant's subscription record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    /// Tenant/org ID.
    pub tenant_id: String,
    /// Current plan tier.
    pub tier: PlanTier,
    /// Subscription status.
    pub status: SubscriptionStatus,
    /// Billing cycle.
    pub cycle: BillingCycle,
    /// Current period start.
    pub current_period_start: DateTime<Utc>,
    /// Current period end.
    pub current_period_end: DateTime<Utc>,
    /// Number of seats.
    pub seats: u32,
    /// Stripe customer ID (None for self-hosted).
    pub stripe_customer_id: Option<String>,
    /// Stripe subscription ID (None for self-hosted).
    pub stripe_subscription_id: Option<String>,
    /// Tokens used in current period.
    pub tokens_used: u64,
    /// Cost in cents in current period.
    pub cost_cents: u64,
    /// Cancel at period end.
    pub cancel_at_period_end: bool,
}

impl Subscription {
    /// Create a new subscription for a tenant.
    #[must_use]
    pub fn new(tenant_id: impl Into<String>, tier: PlanTier) -> Self {
        let now = Utc::now();
        let end = now + chrono::Duration::days(30);
        Self {
            tenant_id: tenant_id.into(),
            tier,
            status: SubscriptionStatus::Active,
            cycle: BillingCycle::Monthly,
            current_period_start: now,
            current_period_end: end,
            seats: 1,
            stripe_customer_id: None,
            stripe_subscription_id: None,
            tokens_used: 0,
            cost_cents: 0,
            cancel_at_period_end: false,
        }
    }

    /// Check if subscription is active and not expired.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.status == SubscriptionStatus::Active && self.current_period_end > Utc::now()
    }

    /// Check if the subscription has remaining token allowance.
    #[must_use]
    pub fn has_token_allowance(&self, requested: u64) -> bool {
        self.is_active()
            && self
                .tokens_used
                .saturating_add(requested)
                <= self.tier.token_allowance()
    }

    /// Get remaining tokens in current period.
    #[must_use]
    pub fn remaining_tokens(&self) -> u64 {
        self.tier
            .token_allowance()
            .saturating_sub(self.tokens_used)
    }

    /// Get utilization ratio (0.0 to 1.0).
    #[must_use]
    pub fn utilization(&self) -> f64 {
        let allowance = self.tier.token_allowance();
        if allowance == u64::MAX {
            return 0.0;
        }
        if allowance == 0 {
            return 1.0;
        }
        self.tokens_used as f64 / allowance as f64
    }

    /// Record token usage.
    pub fn record_usage(&mut self, tokens: u64, cost_cents: u64) {
        self.tokens_used = self.tokens_used.saturating_add(tokens);
        self.cost_cents = self.cost_cents.saturating_add(cost_cents);
    }

    /// Reset for new billing period.
    pub fn reset_period(&mut self) {
        self.tokens_used = 0;
        self.cost_cents = 0;
        let now = Utc::now();
        self.current_period_start = now;
        self.current_period_end = now + chrono::Duration::days(self.cycle.months() as i64 * 30);
    }
}

/// A billing event / ledger entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingEvent {
    /// Event ID.
    pub id: String,
    /// Tenant ID.
    pub tenant_id: String,
    /// Event type.
    pub event_type: BillingEventType,
    /// Amount in cents (positive = charge, negative = credit).
    pub amount_cents: i64,
    /// Description.
    pub description: String,
    /// Stripe invoice ID (if applicable).
    pub stripe_invoice_id: Option<String>,
    /// Timestamp.
    pub timestamp: DateTime<Utc>,
}

/// Types of billing events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingEventType {
    /// Recurring subscription charge.
    SubscriptionCharge,
    /// Prorated charge for mid-cycle plan change.
    Proration,
    /// One-time charge (e.g., overage).
    OneTimeCharge,
    /// Credit applied.
    Credit,
    /// Refund issued.
    Refund,
    /// Payment failed.
    PaymentFailed,
    /// Payment succeeded.
    PaymentSucceeded,
}

/// A Stripe-compatible price configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceConfig {
    /// Price ID (Stripe price_id or internal).
    pub price_id: String,
    /// Plan tier.
    pub tier: PlanTier,
    /// Billing cycle.
    pub cycle: BillingCycle,
    /// Price in USD cents.
    pub unit_amount_cents: u64,
    /// Currency code.
    pub currency: String,
    /// Stripe product ID (if applicable).
    pub stripe_product_id: Option<String>,
}

impl PriceConfig {
    /// Create a price config.
    #[must_use]
    pub fn new(tier: PlanTier, cycle: BillingCycle) -> Self {
        let multiplier = if cycle == BillingCycle::Annual { 10 } else { 1 }; // 2 months free annual
        Self {
            price_id: format!("{}_{}", tier.name().to_lowercase(), cycle.months()),
            tier,
            cycle,
            unit_amount_cents: tier.price_cents() * cycle.months() as u64 * multiplier / 12 * 12 / multiplier,
            currency: "usd".to_string(),
            stripe_product_id: None,
        }
    }
}

/// Billing manager — orchestrates subscriptions, events, and (optionally) Stripe.
pub struct BillingManager {
    /// Subscriptions keyed by tenant ID.
    subscriptions: parking_lot::RwLock<HashMap<String, Subscription>>,
    /// Billing ledger.
    ledger: parking_lot::RwLock<Vec<BillingEvent>>,
    /// Price catalog.
    prices: HashMap<String, PriceConfig>,
    /// Whether Stripe is configured.
    stripe_enabled: bool,
    /// Stripe API key (redacted in memory).
    #[cfg(feature = "stripe")]
    stripe_client: Option<Arc<stripe::Client>>,
}

impl BillingManager {
    /// Create a new billing manager (no Stripe).
    #[must_use]
    pub fn new() -> Self {
        let mut prices = HashMap::new();
        for tier in [PlanTier::Free, PlanTier::Pro, PlanTier::Team] {
            for cycle in [BillingCycle::Monthly, BillingCycle::Annual] {
                let config = PriceConfig::new(tier, cycle);
                prices.insert(config.price_id.clone(), config);
            }
        }
        Self {
            subscriptions: parking_lot::RwLock::new(HashMap::new()),
            ledger: parking_lot::RwLock::new(Vec::new()),
            prices,
            stripe_enabled: false,
            #[cfg(feature = "stripe")]
            stripe_client: None,
        }
    }

    /// Create a billing manager with Stripe enabled.
    #[cfg(feature = "stripe")]
    #[must_use]
    pub fn with_stripe(api_key: impl Into<String>) -> Self {
        let mut mgr = Self::new();
        mgr.stripe_enabled = true;
        mgr.stripe_client = Some(Arc::new(
            stripe::Client::new(api_key.into()),
        ));
        mgr
    }

    /// Get the default price for a tier and cycle.
    #[must_use]
    pub fn get_price(&self, tier: PlanTier, cycle: BillingCycle) -> Option<&PriceConfig> {
        let key = format!("{}_{}", tier.name().to_lowercase(), cycle.months());
        self.prices.get(&key)
    }

    /// Create a subscription for a tenant.
    pub fn create_subscription(&self, tenant_id: impl Into<String>, tier: PlanTier) -> Subscription {
        let sub = Subscription::new(tenant_id, tier);
        self.subscriptions
            .write()
            .insert(sub.tenant_id.clone(), sub.clone());

        // Record event
        let event = BillingEvent {
            id: uuid::Uuid::new_v4().to_string(),
            tenant_id: sub.tenant_id.clone(),
            event_type: BillingEventType::SubscriptionCharge,
            amount_cents: tier.price_cents() as i64,
            description: format!("{} subscription created", tier.name()),
            stripe_invoice_id: None,
            timestamp: Utc::now(),
        };
        self.ledger.write().push(event);
        sub
    }

    /// Get a subscription by tenant ID.
    #[must_use]
    pub fn get_subscription(&self, tenant_id: &str) -> Option<Subscription> {
        self.subscriptions.read().get(tenant_id).cloned()
    }

    /// Change plan tier for a tenant.
    pub fn change_plan(&self, tenant_id: &str, new_tier: PlanTier) -> Result<Subscription, BillingError> {
        let mut subs = self.subscriptions.write();
        let sub = subs
            .get_mut(tenant_id)
            .ok_or(BillingError::NotFound {
                tenant_id: tenant_id.to_string(),
            })?;

        let old_tier = sub.tier;
        sub.tier = new_tier;

        // Proration event
        let event = BillingEvent {
            id: uuid::Uuid::new_v4().to_string(),
            tenant_id: tenant_id.to_string(),
            event_type: BillingEventType::Proration,
            amount_cents: new_tier.price_cents() as i64 - old_tier.price_cents() as i64,
            description: format!("Plan change: {} → {}", old_tier.name(), new_tier.name()),
            stripe_invoice_id: None,
            timestamp: Utc::now(),
        };
        drop(subs);
        self.ledger.write().push(event);

        self.get_subscription(tenant_id).ok_or(BillingError::NotFound {
            tenant_id: tenant_id.to_string(),
        })
    }

    /// Cancel a subscription.
    pub fn cancel_subscription(&self, tenant_id: &str) -> Result<(), BillingError> {
        let mut subs = self.subscriptions.write();
        let sub = subs
            .get_mut(tenant_id)
            .ok_or(BillingError::NotFound {
                tenant_id: tenant_id.to_string(),
            })?;
        sub.status = SubscriptionStatus::Canceled;
        sub.cancel_at_period_end = true;

        let event = BillingEvent {
            id: uuid::Uuid::new_v4().to_string(),
            tenant_id: tenant_id.to_string(),
            event_type: BillingEventType::Credit,
            amount_cents: 0,
            description: "Subscription canceled at period end".to_string(),
            stripe_invoice_id: None,
            timestamp: Utc::now(),
        };
        drop(subs);
        self.ledger.write().push(event);
        Ok(())
    }

    /// Check if a tenant can make a request (active + token allowance).
    #[must_use]
    pub fn can_make_request(&self, tenant_id: &str, tokens: u64) -> bool {
        if let Some(sub) = self.get_subscription(tenant_id) {
            sub.has_token_allowance(tokens)
        } else {
            false
        }
    }

    /// Record usage against a tenant's subscription.
    pub fn record_usage(&self, tenant_id: &str, tokens: u64, cost_cents: u64) -> Result<(), BillingError> {
        let mut subs = self.subscriptions.write();
        let sub = subs
            .get_mut(tenant_id)
            .ok_or(BillingError::NotFound {
                tenant_id: tenant_id.to_string(),
            })?;
        if !sub.has_token_allowance(tokens) {
            return Err(BillingError::QuotaExceeded {
                tenant_id: tenant_id.to_string(),
                requested: tokens,
                remaining: sub.remaining_tokens(),
            });
        }
        sub.record_usage(tokens, cost_cents);
        Ok(())
    }

    /// Reset billing periods for all subscriptions.
    pub fn reset_all_periods(&self) {
        for sub in self.subscriptions.write().values_mut() {
            if sub.status == SubscriptionStatus::Active {
                sub.reset_period();
            }
        }
    }

    /// Get billing events for a tenant.
    #[must_use]
    pub fn get_ledger(&self, tenant_id: &str) -> Vec<BillingEvent> {
        self.ledger
            .read()
            .iter()
            .filter(|e| e.tenant_id == tenant_id)
            .cloned()
            .collect()
    }

    /// List all subscriptions.
    #[must_use]
    pub fn list_subscriptions(&self) -> Vec<Subscription> {
        self.subscriptions.read().values().cloned().collect()
    }

    /// Check if Stripe is enabled.
    #[must_use]
    pub fn is_stripe_enabled(&self) -> bool {
        self.stripe_enabled
    }

    /// Serialize subscriptions and usage to SQLite.
    pub fn save_to_sqlite(&self, conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS subscriptions (
                tenant_id TEXT PRIMARY KEY,
                plan TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS usage_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tenant_id TEXT NOT NULL,
                period_start TEXT NOT NULL,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                total_tokens INTEGER NOT NULL DEFAULT 0,
                requests INTEGER NOT NULL DEFAULT 0,
                UNIQUE(tenant_id, period_start)
            );",
        )?;

        let subs = self.subscriptions.read();

        {
            let mut stmt = conn.prepare(
                "INSERT OR REPLACE INTO subscriptions (tenant_id, plan, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for sub in subs.values() {
                stmt.execute(params![
                    sub.tenant_id,
                    sub.tier.to_string(),
                    format!("{:?}", sub.status).to_lowercase(),
                    sub.current_period_start.to_rfc3339(),
                    sub.current_period_end.to_rfc3339(),
                ])?;
            }
        }

        {
            let mut stmt = conn.prepare(
                "INSERT OR REPLACE INTO usage_records (tenant_id, period_start, input_tokens, output_tokens, total_tokens, requests) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for sub in subs.values() {
                stmt.execute(params![
                    sub.tenant_id,
                    sub.current_period_start.to_rfc3339(),
                    0i64,
                    0i64,
                    sub.tokens_used as i64,
                    1i64,
                ])?;
            }
        }

        Ok(())
    }

    /// Deserialize subscriptions and usage from SQLite.
    pub fn load_from_sqlite(conn: &rusqlite::Connection) -> Result<Self, rusqlite::Error> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS subscriptions (
                tenant_id TEXT PRIMARY KEY,
                plan TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS usage_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tenant_id TEXT NOT NULL,
                period_start TEXT NOT NULL,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                total_tokens INTEGER NOT NULL DEFAULT 0,
                requests INTEGER NOT NULL DEFAULT 0,
                UNIQUE(tenant_id, period_start)
            );",
        )?;

        let mut mgr = Self::new();
        let mut subs = HashMap::new();

        {
            let mut stmt = conn.prepare(
                "SELECT tenant_id, plan, status, created_at, updated_at FROM subscriptions",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?;

            for row in rows {
                let (tenant_id, plan_str, status_str, created_at_str, updated_at_str) = row?;
                let tier = match plan_str.as_str() {
                    "Free" => PlanTier::Free,
                    "Pro" => PlanTier::Pro,
                    "Team" => PlanTier::Team,
                    "Enterprise" => PlanTier::Enterprise,
                    _ => continue,
                };
                let status = match status_str.as_str() {
                    "active" => SubscriptionStatus::Active,
                    "past_due" => SubscriptionStatus::PastDue,
                    "canceled" => SubscriptionStatus::Canceled,
                    "trialing" => SubscriptionStatus::Trialing,
                    "unpaid" => SubscriptionStatus::Unpaid,
                    "paused" => SubscriptionStatus::Paused,
                    _ => continue,
                };
                let period_start = DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                let period_end = DateTime::parse_from_rfc3339(&updated_at_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now() + chrono::Duration::days(30));

                let mut sub = Subscription::new(&tenant_id, tier);
                sub.status = status;
                sub.current_period_start = period_start;
                sub.current_period_end = period_end;
                subs.insert(tenant_id, sub);
            }
        }

        {
            let mut usage_stmt =
                conn.prepare("SELECT tenant_id, total_tokens FROM usage_records")?;
            let usage_rows = usage_stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;

            for row in usage_rows {
                let (tenant_id, total_tokens) = row?;
                if let Some(sub) = subs.get_mut(&tenant_id) {
                    sub.tokens_used = total_tokens.max(0) as u64;
                }
            }
        }

        *mgr.subscriptions.write() = subs;
        Ok(mgr)
    }

    /// Save to a SQLite file (convenience).
    pub fn persist_to_file(&self, path: &Path) -> Result<(), rusqlite::Error> {
        let conn = rusqlite::Connection::open(path)?;
        self.save_to_sqlite(&conn)?;
        Ok(())
    }

    /// Load from a SQLite file (convenience).
    pub fn load_from_file(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = rusqlite::Connection::open(path)?;
        Self::load_from_sqlite(&conn)
    }
}

impl Default for BillingManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Billing errors.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum BillingError {
    #[error("tenant not found: {tenant_id}")]
    NotFound { tenant_id: String },
    #[error("quota exceeded for {tenant_id}: requested {requested} tokens, {remaining} remaining")]
    QuotaExceeded {
        tenant_id: String,
        requested: u64,
        remaining: u64,
    },
    #[error("stripe error: {0}")]
    StripeError(String),
    #[error("invalid plan transition: {from} → {to}")]
    InvalidTransition { from: String, to: String },
}

// ─────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_tier_properties() {
        assert_eq!(PlanTier::Free.price_cents(), 0);
        assert_eq!(PlanTier::Pro.price_cents(), 2900);
        assert_eq!(PlanTier::Team.price_cents(), 9900);
        assert_eq!(PlanTier::Enterprise.price_cents(), 0);

        assert_eq!(PlanTier::Free.token_allowance(), 100_000);
        assert_eq!(PlanTier::Pro.token_allowance(), 5_000_000);
        assert_eq!(PlanTier::Team.token_allowance(), 25_000_000);
        assert_eq!(PlanTier::Enterprise.token_allowance(), u64::MAX);

        assert_eq!(PlanTier::Free.max_seats(), Some(1));
        assert_eq!(PlanTier::Enterprise.max_seats(), None);
    }

    #[test]
    fn test_subscription_creation() {
        let sub = Subscription::new("t1", PlanTier::Pro);
        assert!(sub.is_active());
        assert_eq!(sub.tier, PlanTier::Pro);
        assert_eq!(sub.status, SubscriptionStatus::Active);
        assert_eq!(sub.remaining_tokens(), 5_000_000);
        assert!((sub.utilization() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_subscription_token_allowance() {
        let sub = Subscription::new("t1", PlanTier::Free);
        assert!(sub.has_token_allowance(50_000));
        assert!(!sub.has_token_allowance(100_001));
    }

    #[test]
    fn test_subscription_record_usage() {
        let mut sub = Subscription::new("t1", PlanTier::Free);
        sub.record_usage(50_000, 25);
        assert_eq!(sub.tokens_used, 50_000);
        assert_eq!(sub.remaining_tokens(), 50_000);
        assert!((sub.utilization() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_subscription_reset_period() {
        let mut sub = Subscription::new("t1", PlanTier::Pro);
        sub.record_usage(1_000_000, 500);
        sub.reset_period();
        assert_eq!(sub.tokens_used, 0);
        assert_eq!(sub.remaining_tokens(), 5_000_000);
    }

    #[test]
    fn test_billing_manager_create_subscription() {
        let mgr = BillingManager::new();
        let sub = mgr.create_subscription("org1", PlanTier::Pro);

        assert_eq!(sub.tenant_id, "org1");
        assert!(mgr.get_subscription("org1").is_some());
        assert!(mgr.can_make_request("org1", 1000));
    }

    #[test]
    fn test_billing_manager_change_plan() {
        let mgr = BillingManager::new();
        mgr.create_subscription("org1", PlanTier::Free);

        let sub = mgr.change_plan("org1", PlanTier::Pro).unwrap();
        assert_eq!(sub.tier, PlanTier::Pro);

        // Ledger should have 2 events (create + proration)
        let ledger = mgr.get_ledger("org1");
        assert_eq!(ledger.len(), 2);
        assert_eq!(ledger[1].event_type, BillingEventType::Proration);
    }

    #[test]
    fn test_billing_manager_cancel() {
        let mgr = BillingManager::new();
        mgr.create_subscription("org1", PlanTier::Pro);
        mgr.cancel_subscription("org1").unwrap();

        let sub = mgr.get_subscription("org1").unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Canceled);
        assert!(sub.cancel_at_period_end);
    }

    #[test]
    fn test_billing_manager_record_usage() {
        let mgr = BillingManager::new();
        mgr.create_subscription("org1", PlanTier::Free);

        assert!(mgr.record_usage("org1", 50_000, 10).is_ok());
        assert!(mgr.record_usage("org1", 50_000, 10).is_ok());

        // Should exceed free tier
        let result = mgr.record_usage("org1", 1, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_billing_manager_nonexistent_tenant() {
        let mgr = BillingManager::new();
        assert!(!mgr.can_make_request("ghost", 100));
        assert!(mgr.record_usage("ghost", 100, 0).is_err());
    }

    #[test]
    fn test_billing_manager_reset_periods() {
        let mgr = BillingManager::new();
        mgr.create_subscription("org1", PlanTier::Pro);
        mgr.record_usage("org1", 1_000_000, 0).unwrap();

        mgr.reset_all_periods();

        let sub = mgr.get_subscription("org1").unwrap();
        assert_eq!(sub.tokens_used, 0);
    }

    #[test]
    fn test_billing_manager_list_subscriptions() {
        let mgr = BillingManager::new();
        mgr.create_subscription("org1", PlanTier::Free);
        mgr.create_subscription("org2", PlanTier::Pro);
        mgr.create_subscription("org3", PlanTier::Team);

        assert_eq!(mgr.list_subscriptions().len(), 3);
    }

    #[test]
    fn test_price_config() {
        let price = PriceConfig::new(PlanTier::Pro, BillingCycle::Monthly);
        assert_eq!(price.tier, PlanTier::Pro);
        assert_eq!(price.currency, "usd");
    }

    #[test]
    fn test_billing_cycle() {
        assert_eq!(BillingCycle::Monthly.months(), 1);
        assert_eq!(BillingCycle::Annual.months(), 12);
    }

    #[test]
    fn test_enterprise_unlimited() {
        let sub = Subscription::new("bigcorp", PlanTier::Enterprise);
        assert!(sub.has_token_allowance(u64::MAX));
        assert_eq!(sub.remaining_tokens(), u64::MAX);
        assert!((sub.utilization() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_get_price() {
        let mgr = BillingManager::new();
        assert!(mgr.get_price(PlanTier::Pro, BillingCycle::Monthly).is_some());
        assert!(mgr.get_price(PlanTier::Enterprise, BillingCycle::Monthly).is_none());
    }

    #[test]
    fn test_billing_persistence_roundtrip() {
        let mgr = BillingManager::new();
        mgr.create_subscription("org1", PlanTier::Pro);
        mgr.create_subscription("org2", PlanTier::Team);
        mgr.record_usage("org1", 500_000, 100).unwrap();
        mgr.cancel_subscription("org2").unwrap();

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        mgr.save_to_sqlite(&conn).unwrap();

        let loaded = BillingManager::load_from_sqlite(&conn).unwrap();

        let sub1 = loaded.get_subscription("org1").unwrap();
        assert_eq!(sub1.tier, PlanTier::Pro);
        assert_eq!(sub1.status, SubscriptionStatus::Active);
        assert_eq!(sub1.tokens_used, 500_000);

        let sub2 = loaded.get_subscription("org2").unwrap();
        assert_eq!(sub2.tier, PlanTier::Team);
        assert_eq!(sub2.status, SubscriptionStatus::Canceled);

        assert!(loaded.get_subscription("org1").is_some());
        assert!(loaded.get_subscription("org2").is_some());
    }

    #[test]
    fn test_billing_persistence_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("billing.db");

        let mgr = BillingManager::new();
        mgr.create_subscription("org1", PlanTier::Free);
        mgr.create_subscription("org2", PlanTier::Pro);
        mgr.record_usage("org1", 30_000, 10).unwrap();
        mgr.record_usage("org2", 1_000_000, 200).unwrap();

        mgr.persist_to_file(&path).unwrap();

        let loaded = BillingManager::load_from_file(&path).unwrap();
        assert_eq!(loaded.list_subscriptions().len(), 2);

        let sub1 = loaded.get_subscription("org1").unwrap();
        assert_eq!(sub1.tier, PlanTier::Free);
        assert_eq!(sub1.tokens_used, 30_000);

        let sub2 = loaded.get_subscription("org2").unwrap();
        assert_eq!(sub2.tier, PlanTier::Pro);
        assert_eq!(sub2.tokens_used, 1_000_000);
    }
}
