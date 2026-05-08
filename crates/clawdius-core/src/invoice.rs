//! Invoice generation for SaaS billing.
//!
//! Produces structured invoice records from billing events and usage data.
//! Supports JSON/CSV export and PDF-ready data structures.
//! Self-hosted deployments can use this without Stripe.

use crate::billing::{BillingEvent, BillingEventType, PlanTier, Subscription};
use crate::usage::{UsageAggregation, UsageRecord};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An invoice for a billing period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    /// Invoice ID (format: INV-YYYYMMDD-TENANTID).
    pub id: String,
    /// Tenant/org ID.
    pub tenant_id: String,
    /// Billing cycle (e.g., "2026-04").
    pub cycle: String,
    /// Invoice status.
    pub status: InvoiceStatus,
    /// Plan tier at time of invoice.
    pub tier: String,
    /// Seats billed.
    pub seats: u32,
    /// Invoice date.
    pub issued_at: DateTime<Utc>,
    /// Due date.
    pub due_at: DateTime<Utc>,
    /// Line items.
    pub line_items: Vec<LineItem>,
    /// Subtotal in cents.
    pub subtotal_cents: i64,
    /// Tax in cents.
    pub tax_cents: i64,
    /// Total in cents.
    pub total_cents: i64,
    /// Currency code.
    pub currency: String,
    /// Billing events included.
    pub event_ids: Vec<String>,
    /// Notes.
    pub notes: String,
    /// Stripe invoice ID (if applicable).
    pub stripe_invoice_id: Option<String>,
}

/// Invoice status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceStatus {
    /// Draft — not yet issued.
    Draft,
    /// Issued — awaiting payment.
    Pending,
    /// Paid.
    Paid,
    /// Past due.
    Overdue,
    /// Voided.
    Void,
    /// Refunded.
    Refunded,
}

impl std::fmt::Display for InvoiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => f.write_str("draft"),
            Self::Pending => f.write_str("pending"),
            Self::Paid => f.write_str("paid"),
            Self::Overdue => f.write_str("overdue"),
            Self::Void => f.write_str("void"),
            Self::Refunded => f.write_str("refunded"),
        }
    }
}

/// A line item on an invoice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineItem {
    /// Line item description.
    pub description: String,
    /// Quantity.
    pub quantity: u64,
    /// Unit price in cents.
    pub unit_price_cents: i64,
    /// Total for this line in cents.
    pub total_cents: i64,
    /// Line item type.
    pub kind: LineItemKind,
}

/// Types of invoice line items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineItemKind {
    /// Base subscription fee.
    Subscription,
    /// Additional seats.
    Seats,
    /// Token overage.
    Overage,
    /// Proration credit/debit.
    Proration,
    /// One-time charge.
    Charge,
    /// Credit.
    Credit,
    /// Tax.
    Tax,
}

/// Invoice generation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceConfig {
    /// Payment terms in days.
    pub payment_terms_days: u32,
    /// Tax rate as decimal (e.g., 0.08 for 8%).
    pub tax_rate: f64,
    /// Currency code.
    pub currency: String,
    /// Overage price per 1K tokens in cents.
    pub overage_price_per_1k_tokens_cents: i64,
    /// Company name for invoices.
    pub company_name: String,
    /// Company address.
    pub company_address: String,
}

impl Default for InvoiceConfig {
    fn default() -> Self {
        Self {
            payment_terms_days: 30,
            tax_rate: 0.0,
            currency: "usd".to_string(),
            overage_price_per_1k_tokens_cents: 30, // $0.30 per 1K tokens
            company_name: "Clawdius Inc.".to_string(),
            company_address: String::new(),
        }
    }
}

/// Invoice generator.
pub struct InvoiceGenerator {
    config: InvoiceConfig,
}

impl InvoiceGenerator {
    /// Create with default config.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: InvoiceConfig::default(),
        }
    }

    /// Create with custom config.
    #[must_use]
    pub fn with_config(config: InvoiceConfig) -> Self {
        Self { config }
    }

    /// Generate an invoice from a subscription and billing events.
    ///
    /// # Arguments
    /// * `subscription` - The tenant's subscription
    /// * `events` - Billing events for the period
    /// * `usage_records` - Usage records for the period
    pub fn generate(
        &self,
        subscription: &Subscription,
        events: &[BillingEvent],
        usage_records: &[UsageRecord],
    ) -> Invoice {
        let mut line_items = Vec::new();
        let mut event_ids = Vec::new();

        // Base subscription charge
        let base_price = subscription.tier.price_cents() as i64;
        if base_price > 0 {
            line_items.push(LineItem {
                description: format!("{} subscription ({} cycle)", subscription.tier, {
                    match subscription.cycle {
                        crate::billing::BillingCycle::Monthly => "monthly",
                        crate::billing::BillingCycle::Annual => "annual",
                    }
                }),
                quantity: 1,
                unit_price_cents: base_price,
                total_cents: base_price,
                kind: LineItemKind::Subscription,
            });
        }

        // Additional seats (beyond first)
        if subscription.seats > 1 {
            let seat_price = base_price / 2; // 50% per additional seat
            let extra_seats = (subscription.seats - 1) as u64;
            line_items.push(LineItem {
                description: format!("Additional seats ({} × {})", extra_seats, subscription.tier),
                quantity: extra_seats,
                unit_price_cents: seat_price,
                total_cents: seat_price * extra_seats as i64,
                kind: LineItemKind::Seats,
            });
        }

        // Process billing events
        for event in events {
            event_ids.push(event.id.clone());
            match event.event_type {
                BillingEventType::Proration => {
                    line_items.push(LineItem {
                        description: event.description.clone(),
                        quantity: 1,
                        unit_price_cents: event.amount_cents,
                        total_cents: event.amount_cents,
                        kind: LineItemKind::Proration,
                    });
                },
                BillingEventType::OneTimeCharge => {
                    line_items.push(LineItem {
                        description: event.description.clone(),
                        quantity: 1,
                        unit_price_cents: event.amount_cents,
                        total_cents: event.amount_cents,
                        kind: LineItemKind::Charge,
                    });
                },
                BillingEventType::Credit | BillingEventType::Refund => {
                    line_items.push(LineItem {
                        description: event.description.clone(),
                        quantity: 1,
                        unit_price_cents: event.amount_cents,
                        total_cents: event.amount_cents,
                        kind: LineItemKind::Credit,
                    });
                },
                _ => {
                    // SubscriptionCharge, PaymentFailed, PaymentSucceeded — no line item
                },
            }
        }

        // Token overage
        let allowance = subscription.tier.token_allowance();
        let tokens_used = usage_records.iter().map(|r| r.total_tokens).sum::<u64>();
        if tokens_used > allowance && allowance != u64::MAX {
            let overage_tokens = tokens_used - allowance;
            let overage_1k_units = (overage_tokens + 999) / 1000; // ceiling division
            let overage_cost =
                overage_1k_units as i64 * self.config.overage_price_per_1k_tokens_cents;
            if overage_cost > 0 {
                line_items.push(LineItem {
                    description: format!(
                        "Token overage ({} tokens beyond {} allowance)",
                        overage_tokens, allowance
                    ),
                    quantity: overage_1k_units,
                    unit_price_cents: self.config.overage_price_per_1k_tokens_cents,
                    total_cents: overage_cost,
                    kind: LineItemKind::Overage,
                });
            }
        }

        // Calculate totals
        let subtotal: i64 = line_items.iter().map(|li| li.total_cents).sum();
        let tax_cents = (subtotal as f64 * self.config.tax_rate).round() as i64;
        let total = subtotal + tax_cents;

        let now = Utc::now();
        let due = now + chrono::Duration::days(self.config.payment_terms_days as i64);
        let cycle = now.format("%Y-%m").to_string();

        Invoice {
            id: format!(
                "INV-{}-{}",
                now.format("%Y%m%d"),
                subscription.tenant_id.to_uppercase()
            ),
            tenant_id: subscription.tenant_id.clone(),
            cycle,
            status: InvoiceStatus::Pending,
            tier: subscription.tier.to_string(),
            seats: subscription.seats,
            issued_at: now,
            due_at: due,
            line_items,
            subtotal_cents: subtotal,
            tax_cents,
            total_cents: total,
            currency: self.config.currency.clone(),
            event_ids,
            notes: format!("Tokens used: {}. Plan: {}.", tokens_used, subscription.tier),
            stripe_invoice_id: None,
        }
    }

    /// Generate a CSV string from an invoice.
    pub fn to_csv(&self, invoice: &Invoice) -> String {
        let mut csv = String::new();
        csv.push_str(
            "Invoice ID,Tenant,Cycle,Status,Tier,Seats,Subtotal,Tax,Total,Currency,Due Date\n",
        );
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            invoice.id,
            invoice.tenant_id,
            invoice.cycle,
            invoice.status,
            invoice.tier,
            invoice.seats,
            invoice.subtotal_cents,
            invoice.tax_cents,
            invoice.total_cents,
            invoice.currency,
            invoice.due_at.format("%Y-%m-%d"),
        ));
        csv.push_str("\nLine Items\n");
        csv.push_str("Description,Quantity,Unit Price,Total,Kind\n");
        for item in &invoice.line_items {
            csv.push_str(&format!(
                "\"{}\",{},{},{},{}\n",
                item.description,
                item.quantity,
                item.unit_price_cents,
                item.total_cents,
                match item.kind {
                    LineItemKind::Subscription => "subscription",
                    LineItemKind::Seats => "seats",
                    LineItemKind::Overage => "overage",
                    LineItemKind::Proration => "proration",
                    LineItemKind::Charge => "charge",
                    LineItemKind::Credit => "credit",
                    LineItemKind::Tax => "tax",
                },
            ));
        }
        csv
    }

    /// Get the invoice config.
    #[must_use]
    pub fn config(&self) -> &InvoiceConfig {
        &self.config
    }
}

impl Default for InvoiceGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::billing::BillingCycle;

    fn test_subscription() -> Subscription {
        Subscription::new("org1", PlanTier::Pro)
    }

    #[test]
    fn test_generate_basic_invoice() {
        let gen = InvoiceGenerator::new();
        let sub = test_subscription();
        let invoice = gen.generate(&sub, &[], &[]);

        assert!(invoice.id.starts_with("INV-"));
        assert_eq!(invoice.tenant_id, "org1");
        assert_eq!(invoice.tier, "Pro");
        assert_eq!(invoice.status, InvoiceStatus::Pending);
        assert!(invoice.subtotal_cents > 0);
        assert_eq!(invoice.total_cents, invoice.subtotal_cents); // no tax
        assert_eq!(invoice.currency, "usd");
        assert!(!invoice.line_items.is_empty());
    }

    #[test]
    fn test_free_tier_invoice() {
        let gen = InvoiceGenerator::new();
        let sub = Subscription::new("org1", PlanTier::Free);
        let invoice = gen.generate(&sub, &[], &[]);

        // Free tier has $0 base price — no subscription line item
        let sub_items: Vec<_> = invoice
            .line_items
            .iter()
            .filter(|li| li.kind == LineItemKind::Subscription)
            .collect();
        assert!(sub_items.is_empty());
        assert_eq!(invoice.total_cents, 0);
    }

    #[test]
    fn test_additional_seats() {
        let gen = InvoiceGenerator::new();
        let mut sub = test_subscription();
        sub.seats = 3;
        let invoice = gen.generate(&sub, &[], &[]);

        let seat_items: Vec<_> = invoice
            .line_items
            .iter()
            .filter(|li| li.kind == LineItemKind::Seats)
            .collect();
        assert_eq!(seat_items.len(), 1);
        assert_eq!(seat_items[0].quantity, 2); // 3 seats - 1 included
    }

    #[test]
    fn test_token_overage() {
        let gen = InvoiceGenerator::with_config(InvoiceConfig {
            overage_price_per_1k_tokens_cents: 30,
            ..Default::default()
        });

        let sub = Subscription::new("org1", PlanTier::Free); // 100K allowance
        let records = vec![UsageRecord::new("org1", "s1", "test", "m1", 200_000, 0)];
        let invoice = gen.generate(&sub, &[], &records);

        let overage_items: Vec<_> = invoice
            .line_items
            .iter()
            .filter(|li| li.kind == LineItemKind::Overage)
            .collect();
        assert_eq!(overage_items.len(), 1);
        // 100K overage = 100 units of 1K = $3.00 = 300 cents
        assert_eq!(overage_items[0].quantity, 100);
        assert_eq!(overage_items[0].total_cents, 3000);
    }

    #[test]
    fn test_no_overage_within_allowance() {
        let gen = InvoiceGenerator::new();
        let sub = Subscription::new("org1", PlanTier::Pro); // 5M allowance
        let records = vec![UsageRecord::new("org1", "s1", "test", "m1", 1_000_000, 0)];
        let invoice = gen.generate(&sub, &[], &records);

        let overage_items: Vec<_> = invoice
            .line_items
            .iter()
            .filter(|li| li.kind == LineItemKind::Overage)
            .collect();
        assert!(overage_items.is_empty());
    }

    #[test]
    fn test_proration_line_item() {
        let gen = InvoiceGenerator::new();
        let sub = test_subscription();
        let events = vec![BillingEvent {
            id: "evt1".to_string(),
            tenant_id: "org1".to_string(),
            event_type: BillingEventType::Proration,
            amount_cents: -500,
            description: "Upgrade credit".to_string(),
            stripe_invoice_id: None,
            timestamp: Utc::now(),
        }];
        let invoice = gen.generate(&sub, &events, &[]);

        let proration_items: Vec<_> = invoice
            .line_items
            .iter()
            .filter(|li| li.kind == LineItemKind::Proration)
            .collect();
        assert_eq!(proration_items.len(), 1);
        assert_eq!(proration_items[0].total_cents, -500);
        assert!(invoice.event_ids.contains(&"evt1".to_string()));
    }

    #[test]
    fn test_tax_calculation() {
        let gen = InvoiceGenerator::with_config(InvoiceConfig {
            tax_rate: 0.08,
            ..Default::default()
        });
        let sub = test_subscription();
        let invoice = gen.generate(&sub, &[], &[]);

        // Pro = $29.00 = 2900 cents. Tax = 232 cents.
        assert_eq!(invoice.subtotal_cents, 2900);
        assert_eq!(invoice.tax_cents, 232);
        assert_eq!(invoice.total_cents, 3132);
    }

    #[test]
    fn test_credit_line_item() {
        let gen = InvoiceGenerator::new();
        let sub = test_subscription();
        let events = vec![BillingEvent {
            id: "evt2".to_string(),
            tenant_id: "org1".to_string(),
            event_type: BillingEventType::Credit,
            amount_cents: -1000,
            description: "Service credit".to_string(),
            stripe_invoice_id: None,
            timestamp: Utc::now(),
        }];
        let invoice = gen.generate(&sub, &events, &[]);

        // 2900 - 1000 = 1900 subtotal
        assert_eq!(invoice.subtotal_cents, 1900);
    }

    #[test]
    fn test_enterprise_no_overage() {
        let gen = InvoiceGenerator::new();
        let sub = Subscription::new("bigcorp", PlanTier::Enterprise);
        let records = vec![UsageRecord::new(
            "bigcorp",
            "s1",
            "test",
            "m1",
            100_000_000,
            0,
        )];
        let invoice = gen.generate(&sub, &[], &records);

        // Enterprise has unlimited tokens — no overage
        let overage_items: Vec<_> = invoice
            .line_items
            .iter()
            .filter(|li| li.kind == LineItemKind::Overage)
            .collect();
        assert!(overage_items.is_empty());
        // Enterprise has $0 base price
        assert_eq!(invoice.total_cents, 0);
    }

    #[test]
    fn test_csv_output() {
        let gen = InvoiceGenerator::new();
        let sub = test_subscription();
        let invoice = gen.generate(&sub, &[], &[]);
        let csv = gen.to_csv(&invoice);

        assert!(csv.contains("INV-"));
        assert!(csv.contains("org1"));
        assert!(csv.contains("subscription"));
        assert!(csv.contains("Line Items"));
    }

    #[test]
    fn test_invoice_serialization() {
        let gen = InvoiceGenerator::new();
        let sub = test_subscription();
        let invoice = gen.generate(&sub, &[], &[]);

        let json = serde_json::to_string(&invoice).unwrap();
        let deserialized: Invoice = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, invoice.id);
        assert_eq!(deserialized.tenant_id, invoice.tenant_id);
    }

    #[test]
    fn test_invoice_status_display() {
        assert_eq!(InvoiceStatus::Draft.to_string(), "draft");
        assert_eq!(InvoiceStatus::Pending.to_string(), "pending");
        assert_eq!(InvoiceStatus::Paid.to_string(), "paid");
        assert_eq!(InvoiceStatus::Overdue.to_string(), "overdue");
    }

    #[test]
    fn test_invoice_due_date() {
        let gen = InvoiceGenerator::with_config(InvoiceConfig {
            payment_terms_days: 15,
            ..Default::default()
        });
        let sub = test_subscription();
        let invoice = gen.generate(&sub, &[], &[]);

        let expected_due = invoice.issued_at + chrono::Duration::days(15);
        assert_eq!(invoice.due_at, expected_due);
    }

    #[test]
    fn test_overage_rounding() {
        let gen = InvoiceGenerator::with_config(InvoiceConfig {
            overage_price_per_1k_tokens_cents: 30,
            ..Default::default()
        });
        let sub = Subscription::new("org1", PlanTier::Free);
        // 100,500 tokens used — 500 overage = 1 unit of 1K (ceiling)
        let records = vec![UsageRecord::new("org1", "s1", "test", "m1", 100_500, 0)];
        let invoice = gen.generate(&sub, &[], &records);

        let overage_items: Vec<_> = invoice
            .line_items
            .iter()
            .filter(|li| li.kind == LineItemKind::Overage)
            .collect();
        assert_eq!(overage_items.len(), 1);
        assert_eq!(overage_items[0].quantity, 1); // ceiling(500/1000) = 1
    }
}
