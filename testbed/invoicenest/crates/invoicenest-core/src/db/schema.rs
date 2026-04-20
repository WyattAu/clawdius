//! SQL schema types mapping directly to the database tables defined in
//! [`migrations/001_initial.sql`](../../migrations/001_initial.sql).
//!
//! Every struct derives [`sqlx::FromRow`] so it can be materialised from
//! query results. Nullable columns use `Option<T>`. Monetary amounts are
//! stored as `i64` representing whole cents.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ── Workspaces ────────────────────────────────────────────────────────────

/// Tenant / organisation. All other tenant-scoped data references a workspace.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub base_currency: String,
    pub tax_region: String,
    pub plan: WorkspacePlan,
    pub stripe_customer_id: Option<String>,
    pub branding_logo_url: Option<String>,
    pub branding_primary_color: Option<String>,
    pub branding_company_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Subscription plan tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum WorkspacePlan {
    Free,
    Pro,
    Business,
}

// ── Users & Membership ────────────────────────────────────────────────────

/// A registered user who may belong to multiple workspaces.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Many-to-many join linking a user to a workspace with a specific role.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WorkspaceMember {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub role: MembershipRole,
    pub invited_at: DateTime<Utc>,
    pub joined_at: Option<DateTime<Utc>>,
}

/// Role within a workspace governing RBAC permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum MembershipRole {
    Owner,
    Admin,
    Member,
}

// ── Clients ───────────────────────────────────────────────────────────────

/// A customer / client belonging to a workspace.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Client {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub company: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: String,
    pub tax_id: Option<String>,
    pub currency: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

// ── Invoices ──────────────────────────────────────────────────────────────

/// An invoice issued by a workspace to a client.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Invoice {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub client_id: Uuid,
    pub number: String,
    pub status: InvoiceStatus,
    pub currency: String,
    pub subtotal_cents: i64,
    pub tax_cents: i64,
    pub discount_cents: i64,
    pub total_cents: i64,
    pub paid_cents: i64,
    pub due_date: Option<NaiveDate>,
    pub sent_at: Option<DateTime<Utc>>,
    pub paid_at: Option<DateTime<Utc>>,
    pub viewed_at: Option<DateTime<Utc>>,
    pub reminder_count: i32,
    pub notes: Option<String>,
    pub terms: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Lifecycle status of an invoice. Used for the state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum InvoiceStatus {
    Draft,
    Sent,
    Viewed,
    Overdue,
    Partial,
    Paid,
    Cancelled,
}

// ── Line Items ────────────────────────────────────────────────────────────

/// A single line item on an invoice.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LineItem {
    pub id: Uuid,
    pub invoice_id: Uuid,
    pub description: String,
    pub quantity: i32,
    pub unit_price_cents: i64,
    pub tax_rate_cents: i32,
    pub tax_cents: i64,
    pub discount_cents: i64,
    pub subtotal_cents: i64,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Payments ──────────────────────────────────────────────────────────────

/// A payment (or attempted payment) against an invoice.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Payment {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub invoice_id: Uuid,
    pub amount_cents: i64,
    pub method: PaymentMethod,
    pub status: PaymentStatus,
    pub stripe_payment_intent_id: Option<String>,
    pub stripe_charge_id: Option<String>,
    pub refund_id: Option<String>,
    pub refunded_amount_cents: i64,
    pub description: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// How the payment was collected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum PaymentMethod {
    Stripe,
    Paypal,
    BankTransfer,
    Cash,
    Other,
}

/// Outcome of a payment attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum PaymentStatus {
    Pending,
    Succeeded,
    Failed,
    Refunded,
}

// ── Tax Rates ─────────────────────────────────────────────────────────────

/// A configurable tax rate scoped to a workspace.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaxRate {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub rate_cents: i32,
    pub region: Option<String>,
    pub is_default: bool,
    pub is_active: bool,
    pub effective_from: NaiveDate,
    pub effective_until: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Audit Log ─────────────────────────────────────────────────────────────

/// An immutable record of a user action within a workspace (Business plan).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditLog {
    pub id: i64,
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Uuid,
    pub changes: serde_json::Value,
    pub ip_address: Option<std::net::IpAddr>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ── Workspace Settings ────────────────────────────────────────────────────

/// Per-workspace configuration for invoicing defaults and automation.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WorkspaceSettings {
    pub workspace_id: Uuid,
    pub default_payment_terms: String,
    pub default_notes: Option<String>,
    pub invoice_prefix: String,
    pub next_invoice_number: i32,
    pub late_fee_enabled: bool,
    pub late_fee_rate_cents: i32,
    pub late_fee_grace_days: i32,
    pub auto_reminders: bool,
    pub reminder_days: Vec<i32>,
    pub updated_at: DateTime<Utc>,
}

// ── Exchange Rates ────────────────────────────────────────────────────────

/// Cached daily exchange rate between two currencies.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ExchangeRate {
    pub base_currency: String,
    pub quote_currency: String,
    pub rate: rust_decimal::Decimal,
    pub fetched_at: DateTime<Utc>,
}
