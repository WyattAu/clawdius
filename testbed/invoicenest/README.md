# InvoiceNest — Freelancer Invoicing SaaS Platform

> **A testbed project for exercising Clawdius's full capabilities: long context,
> concurrent agents, specialized agents, LSP integration, and MCP tools.**

## Business Overview

InvoiceNest is a multi-tenant SaaS platform that helps freelancers and small agencies
create, send, and track invoices. It handles payment processing, client management,
tax calculations, and financial analytics.

### Target Users

1. **Freelancers** (Free tier) — Up to 5 clients, 20 invoices/month
2. **Small Agencies** (Pro tier, $29/mo) — Up to 50 clients, unlimited invoices, team members
3. **Enterprise** (Business tier, $99/mo) — Unlimited everything, custom branding, API access, audit log

### Core Features

| Feature | Free | Pro | Business |
|---------|------|-----|---------|
| Clients | 5 | 50 | Unlimited |
| Invoices/month | 20 | Unlimited | Unlimited |
| Team members | 1 | 5 | Unlimited |
| Payment processing | Stripe only | Stripe + PayPal | All gateways |
| Custom branding | No | Yes | Yes |
| API access | No | Read-only | Full |
| Audit log | No | 30 days | Unlimited |
| Analytics | Basic | Advanced | Advanced + exports |
| Support | Email | Priority | Dedicated |
| Data retention | 90 days | 2 years | Unlimited |

### Technical Requirements

#### Architecture

- **Backend:** Rust (Axum HTTP framework, SQLx for database, JWT for auth)
- **Database:** PostgreSQL 15+ with row-level security for multi-tenancy
- **Frontend:** Not in scope for backend testbed (API-only)
- **Payments:** Stripe API v2 (credit cards, bank transfers)
- **Email:** Transactional emails via SendGrid API
- **Deployment:** Docker Compose (local), Kubernetes (production)

#### API Design

RESTful API with the following resource hierarchy:

```
/api/v1/
├── auth/
│   ├── POST /register
│   ├── POST /login
│   ├── POST /refresh
│   └── POST /logout
├── workspace/
│   ├── GET /
│   ├── PATCH /
│   └── DELETE /
├── clients/
│   ├── GET /
│   ├── POST /
│   ├── GET /:id
│   ├── PATCH /:id
│   └── DELETE /:id
├── invoices/
│   ├── GET /
│   ├── POST /
│   ├── GET /:id
│   ├── PATCH /:id
│   ├── DELETE /:id
│   ├── POST /:id/send       (email to client)
│   ├── POST /:id/pay        (record payment)
│   └── GET /:id/pdf         (generate PDF)
├── payments/
│   ├── GET /
│   ├── POST /refund/:id
│   └── GET /:id/receipt
├── analytics/
│   ├── GET /revenue
│   ├── GET /clients
│   ├── GET /aging
│   └── GET /tax-summary
└── settings/
    ├── GET /
    ├── PATCH /
    └── POST /branding
```

#### Database Schema

See `migrations/001_initial.sql` for the full schema. Key design decisions:

1. **Multi-tenancy via `workspace_id`** — Every table has a `workspace_id` foreign key.
   Row-Level Security (RLS) policies ensure tenants can only access their own data.

2. **Invoice status machine:**
   ```
   draft → sent → viewed → paid
                  ↓       ↓
               overdue  partial
                  ↓       ↓
               cancelled  paid
   ```

3. **Money stored as integers** (cents) to avoid floating-point issues.

4. **Soft deletes** on clients and invoices (`deleted_at` column).

5. **Audit trail** — All mutations logged to `audit_log` table (Business tier only).

#### Business Logic Rules

1. **Tax calculation:** Support for multiple tax jurisdictions. US sales tax based on
   client address. EU VAT reverse-charge for B2B. Australian GST (10%).
   Tax is calculated per line item, not per invoice.

2. **Late payment:** Invoices overdue >30 days are flagged. Interest at 1.5%/month
   (configurable per workspace). Automated reminder emails at 7, 14, 30 days overdue.

3. **Discount logic:** Two types:
   - **Percentage discount** on entire invoice (e.g., 10% off for early payment)
   - **Fixed discount** per line item (e.g., -$50 setup fee)

4. **Multi-currency:** Invoices can be in any currency. Exchange rates fetched from
   Open Exchange Rates API daily. Analytics always reported in workspace's base currency.

5. **Payment reconciliation:** Stripe webhooks update invoice status. Partial payments
   tracked. Refunds reduce invoice total (cannot go negative).

6. **Rate limiting:** API rate limits per tier:
   - Free: 100 req/min
   - Pro: 1000 req/min
   - Business: 5000 req/min

### Security Requirements

1. **Authentication:** JWT access tokens (15min TTL) + refresh tokens (7 day TTL).
   Tokens are workspace-scoped (a user can belong to multiple workspaces).

2. **Authorization:** RBAC with 3 roles:
   - `owner` — Full access, can delete workspace
   - `admin` — Manage clients, invoices, settings. Cannot delete workspace.
   - `member` — Create/edit invoices only. Read-only on settings.

3. **Input validation:** All monetary amounts validated as non-negative integers.
   Email addresses validated via regex. API keys validated format before DB lookup.

4. **Encryption:** Database encryption at rest (PostgreSQL pgcrypto). TLS in transit.
   Stripe API keys encrypted with workspace-specific key.

### Testing Requirements

- **Unit tests:** All business logic functions (tax calc, discount, status transitions)
- **Integration tests:** API endpoints with test database (SQLite for CI, Postgres for staging)
- **Property-based tests:** Tax calculations, discount edge cases (Proptest)
- **Load tests:** Invoice listing with 10K records (Criterion benchmarking)
- **Security tests:** SQL injection, XSS, unauthorized access attempts

### Key Business Decisions to Make

These are intentionally ambiguous — the agent should reason through them:

1. **Should `deleted_at` invoices count toward the monthly limit?**
   (Argument for: prevents abuse. Argument against: confuses users.)

2. **How to handle currency conversion for partial refunds?**
   (Use rate at time of payment? Rate at time of refund? Average?)

3. **Should team members see each other's draft invoices?**
   (Argument for: collaboration. Argument against: privacy.)

4. **When a workspace is downgraded from Pro to Free, what happens to excess clients?**
   (Block creation? Archive oldest? Require user to choose which to keep?)

5. **Should the audit log be immutable?**
   (Append-only vs. allowing corrections. GDPR right-to-erasure implications.)

6. **How to handle timezone-aware due dates?**
   (Store in UTC and convert? Store in local timezone? What about DST transitions?)
