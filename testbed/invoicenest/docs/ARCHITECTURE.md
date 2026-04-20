# InvoiceNest Architecture

## System Overview

InvoiceNest is a multi-tenant SaaS invoicing platform for freelancers and small businesses. Users create workspaces (tenants), manage clients, issue invoices, and track payments — all through a RESTful JSON API.

```
┌─────────────┐       ┌──────────────────┐       ┌──────────────┐
│   Client     │──────▶│   invoicenest-api │──────▶│  PostgreSQL  │
│  (Browser /  │  HTTP │   (Axum server)   │ SQLx  │   (RDS)      │
│   cURL / SDK)│◀──────│                   │◀──────│              │
└─────────────┘       └──────┬───────────┘       └──────────────┘
                             │
                      ┌──────▼───────────┐
                      │ invoicenest-core  │
                      │  (domain logic)   │
                      └──────┬───────────┘
                             │
                ┌────────────┼────────────┐
                ▼            ▼            ▼
          ┌──────────┐ ┌─────────┐ ┌──────────┐
          │  Stripe  │ │ Lettre  │ │ Exchange │
          │ Payments │ │ Email   │ │  Rates   │
          └──────────┘ └─────────┘ └──────────┘
```

## Crate Structure

```
invoicenest/
├── Cargo.toml                  # Workspace root
├── migrations/
│   └── 001_initial.sql         # Full schema (PostgreSQL)
├── crates/
│   ├── invoicenest-core/       # Domain logic (no HTTP)
│   │   └── src/
│   │       ├── db/             # PgPool, migrations, schema types
│   │       ├── auth/           # JWT, Argon2, RBAC
│   │       ├── clients/        # Client CRUD
│   │       ├── invoices/       # Invoice state machine, line items
│   │       ├── payments/       # Payment recording, refunds
│   │       ├── analytics/      # Revenue reports, overdue tracking
│   │       └── api/            # Shared error types, pagination
│   └── invoicenest-api/        # HTTP layer (Axum)
│       └── src/
│           └── lib.rs          # Router setup, handler stubs
└── docs/
    └── ARCHITECTURE.md
```

### `invoicenest-core`

Pure domain crate with zero Axum dependencies (except the `IntoResponse` impl on `AppError`). Owns all database interaction, business rules, and domain types.

### `invoicenest-api`

Thin HTTP layer. Handlers extract auth context, validate input, call core services, and return JSON. Depends only on `invoicenest-core`.

## Data Flow

### Invoice Creation

```
POST /api/v1/invoices
  │
  ▼
auth middleware (JWT → AuthContext)
  │
  ▼
handler: parse JSON body
  │
  ▼
core::invoices::create_invoice()
  ├─ Validate client belongs to workspace
  ├─ Generate invoice number from settings
  ├─ INSERT into invoices table
  └─ Increment next_invoice_number
  │
  ▼
POST /api/v1/invoices/:id/line-items
  │
  ▼
core::invoices::add_line_item()
  ├─ Calculate subtotal, tax per item
  ├─ INSERT into line_items
  └─ Recalculate invoice totals (SUM query)
```

### Payment Processing

```
POST /api/v1/payments  (or Stripe webhook)
  │
  ▼
core::payments::record_payment()  [in a transaction]
  ├─ INSERT into payments
  ├─ Increment invoice.paid_cents
  ├─ Determine new status (partial / paid)
  └─ UPDATE invoice status, set paid_at
```

### Authentication Flow

```
POST /auth/register
  ├─ Hash password (Argon2id)
  ├─ INSERT user
  └─ Create workspace + owner membership

POST /auth/login
  ├─ Lookup user by email
  ├─ Verify password hash
  └─ Issue JWT {sub, workspace_id, role, exp}

Every authenticated request:
  ├─ Extract Bearer token
  ├─ Verify JWT → AuthContext
  └─ Inject into request extensions
```

## Database Design

### Multi-Tenant Isolation

Every tenant-scoped table has a `workspace_id` foreign key. Row-Level Security (RLS) is enabled on all such tables. The API layer sets `app.current_workspace_id` via `SET LOCAL` at the start of each request so policies enforce isolation even if application-level checks are bypassed.

### Money

All monetary amounts are stored as `BIGINT` representing whole cents (e.g., `$12.34` → `1234`). This avoids floating-point rounding issues. The API serialises these as integer cents and the frontend formats them for display.

### Schema

See `migrations/001_initial.sql` for the full DDL. Key tables:

| Table | Purpose |
|---|---|
| `workspaces` | Tenant |
| `users` | Account |
| `workspace_members` | User ↔ Workspace (role) |
| `clients` | Customer |
| `invoices` | Invoice with status machine |
| `line_items` | Invoice line items |
| `payments` | Payment records |
| `tax_rates` | Per-workspace tax config |
| `audit_log` | Immutable action log (Business) |
| `workspace_settings` | Invoice defaults, automation |
| `exchange_rates` | Cached FX rates |

## Invoice Status Machine

```
           ┌──────────────────────────────┐
           ▼                              │
 draft ──► sent ──► viewed ──► paid      │
   │        │         │                   │
   │        ▼         ▼                   │
   │      overdue   partial ──► paid      │
   │                              │       │
   ▼                              ▼       │
 cancelled ◄──────────────────────┘───────┘
```

Transitions are validated by `core::invoices::can_transition()` before any state change is persisted.

## RBAC Model

Three roles per workspace:

| Role | Permissions |
|---|---|
| **owner** | Full access including workspace deletion |
| **admin** | Everything except workspace deletion |
| **member** | CRUD on clients/invoices, read on payments/settings/analytics |

Permissions are checked in API middleware after extracting the role from the JWT.

## Deployment Topology

```
┌─────────────────────────────────────────┐
│               AWS / Cloud               │
│                                         │
│  ┌──────────┐    ┌──────────────────┐   │
│  │  ALB /   │───▶│  ECS Fargate     │   │
│  │  CloudFlare│   │  (2+ containers) │   │
│  └──────────┘    └────────┬─────────┘   │
│                           │             │
│                  ┌────────▼─────────┐   │
│                  │  Amazon RDS       │   │
│                  │  PostgreSQL 16    │   │
│                  │  (Multi-AZ)       │   │
│                  └──────────────────┘   │
│                                         │
│  ┌──────────┐    ┌──────────────────┐   │
│  │  Stripe  │    │  S3 / CloudFront │   │
│  │  API     │    │  (PDF storage)   │   │
│  └──────────┘    └──────────────────┘   │
│                                         │
│  ┌──────────┐                           │
│  │  SES     │  (Transactional email)    │
│  └──────────┘                           │
└─────────────────────────────────────────┘
```

### Local Development

```
docker compose up -d postgres
DATABASE_URL=postgres://localhost:5432/invoicenest cargo run
```

### Environment Variables

| Variable | Description | Default |
|---|---|---|
| `DATABASE_URL` | PostgreSQL connection string | `postgres://localhost:5432/invoicenest` |
| `JWT_SECRET` | Symmetric key for signing JWTs | `dev-secret-change-me` |
| `STRIPE_SECRET_KEY` | Stripe API secret | — |
| `STRIPE_WEBHOOK_SECRET` | Stripe webhook signing secret | — |
| `RUST_LOG` | Log filter | `info` |

## Future Considerations

- **Real-time**: WebSocket notifications for invoice status changes (via `axum::extract::ws`).
- **PDF generation**: Server-side rendering of invoice PDFs (lopdf or headless Chrome).
- **Background jobs**: Scheduled overdue marking and reminder emails (tokio tasks or a dedicated worker).
- **i18n**: Localised invoice templates and email content.
- **API versioning**: URL-based (`/api/v1/`) with graceful deprecation.
