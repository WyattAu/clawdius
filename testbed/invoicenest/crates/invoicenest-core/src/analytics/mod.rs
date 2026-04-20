//! Analytics and reporting queries.
//!
//! Provides aggregated queries for revenue, outstanding balances,
//! client summaries, and growth metrics — all scoped to a workspace.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::PgPool;
use crate::AppError;

/// Summary of a workspace's financial health.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueSummary {
    pub workspace_id: Uuid,
    pub total_revenue_cents: i64,
    pub outstanding_cents: i64,
    pub overdue_cents: i64,
    pub paid_this_month_cents: i64,
    pub paid_last_month_cents: i64,
    pub invoice_count: i64,
    pub client_count: i64,
}

/// Per-client revenue breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRevenue {
    pub client_id: Uuid,
    pub client_name: String,
    pub total_invoiced_cents: i64,
    pub total_paid_cents: i64,
    pub outstanding_cents: i64,
    pub invoice_count: i64,
}

/// Monthly revenue data point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyRevenue {
    pub month: String,
    pub invoiced_cents: i64,
    pub paid_cents: i64,
    pub overdue_cents: i64,
}

/// Top overdue invoices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverdueInvoice {
    pub invoice_id: Uuid,
    pub number: String,
    pub client_name: String,
    pub total_cents: i64,
    pub paid_cents: i64,
    pub outstanding_cents: i64,
    pub due_date: chrono::NaiveDate,
    pub days_overdue: i64,
}

/// Fetches a high-level revenue summary for the workspace.
pub async fn get_revenue_summary(
    pool: &PgPool,
    workspace_id: Uuid,
) -> Result<RevenueSummary, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT
            COALESCE(SUM(CASE WHEN i.status IN ('paid','partial') THEN i.paid_cents ELSE 0 END), 0) AS "total_revenue!",
            COALESCE(SUM(CASE WHEN i.status IN ('sent','viewed','partial','overdue') THEN (i.total_cents - i.paid_cents) ELSE 0 END), 0) AS "outstanding!",
            COALESCE(SUM(CASE WHEN i.status = 'overdue' THEN (i.total_cents - i.paid_cents) ELSE 0 END), 0) AS "overdue!",
            COUNT(DISTINCT i.id) AS "invoice_count!",
            COUNT(DISTINCT c.id) AS "client_count!"
        FROM invoices i
        LEFT JOIN clients c ON c.workspace_id = i.workspace_id
        WHERE i.workspace_id = $1 AND i.deleted_at IS NULL
        "#,
        workspace_id,
    )
    .fetch_one(pool)
    .await?;

    let paid_this_month = sqlx::query_scalar!(
        r#"
        SELECT COALESCE(SUM(p.amount_cents), 0)
        FROM payments p
        WHERE p.workspace_id = $1
          AND p.status = 'succeeded'
          AND p.created_at >= date_trunc('month', NOW())
        "#,
        workspace_id,
    )
    .fetch_one(pool)
    .await?;

    let paid_last_month = sqlx::query_scalar!(
        r#"
        SELECT COALESCE(SUM(p.amount_cents), 0)
        FROM payments p
        WHERE p.workspace_id = $1
          AND p.status = 'succeeded'
          AND p.created_at >= date_trunc('month', NOW() - INTERVAL '1 month')
          AND p.created_at < date_trunc('month', NOW())
        "#,
        workspace_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(RevenueSummary {
        workspace_id,
        total_revenue_cents: row.total_revenue,
        outstanding_cents: row.outstanding,
        overdue_cents: row.overdue,
        paid_this_month_cents: paid_this_month,
        paid_last_month_cents: paid_last_month,
        invoice_count: row.invoice_count,
        client_count: row.client_count,
    })
}

/// Revenue broken down by client.
pub async fn get_client_revenue(
    pool: &PgPool,
    workspace_id: Uuid,
    limit: i64,
) -> Result<Vec<ClientRevenue>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            c.id AS "client_id!",
            c.name AS "client_name!",
            COALESCE(SUM(i.total_cents), 0) AS "total_invoiced!",
            COALESCE(SUM(i.paid_cents), 0) AS "total_paid!",
            COALESCE(SUM(i.total_cents - i.paid_cents), 0) AS "outstanding!",
            COUNT(i.id) AS "invoice_count!"
        FROM clients c
        LEFT JOIN invoices i ON i.client_id = c.id AND i.deleted_at IS NULL
        WHERE c.workspace_id = $1 AND c.deleted_at IS NULL
        GROUP BY c.id, c.name
        ORDER BY total_invoiced DESC
        LIMIT $2
        "#,
        workspace_id,
        limit,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ClientRevenue {
            client_id: r.client_id,
            client_name: r.client_name,
            total_invoiced_cents: r.total_invoiced,
            total_paid_cents: r.total_paid,
            outstanding_cents: r.outstanding,
            invoice_count: r.invoice_count,
        })
        .collect())
}

/// Monthly revenue trend for the last N months.
pub async fn get_monthly_revenue(
    pool: &PgPool,
    workspace_id: Uuid,
    months: i32,
) -> Result<Vec<MonthlyRevenue>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            TO_CHAR(date_trunc('month', i.created_at), 'YYYY-MM') AS "month!",
            COALESCE(SUM(i.total_cents), 0) AS "invoiced!",
            COALESCE(SUM(i.paid_cents), 0) AS "paid!",
            COALESCE(SUM(CASE WHEN i.status = 'overdue' THEN (i.total_cents - i.paid_cents) ELSE 0 END), 0) AS "overdue!"
        FROM invoices i
        WHERE i.workspace_id = $1
          AND i.deleted_at IS NULL
          AND i.created_at >= date_trunc('month', NOW()) - ($2 || ' months')::INTERVAL
        GROUP BY date_trunc('month', i.created_at)
        ORDER BY month
        "#,
        workspace_id,
        months.to_string(),
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| MonthlyRevenue {
            month: r.month,
            invoiced_cents: r.invoiced,
            paid_cents: r.paid,
            overdue_cents: r.overdue,
        })
        .collect())
}

/// Lists invoices that are past their due date.
pub async fn get_overdue_invoices(
    pool: &PgPool,
    workspace_id: Uuid,
    limit: i64,
) -> Result<Vec<OverdueInvoice>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            i.id AS "invoice_id!",
            i.number AS "number!",
            c.name AS "client_name!",
            i.total_cents AS "total_cents!",
            i.paid_cents AS "paid_cents!",
            (i.total_cents - i.paid_cents) AS "outstanding_cents!",
            i.due_date AS "due_date!",
            EXTRACT(DAY FROM NOW() - i.due_date)::BIGINT AS "days_overdue!"
        FROM invoices i
        JOIN clients c ON c.id = i.client_id
        WHERE i.workspace_id = $1
          AND i.status IN ('sent', 'viewed', 'overdue')
          AND i.due_date < CURRENT_DATE
          AND i.deleted_at IS NULL
        ORDER BY i.due_date ASC
        LIMIT $2
        "#,
        workspace_id,
        limit,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| OverdueInvoice {
            invoice_id: r.invoice_id,
            number: r.number,
            client_name: r.client_name,
            total_cents: r.total_cents,
            paid_cents: r.paid_cents,
            outstanding_cents: r.outstanding_cents,
            due_date: r.due_date.unwrap_or_default(),
            days_overdue: r.days_overdue,
        })
        .collect())
}
