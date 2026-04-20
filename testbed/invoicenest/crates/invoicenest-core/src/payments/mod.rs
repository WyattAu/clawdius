//! Payment processing and tracking.
//!
//! Handles recording payments against invoices, processing refunds,
//! and updating invoice status based on cumulative payment amounts.

use uuid::Uuid;

use crate::db::schema::{InvoiceStatus, Payment, PaymentMethod, PaymentStatus};
use crate::db::PgPool;
use crate::AppError;

/// Parameters for recording a new payment.
#[derive(Debug, Clone)]
pub struct CreatePayment {
    pub invoice_id: Uuid,
    pub amount_cents: i64,
    pub method: PaymentMethod,
    pub description: Option<String>,
    pub stripe_payment_intent_id: Option<String>,
    pub stripe_charge_id: Option<String>,
}

/// Parameters for recording a refund.
#[derive(Debug, Clone)]
pub struct RefundPayment {
    pub refund_id: String,
    pub refunded_amount_cents: i64,
}

/// Records a new payment and updates the invoice status.
///
/// After recording, the invoice's `paid_cents` is incremented and the
/// status machine is advanced:
/// - If fully paid → `paid`
/// - If partially paid → `partial`
pub async fn record_payment(
    pool: &PgPool,
    workspace_id: Uuid,
    input: CreatePayment,
) -> Result<Payment, AppError> {
    let mut tx = pool.begin().await?;

    let row = sqlx::query_as!(
        Payment,
        r#"
        INSERT INTO payments (workspace_id, invoice_id, amount_cents, method,
                              status, description, stripe_payment_intent_id,
                              stripe_charge_id)
        VALUES ($1, $2, $3, $4, 'succeeded', $5, $6, $7)
        RETURNING *
        "#,
        workspace_id,
        input.invoice_id,
        input.amount_cents,
        input.method as PaymentMethod,
        input.description,
        input.stripe_payment_intent_id,
        input.stripe_charge_id,
    )
    .fetch_one(&mut *tx)
    .await?;

    // Increment paid amount on the invoice
    sqlx::query!(
        r#"
        UPDATE invoices SET
            paid_cents = paid_cents + $3,
            updated_at = NOW()
        WHERE id = $1 AND workspace_id = $2
        "#,
        input.invoice_id,
        workspace_id,
        input.amount_cents,
    )
    .execute(&mut *tx)
    .await?;

    // Determine new invoice status based on payment progress
    let invoice = sqlx::query_as!(
        crate::db::schema::Invoice,
        r#"SELECT * FROM invoices WHERE id = $1 AND workspace_id = $2"#,
        input.invoice_id,
        workspace_id,
    )
    .fetch_one(&mut *tx)
    .await?;

    let new_status = if invoice.paid_cents >= invoice.total_cents {
        InvoiceStatus::Paid
    } else if invoice.paid_cents > 0 {
        InvoiceStatus::Partial
    } else {
        invoice.status
    };

    sqlx::query!(
        r#"
        UPDATE invoices SET status = $3, paid_at = CASE WHEN $3 = 'paid' THEN NOW() ELSE paid_at END
        WHERE id = $1 AND workspace_id = $2
        "#,
        input.invoice_id,
        workspace_id,
        new_status as InvoiceStatus,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(row)
}

/// Records a refund against an existing payment and adjusts the invoice.
pub async fn refund_payment(
    pool: &PgPool,
    workspace_id: Uuid,
    payment_id: Uuid,
    input: RefundPayment,
) -> Result<Payment, AppError> {
    let mut tx = pool.begin().await?;

    let existing = sqlx::query_as!(
        Payment,
        r#"SELECT * FROM payments WHERE id = $1 AND workspace_id = $2"#,
        payment_id,
        workspace_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Payment {payment_id} not found")))?;

    if existing.status != PaymentStatus::Succeeded {
        return Err(AppError::Validation("Only succeeded payments can be refunded".into()));
    }

    if input.refunded_amount_cents > existing.amount_cents {
        return Err(AppError::Validation("Refund amount exceeds payment amount".into()));
    }

    let updated = sqlx::query_as!(
        Payment,
        r#"
        UPDATE payments SET
            status = 'refunded',
            refund_id = $3,
            refunded_amount_cents = $4,
            updated_at = NOW()
        WHERE id = $1 AND workspace_id = $2
        RETURNING *
        "#,
        payment_id,
        workspace_id,
        input.refund_id,
        input.refunded_amount_cents,
    )
    .fetch_one(&mut *tx)
    .await?;

    // Decrease paid amount on the invoice
    sqlx::query!(
        r#"
        UPDATE invoices SET
            paid_cents = GREATEST(0, paid_cents - $3),
            updated_at = NOW()
        WHERE id = $1 AND workspace_id = $2
        "#,
        existing.invoice_id,
        workspace_id,
        input.refunded_amount_cents,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(updated)
}

/// Lists all payments for an invoice.
pub async fn list_payments(
    pool: &PgPool,
    workspace_id: Uuid,
    invoice_id: Uuid,
) -> Result<Vec<Payment>, AppError> {
    let rows = sqlx::query_as!(
        Payment,
        r#"
        SELECT * FROM payments
        WHERE workspace_id = $1 AND invoice_id = $2
        ORDER BY created_at DESC
        "#,
        workspace_id,
        invoice_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Retrieves a single payment by ID.
pub async fn get_payment(
    pool: &PgPool,
    workspace_id: Uuid,
    payment_id: Uuid,
) -> Result<Payment, AppError> {
    let row = sqlx::query_as!(
        Payment,
        r#"
        SELECT * FROM payments
        WHERE id = $1 AND workspace_id = $2
        "#,
        payment_id,
        workspace_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Payment {payment_id} not found")))?;

    Ok(row)
}
