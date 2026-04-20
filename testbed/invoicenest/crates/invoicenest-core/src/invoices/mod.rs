//! Invoice service with status machine, line-item management, and totals.
//!
//! An invoice progresses through a well-defined state machine:
//!
//! ```text
//! draft ──► sent ──► viewed ──► paid
//!   │        │         │
//!   │        ▼         ▼
//!   │      overdue   partial ──► paid
//!   │                            │
//!   ▼                            ▼
//! cancelled ◄────────────────────┘
//! ```
//!
//! Totals (subtotal, tax, discount, grand total) are recalculated
//! from the attached line items whenever items change.

use chrono::NaiveDate;
use uuid::Uuid;

use crate::db::schema::{Invoice, InvoiceStatus, LineItem};
use crate::db::PgPool;
use crate::AppError;

/// Parameters for creating a new invoice.
#[derive(Debug, Clone)]
pub struct CreateInvoice {
    pub client_id: Uuid,
    pub currency: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub terms: Option<String>,
}

/// Parameters for adding a line item to an invoice.
#[derive(Debug, Clone)]
pub struct CreateLineItem {
    pub description: String,
    pub quantity: i32,
    pub unit_price_cents: i64,
    pub tax_rate_cents: i32,
    pub discount_cents: i64,
    pub sort_order: i32,
}

/// Valid status transitions. Returns the next status if the transition
/// is legal, or `None` if it is not.
pub fn can_transition(from: InvoiceStatus, to: InvoiceStatus) -> bool {
    matches!(
        (from, to),
        (InvoiceStatus::Draft, InvoiceStatus::Sent)
            | (InvoiceStatus::Draft, InvoiceStatus::Cancelled)
            | (InvoiceStatus::Sent, InvoiceStatus::Viewed)
            | (InvoiceStatus::Sent, InvoiceStatus::Paid)
            | (InvoiceStatus::Sent, InvoiceStatus::Cancelled)
            | (InvoiceStatus::Sent, InvoiceStatus::Partial)
            | (InvoiceStatus::Viewed, InvoiceStatus::Paid)
            | (InvoiceStatus::Viewed, InvoiceStatus::Partial)
            | (InvoiceStatus::Viewed, InvoiceStatus::Cancelled)
            | (InvoiceStatus::Overdue, InvoiceStatus::Paid)
            | (InvoiceStatus::Overdue, InvoiceStatus::Partial)
            | (InvoiceStatus::Overdue, InvoiceStatus::Cancelled)
            | (InvoiceStatus::Partial, InvoiceStatus::Paid)
            | (InvoiceStatus::Partial, InvoiceStatus::Cancelled)
    )
}

/// Transitions an invoice to a new status, returning the updated row.
///
/// # Errors
///
/// Returns [`AppError::Validation`] if the transition is illegal,
/// or [`AppError::NotFound`] if the invoice does not exist.
pub async fn transition_status(
    pool: &PgPool,
    workspace_id: Uuid,
    invoice_id: Uuid,
    new_status: InvoiceStatus,
) -> Result<Invoice, AppError> {
    let invoice = get_invoice(pool, workspace_id, invoice_id).await?;

    if !can_transition(invoice.status, new_status) {
        return Err(AppError::Validation(format!(
            "Cannot transition from {:?} to {:?}",
            invoice.status, new_status
        )));
    }

    let row = sqlx::query_as!(
        Invoice,
        r#"
        UPDATE invoices SET
            status = $3,
            updated_at = NOW()
        WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL
        RETURNING *
        "#,
        invoice_id,
        workspace_id,
        new_status as InvoiceStatus,
    )
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Creates a new draft invoice and assigns the next invoice number
/// based on workspace settings.
pub async fn create_invoice(
    pool: &PgPool,
    workspace_id: Uuid,
    input: CreateInvoice,
) -> Result<Invoice, AppError> {
    let currency = input.currency.unwrap_or_else(|| "USD".into());

    let row = sqlx::query_as!(
        Invoice,
        r#"
        INSERT INTO invoices (workspace_id, client_id, number, status, currency,
                              due_date, notes, terms)
        VALUES ($1, $2,
                (SELECT invoice_prefix || '-' || TO_CHAR(NOW(), 'YYYY') || '-' || LPAD(next_invoice_number::TEXT, 3, '0')
                 FROM workspace_settings WHERE workspace_id = $1),
                'draft', $3, $4, $5, $6)
        RETURNING *
        "#,
        workspace_id,
        input.client_id,
        currency,
        input.due_date,
        input.notes,
        input.terms,
    )
    .fetch_one(pool)
    .await?;

    // Bump the invoice number counter
    sqlx::query!(
        r#"
        UPDATE workspace_settings
        SET next_invoice_number = next_invoice_number + 1
        WHERE workspace_id = $1
        "#,
        workspace_id,
    )
    .execute(pool)
    .await?;

    Ok(row)
}

/// Retrieves a single invoice by ID.
pub async fn get_invoice(
    pool: &PgPool,
    workspace_id: Uuid,
    invoice_id: Uuid,
) -> Result<Invoice, AppError> {
    let row = sqlx::query_as!(
        Invoice,
        r#"
        SELECT * FROM invoices
        WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL
        "#,
        invoice_id,
        workspace_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Invoice {invoice_id} not found")))?;

    Ok(row)
}

/// Lists invoices in a workspace, optionally filtered by status.
pub async fn list_invoices(
    pool: &PgPool,
    workspace_id: Uuid,
    status: Option<InvoiceStatus>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Invoice>, AppError> {
    let rows = if let Some(st) = status {
        sqlx::query_as!(
            Invoice,
            r#"
            SELECT * FROM invoices
            WHERE workspace_id = $1 AND status = $2 AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
            workspace_id,
            st as InvoiceStatus,
            limit,
            offset,
        )
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as!(
            Invoice,
            r#"
            SELECT * FROM invoices
            WHERE workspace_id = $1 AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            workspace_id,
            limit,
            offset,
        )
        .fetch_all(pool)
        .await?
    };

    Ok(rows)
}

/// Adds a line item to an invoice and recalculates totals.
pub async fn add_line_item(
    pool: &PgPool,
    workspace_id: Uuid,
    invoice_id: Uuid,
    input: CreateLineItem,
) -> Result<LineItem, AppError> {
    let subtotal_cents = (input.quantity as i64 * input.unit_price_cents)
        .saturating_sub(input.discount_cents);
    let tax_cents = (subtotal_cents * input.tax_rate_cents as i64) / 10_000;

    let row = sqlx::query_as!(
        LineItem,
        r#"
        INSERT INTO line_items (invoice_id, description, quantity,
                                unit_price_cents, tax_rate_cents,
                                tax_cents, discount_cents, subtotal_cents,
                                sort_order)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING *
        "#,
        invoice_id,
        input.description,
        input.quantity,
        input.unit_price_cents,
        input.tax_rate_cents,
        tax_cents,
        input.discount_cents,
        subtotal_cents,
        input.sort_order,
    )
    .fetch_one(pool)
    .await?;

    recalculate_invoice_totals(pool, workspace_id, invoice_id).await?;

    Ok(row)
}

/// Removes a line item and recalculates totals.
pub async fn delete_line_item(
    pool: &PgPool,
    workspace_id: Uuid,
    invoice_id: Uuid,
    line_item_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        DELETE FROM line_items WHERE id = $1 AND invoice_id = $2
        "#,
        line_item_id,
        invoice_id,
    )
    .execute(pool)
    .await?;

    recalculate_invoice_totals(pool, workspace_id, invoice_id).await?;
    Ok(())
}

/// Recalculates subtotal, tax, and total from line items and writes
/// the result back to the invoice row.
async fn recalculate_invoice_totals(
    pool: &PgPool,
    workspace_id: Uuid,
    invoice_id: Uuid,
) -> Result<(), AppError> {
    let _invoice = get_invoice(pool, workspace_id, invoice_id).await?;

    sqlx::query!(
        r#"
        UPDATE invoices SET
            subtotal_cents = COALESCE((SELECT SUM(subtotal_cents) FROM line_items WHERE invoice_id = $1), 0),
            tax_cents      = COALESCE((SELECT SUM(tax_cents) FROM line_items WHERE invoice_id = $1), 0),
            discount_cents = COALESCE((SELECT SUM(discount_cents) FROM line_items WHERE invoice_id = $1), 0),
            total_cents    = COALESCE((SELECT SUM(subtotal_cents + tax_cents) FROM line_items WHERE invoice_id = $1), 0),
            updated_at     = NOW()
        WHERE id = $1
        "#,
        invoice_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}
