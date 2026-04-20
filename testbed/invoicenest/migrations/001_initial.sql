-- InvoiceNest Database Schema
-- Migration: 001_initial
-- Multi-tenant SaaS invoicing platform

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- ============================================================================
-- WORKSPACE (Tenant)
-- ============================================================================

CREATE TABLE workspaces (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name            TEXT NOT NULL,
    slug            TEXT NOT NULL UNIQUE,
    base_currency   TEXT NOT NULL DEFAULT 'USD',
    tax_region      TEXT NOT NULL DEFAULT 'US',
    plan            TEXT NOT NULL DEFAULT 'free' CHECK (plan IN ('free', 'pro', 'business')),
    stripe_customer_id TEXT,
    branding_logo_url TEXT,
    branding_primary_color TEXT DEFAULT '#2563EB',
    branding_company_name TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE INDEX idx_workspaces_slug ON workspaces(slug) WHERE deleted_at IS NULL;
CREATE INDEX idx_workspaces_plan ON workspaces(plan) WHERE deleted_at IS NULL;

-- ============================================================================
-- USERS
-- ============================================================================

CREATE TABLE users (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email           TEXT NOT NULL UNIQUE,
    password_hash   TEXT NOT NULL,  -- argon2 hash
    display_name    TEXT NOT NULL,
    avatar_url      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE INDEX idx_users_email ON users(email) WHERE deleted_at IS NULL;

-- Workspace membership (many-to-many with role)
CREATE TABLE workspace_members (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role            TEXT NOT NULL DEFAULT 'member' CHECK (role IN ('owner', 'admin', 'member')),
    invited_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    joined_at       TIMESTAMPTZ,
    UNIQUE(workspace_id, user_id)
);

CREATE INDEX idx_workspace_members_user ON workspace_members(user_id);
CREATE INDEX idx_workspace_members_workspace ON workspace_members(workspace_id);

-- Each user can belong to multiple workspaces. The JWT token encodes
-- (user_id, workspace_id, role) so the same user can switch contexts.

-- ============================================================================
-- CLIENTS
-- ============================================================================

CREATE TABLE clients (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    email           TEXT,
    phone           TEXT,
    company         TEXT,
    address_line1   TEXT,
    address_line2   TEXT,
    city            TEXT,
    state           TEXT,
    postal_code     TEXT,
    country         TEXT NOT NULL DEFAULT 'US',
    tax_id          TEXT,  -- VAT ID, ABN, etc.
    currency        TEXT NOT NULL DEFAULT 'USD',
    notes           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE INDEX idx_clients_workspace ON clients(workspace_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_clients_name ON clients(workspace_id, name) WHERE deleted_at IS NULL;

-- ============================================================================
-- INVOICES
-- ============================================================================

CREATE TABLE invoices (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    client_id       UUID NOT NULL REFERENCES clients(id) ON DELETE RESTRICT,
    number          TEXT NOT NULL,  -- e.g., "INV-2026-001"
    status          TEXT NOT NULL DEFAULT 'draft'
                    CHECK (status IN ('draft', 'sent', 'viewed', 'overdue', 'partial', 'paid', 'cancelled')),
    currency        TEXT NOT NULL DEFAULT 'USD',
    subtotal_cents  BIGINT NOT NULL DEFAULT 0,     -- Before tax and discount
    tax_cents       BIGINT NOT NULL DEFAULT 0,     -- Total tax
    discount_cents  BIGINT NOT NULL DEFAULT 0,     -- Total discount
    total_cents     BIGINT NOT NULL DEFAULT 0,     -- Final amount
    paid_cents      BIGINT NOT NULL DEFAULT 0,     -- Amount paid so far
    due_date        DATE,
    sent_at         TIMESTAMPTZ,
    paid_at         TIMESTAMPTZ,
    viewed_at       TIMESTAMPTZ,
    reminder_count  INT NOT NULL DEFAULT 0,        -- Number of payment reminders sent
    notes           TEXT,
    terms           TEXT,                           -- Payment terms (e.g., "Net 30")
    metadata        JSONB DEFAULT '{}',             -- Extensible metadata
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE INDEX idx_invoices_workspace ON invoices(workspace_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_invoices_client ON invoices(client_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_invoices_status ON invoices(workspace_id, status) WHERE deleted_at IS NULL;
CREATE INDEX idx_invoices_due_date ON invoices(due_date) WHERE status IN ('sent', 'viewed', 'partial');
CREATE INDEX idx_invoices_number ON invoices(workspace_id, number) WHERE deleted_at IS NULL;

-- ============================================================================
-- LINE ITEMS
-- ============================================================================

CREATE TABLE line_items (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    invoice_id      UUID NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    description     TEXT NOT NULL,
    quantity        INT NOT NULL DEFAULT 1 CHECK (quantity > 0),
    unit_price_cents BIGINT NOT NULL DEFAULT 0 CHECK (unit_price_cents >= 0),
    tax_rate_cents  INT NOT NULL DEFAULT 0,       -- e.g., 1000 = 10.00%
    tax_cents       BIGINT NOT NULL DEFAULT 0,     -- Calculated: quantity * unit_price * tax_rate
    discount_cents  BIGINT NOT NULL DEFAULT 0,     -- Per-item discount
    subtotal_cents  BIGINT NOT NULL DEFAULT 0,     -- (quantity * unit_price) - discount
    sort_order      INT NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_line_items_invoice ON line_items(invoice_id);
CREATE INDEX idx_line_items_sort ON line_items(invoice_id, sort_order);

-- ============================================================================
-- PAYMENTS
-- ============================================================================

CREATE TABLE payments (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    invoice_id      UUID NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    amount_cents    BIGINT NOT NULL CHECK (amount_cents > 0),
    method          TEXT NOT NULL DEFAULT 'stripe' CHECK (method IN ('stripe', 'paypal', 'bank_transfer', 'cash', 'other')),
    status          TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'succeeded', 'failed', 'refunded')),
    stripe_payment_intent_id TEXT,
    stripe_charge_id TEXT,
    refund_id       TEXT,                           -- ID of the refund payment (if any)
    refunded_amount_cents BIGINT NOT NULL DEFAULT 0,
    description     TEXT,
    metadata        JSONB DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_payments_workspace ON payments(workspace_id);
CREATE INDEX idx_payments_invoice ON payments(invoice_id);
CREATE INDEX idx_payments_stripe ON payments(stripe_payment_intent_id) WHERE stripe_payment_intent_id IS NOT NULL;
CREATE INDEX idx_payments_status ON payments(workspace_id, status);

-- ============================================================================
-- TAX RATES (per-workspace custom rates)
-- ============================================================================

CREATE TABLE tax_rates (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,                   -- e.g., "CA Sales Tax", "EU VAT Standard"
    rate_cents      INT NOT NULL,                    -- e.g., 825 = 8.25%
    region          TEXT,                            -- Tax jurisdiction (state, country code)
    is_default      BOOLEAN NOT NULL DEFAULT FALSE,
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    effective_from  DATE NOT NULL DEFAULT CURRENT_DATE,
    effective_until DATE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_tax_rates_workspace ON tax_rates(workspace_id, is_active);

-- ============================================================================
-- AUDIT LOG (Business tier)
-- ============================================================================

CREATE TABLE audit_log (
    id              BIGSERIAL PRIMARY KEY,
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id),
    action          TEXT NOT NULL,                   -- e.g., "invoice.create", "client.delete"
    resource_type   TEXT NOT NULL,                   -- e.g., "invoice", "client", "payment"
    resource_id     UUID NOT NULL,
    changes         JSONB DEFAULT '{}',             -- {before: {...}, after: {...}}
    ip_address      INET,
    user_agent      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_log_workspace ON audit_log(workspace_id);
CREATE INDEX idx_audit_log_resource ON audit_log(resource_type, resource_id);
CREATE INDEX idx_audit_log_user ON audit_log(user_id);
CREATE INDEX idx_audit_log_created ON audit_log(created_at);

-- ============================================================================
-- WORKSPACE SETTINGS
-- ============================================================================

CREATE TABLE workspace_settings (
    workspace_id    UUID PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
    default_payment_terms TEXT NOT NULL DEFAULT 'Net 30',
    default_notes   TEXT,
    invoice_prefix  TEXT NOT NULL DEFAULT 'INV',
    next_invoice_number INT NOT NULL DEFAULT 1,
    late_fee_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    late_fee_rate_cents INT NOT NULL DEFAULT 150,  -- 1.5% = 150 basis points
    late_fee_grace_days INT NOT NULL DEFAULT 30,
    auto_reminders  BOOLEAN NOT NULL DEFAULT TRUE,
    reminder_days   INT[] NOT NULL DEFAULT '{7, 14, 30}',
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- EXCHANGE RATES (cached daily)
-- ============================================================================

CREATE TABLE exchange_rates (
    base_currency   TEXT NOT NULL,                   -- e.g., 'USD'
    quote_currency  TEXT NOT NULL,                   -- e.g., 'EUR'
    rate            NUMERIC(18, 9) NOT NULL,        -- e.g., 0.92 means 1 USD = 0.92 EUR
    fetched_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (base_currency, quote_currency)
);

CREATE INDEX idx_exchange_rates_fetched ON exchange_rates(fetched_at);

-- ============================================================================
-- ROW-LEVEL SECURITY (RLS) — Enforce multi-tenant isolation
-- ============================================================================

-- Enable RLS on all tenant-scoped tables
ALTER TABLE clients ENABLE ROW LEVEL SECURITY;
ALTER TABLE invoices ENABLE ROW LEVEL SECURITY;
ALTER TABLE line_items ENABLE ROW LEVEL SECURITY;
ALTER TABLE payments ENABLE ROW LEVEL SECURITY;
ALTER TABLE tax_rates ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_log ENABLE ROW LEVEL SECURITY;
ALTER TABLE workspace_settings ENABLE ROW LEVEL SECURITY;

-- RLS policies: users can only access data in their workspace
-- These are simplified; in production, use set_config('app.current_workspace_id', ...)
-- and check against it.

-- Example policy for clients:
-- CREATE POLICY clients_workspace_isolation ON clients
--   USING (workspace_id = current_setting('app.current_workspace_id')::uuid);
