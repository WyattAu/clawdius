# Clawdius Test Scenarios for InvoiceNest

> This file defines specific test scenarios to exercise Clawdius's capabilities
> against the InvoiceNest testbed project. Each scenario tests different aspects
> of the agentic system.

## Setup

```bash
# From the clawdius repo root:
export OPENROUTER_API_KEY="sk-or-v1-..."

# Point clawdius at the testbed project
cd testbed/invoicenest

# Ensure rust-analyzer is running for LSP integration
rust-analyzer &

# Run individual scenarios with clawdius sprint
clawdius sprint "<task description>" --lsp "rust-analyzer"
```

---

## Scenario 1: Long Context — Full Database Schema Design
**Tests:** Long context handling, architectural reasoning, cross-file consistency

```
Task: "Design and implement the complete database schema for InvoiceNest.
Create src/db/schema.rs with all table definitions using SQLx macros.
Include: workspaces, users, clients, invoices, line_items, payments,
tax_rates, audit_log, and workspace_settings tables. Apply row-level
security patterns. Use integer cents for money. Add appropriate indexes
for common query patterns. Read the README.md for full business requirements."
```

**What to verify:**
- [ ] Agent reads README.md (long context, ~200 lines of requirements)
- [ ] All 8+ tables defined with correct relationships
- [ ] workspace_id foreign key on every tenant-scoped table
- [ ] Money stored as i64 (cents), not f64
- [ ] Indexes on common query patterns (workspace_id + status, etc.)
- [ ] Appropriate constraints (NOT NULL, UNIQUE, CHECK)
- [ ] Migration file created in migrations/

---

## Scenario 2: Specialized Agent — Authentication Module
**Tests:** LSP integration, security-sensitive code, type-safe implementation

```
Task: "Implement JWT authentication for InvoiceNest. Create the auth module
at src/auth/ with: JWT token generation and validation (15min access, 7d
refresh), password hashing with argon2, RBAC middleware with owner/admin/member
roles, login and refresh endpoints. Use the jsonwebtoken crate. Ensure tokens
are workspace-scoped. Read src/db/schema.rs for the users table structure."
```

**What to verify:**
- [ ] LSP provides schema information (agent references existing types)
- [ ] JWT tokens have correct TTLs
- [ ] Password hashing uses argon2 (not bcrypt or SHA)
- [ ] RBAC middleware correctly checks roles
- [ ] Workspace-scoped tokens (not just user-scoped)
- [ ] Error handling for expired/invalid tokens
- [ ] No hardcoded secrets

---

## Scenario 3: Concurrent Agents — Invoice + Client + Payment APIs
**Tests:** Parallel sprint execution, worktree isolation, concurrent file writes

Submit THREE parallel sprints (tests worktree isolation — no file conflicts):

```
Sprint A: "Implement the clients CRUD API. Create src/clients/handlers.rs
with list, create, get, update, delete endpoints. Include pagination,
search by name, and soft-delete support. Use Axum extractors. Read
src/db/schema.rs for the clients table."

Sprint B: "Implement the invoices CRUD API. Create src/invoices/handlers.rs
with list, create, get, update, delete endpoints. Include line item
management, status transitions (draft→sent→paid), and PDF generation
stub. Use Axum extractors. Read src/db/schema.rs for tables."

Sprint C: "Implement the payments module. Create src/payments/ with
Stripe webhook handling, payment recording, refund logic, and receipt
generation. Handle partial payments. Read src/db/schema.rs for the
payments table and README.md for business rules."
```

**What to verify:**
- [ ] All three sprints run concurrently (not sequentially)
- [ ] Each sprint operates in its own worktree (no file conflicts)
- [ ] All three modules compile together after merge
- [ ] No duplicate type definitions across modules
- [ ] Each module correctly references shared schema types
- [ ] Worktrees cleaned up after completion

---

## Scenario 4: Long Context + Business Logic — Tax Calculation Engine
**Tests:** Multi-file reasoning, business rule implementation, edge cases

```
Task: "Implement the tax calculation engine for InvoiceNest. Create
src/invoices/tax.rs with support for: US sales tax (origin-based for
physical goods, destination-based for digital), EU VAT reverse charge
(B2B exempt with valid VAT ID), Australian GST (10% flat), and
custom tax rates per workspace. Tax is per-line-item, not per-invoice.
Handle edge cases: tax-exempt items, mixed jurisdictions in one invoice,
rounding (round per-line-item, not per-invoice to avoid penny drift).
Read README.md sections on tax calculation and multi-currency."
```

**What to verify:**
- [ ] US sales tax correctly determines origin vs destination
- [ ] EU VAT reverse charge applies for B2B with valid VAT ID
- [ ] Australian GST is flat 10%
- [ ] Rounding is per-line-item (not per-invoice)
- [ ] Tax-exempt items handled correctly
- [ ] Custom workspace tax rates supported
- [ ] No floating-point arithmetic (use integer cents)

---

## Scenario 5: MCP Tools — Multi-File Edit Across Modules
**Tests:** MCP tool use, multi-file consistency, grep_search, multi_file_edit

```
Task: "Refactor InvoiceNest to use a shared error type across all modules.
Currently each module has its own error enum. Create src/api/errors.rs
with a unified AppError enum that covers: NotFound, Unauthorized,
Validation, Database, Payment, and Internal errors. Then update ALL
existing handler files to use the unified error type. Ensure all
error responses return consistent JSON format with error code,
message, and request ID."
```

**What to verify:**
- [ ] Agent uses grep_search to find all existing error types
- [ ] Agent uses multi_file_edit to update multiple files
- [ ] Unified error type covers all cases
- [ ] All handler files updated consistently
- [ ] JSON error format is consistent across endpoints
- [ ] No compilation errors after refactoring

---

## Scenario 6: Specialized Agent — Analytics Dashboard Queries
**Tests:** Complex SQL generation, aggregation logic, multi-currency

```
Task: "Implement the analytics queries for InvoiceNest. Create
src/analytics/queries.rs with functions for: monthly revenue (with
currency conversion to workspace base currency), client lifetime value,
invoice aging report (0-30, 31-60, 61-90, 90+ days), tax summary by
jurisdiction, and top clients by revenue. Use SQLx queries with proper
aggregation and JOIN clauses. Read src/db/schema.rs for the full schema."
```

**What to verify:**
- [ ] Revenue query handles currency conversion
- [ ] Aging report uses correct date buckets
- [ ] CLV calculation is accurate (total revenue / number of clients)
- [ ] Tax summary groups by jurisdiction
- [ ] Queries use appropriate indexes (EXPLAIN ANALYZE should be fast)
- [ ] No N+1 query patterns

---

## Scenario 7: LSP-Driven — API Route Registration
**Tests:** LSP symbol resolution, type-safe route handlers, auto-completion

```
Task: "Create the main API router for InvoiceNest. Create src/api/routes.rs
that registers all routes from the handler modules (auth, clients, invoices,
payments, analytics, settings). Use Axum's Router::new() with proper nesting.
Apply authentication middleware to all routes except /auth/register and
/auth/login. Apply rate limiting middleware. Apply RBAC middleware to
admin-only routes. Read all handler files in src/ to understand their
signatures."
```

**What to verify:**
- [ ] LSP provides handler function signatures
- [ ] All routes correctly reference handler functions
- [ ] Auth middleware excluded from register/login
- [ ] Rate limiting applied globally
- [ ] RBAC applied to admin-only endpoints
- [ ] No orphaned routes (every registered route has a handler)

---

## Scenario 8: Web Search — Stripe API Integration
**Tests:** Web search agent, external API documentation, up-to-date code

```
Task: "Implement Stripe payment intent creation for InvoiceNest. Create
src/payments/stripe.rs with: create payment intent, handle webhook events
(payment_succeeded, payment_failed, charge.refunded), update invoice
status on payment. Use the latest Stripe API v2. Search the web for the
current Stripe API documentation for payment intents and webhooks to
ensure we're using the correct API format."
```

**What to verify:**
- [ ] Agent searches for current Stripe API docs
- [ ] Payment intent creation uses correct parameters
- [ ] Webhook signature verification implemented
- [ ] Invoice status updated atomically with payment
- [ ] Idempotency key used for payment creation
- [ ] Error handling for declined cards, network failures

---

## Scenario 9: Full Sprint — Build Complete Invoice Flow
**Tests:** End-to-end sprint, all 7 phases, tool-use, file creation

```
Task: "Implement the complete invoice creation and sending flow:
1. Create src/invoices/service.rs with business logic for creating
   invoices from a template, adding line items, calculating totals
   (subtotal, tax, discounts, grand total), and sending via email.
2. Status transitions must follow: draft → sent → paid (with viewed,
   overdue, partial as intermediate states).
3. Email sending via SendGrid API (stub the HTTP call).
4. PDF generation stub (return empty bytes for now).
5. Write unit tests for: total calculation, tax calculation, discount
   application, status transition validation.
Read README.md and src/db/schema.rs for requirements."
```

**What to verify:**
- [ ] All 7 sprint phases execute
- [ ] Tool-use loop creates/modifies files (not just describes)
- [ ] Invoice totals calculated correctly
- [ ] Status transitions validated
- [ ] Unit tests written and pass
- [ ] No compilation errors

---

## Scenario 10: Concurrent Build + Test — Parallel CI Simulation
**Tests:** Maximum concurrency, worktree isolation, merge correctness

Submit FIVE parallel sprints simulating a CI pipeline:

```
Sprint A: "Implement src/db/mod.rs with database connection pool setup
using SQLx. Configure connection pool size, statement cache, and
timeouts. Read Cargo.toml for available dependencies."

Sprint B: "Implement src/auth/mod.rs with user registration and login
handler stubs. Return placeholder JSON responses. Read src/db/schema.rs
for user table structure."

Sprint C: "Implement src/api/errors.rs with unified error types for the
API. Cover NotFound, Unauthorized, Validation, Database, Internal
errors. Implement IntoResponse for Axum."

Sprint D: "Implement src/api/middleware.rs with rate limiting middleware
using governor crate. Apply per-IP rate limiting with configurable
requests per minute."

Sprint E: "Write integration test stubs in tests/integration/ for the
auth flow. Create a test module that sets up a test database and tests
user registration and login."
```

**What to verify:**
- [ ] All 5 sprints start concurrently
- [ ] Each in isolated worktree
- [ ] All complete without conflicts
- [ ] Combined code compiles after merge
- [ ] No duplicate module declarations

---

## OpenRouter Model Testing Matrix

| Scenario | Model | Why | Expected Quality |
|----------|-------|-----|-----------------|
| 1, 4 | `anthropic/claude-sonnet-4` | Best for long context + reasoning | High |
| 2, 6 | `anthropic/claude-haiku-3.5` | Fast, good for specialized tasks | Medium-High |
| 3, 10 | `anthropic/claude-sonnet-4` | Best for concurrent coordination | High |
| 5, 7 | `google/gemini-2.5-pro` | Large context window | Medium |
| 8 | `anthropic/claude-sonnet-4` | Best for web search + integration | High |
| 9 | `openai/gpt-4o` | Good all-rounder | Medium-High |

### Testing Commands

```bash
# Test long context with Claude Sonnet (best model)
OPENROUTER_MODEL="anthropic/claude-sonnet-4" \
clawdius sprint "$(sed -n '/Scenario 1/,/^---$/p' testbed/invoicenest/CLAWDIUS_TASKS.md | head -5)" \
  --lsp "rust-analyzer" --real-execution

# Test concurrent agents (3 parallel sprints)
# Use the parallel sprint API:
curl -X POST http://localhost:8080/api/v1/sprint/sessions \
  -H "Content-Type: application/json" \
  -d '{"task": "Sprint A from Scenario 3...", "real_execution": true}'

curl -X POST http://localhost:8080/api/v1/sprint/sessions \
  -H "Content-Type: application/json" \
  -d '{"task": "Sprint B from Scenario 3...", "real_execution": true}'

curl -X POST http://localhost:8080/api/v1/sprint/sessions \
  -H "Content-Type: application/json" \
  -d '{"task": "Sprint C from Scenario 3...", "real_execution": true}'

# Check status
curl http://localhost:8080/api/v1/sprint/sessions
```
