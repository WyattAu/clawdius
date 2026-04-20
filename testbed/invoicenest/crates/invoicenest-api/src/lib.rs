//! InvoiceNest API — HTTP layer built on Axum.
//!
//! This crate wires up route handlers, middleware (auth, CORS, tracing,
//! rate limiting), and serves the RESTful JSON API over HTTP.

use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// Builds the application router with all routes and middleware attached.
pub fn create_app() -> Router {
    Router::new()
        // ── Health ──
        .route("/health", axum::routing::get(health_handler))
        // ── API v1 routes ──
        .nest(
            "/api/v1",
            api_v1_routes(),
        )
        // ── Global middleware ──
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
}

fn api_v1_routes() -> Router {
    Router::new()
        // ── Auth ──
        .route("/auth/register", axum::routing::post(register_handler))
        .route("/auth/login", axum::routing::post(login_handler))
        .route("/auth/refresh", axum::routing::post(refresh_handler))
        // ── Workspaces ──
        .route("/workspaces", axum::routing::get(list_workspaces_handler))
        .route("/workspaces", axum::routing::post(create_workspace_handler))
        .route("/workspaces/:id", axum::routing::get(get_workspace_handler))
        .route("/workspaces/:id", axum::routing::patch(update_workspace_handler))
        // ── Clients ──
        .route("/clients", axum::routing::get(list_clients_handler))
        .route("/clients", axum::routing::post(create_client_handler))
        .route("/clients/:id", axum::routing::get(get_client_handler))
        .route("/clients/:id", axum::routing::patch(update_client_handler))
        .route("/clients/:id", axum::routing::delete(delete_client_handler))
        .route("/clients/search", axum::routing::get(search_clients_handler))
        // ── Invoices ──
        .route("/invoices", axum::routing::get(list_invoices_handler))
        .route("/invoices", axum::routing::post(create_invoice_handler))
        .route("/invoices/:id", axum::routing::get(get_invoice_handler))
        .route("/invoices/:id", axum::routing::patch(update_invoice_handler))
        .route("/invoices/:id/status", axum::routing::patch(transition_invoice_status_handler))
        .route("/invoices/:id/line-items", axum::routing::post(add_line_item_handler))
        .route("/invoices/:id/line-items/:line_id", axum::routing::delete(delete_line_item_handler))
        // ── Payments ──
        .route("/payments", axum::routing::get(list_payments_handler))
        .route("/payments", axum::routing::post(create_payment_handler))
        .route("/payments/:id", axum::routing::get(get_payment_handler))
        .route("/payments/:id/refund", axum::routing::post(refund_payment_handler))
        // ── Analytics ──
        .route("/analytics/revenue", axum::routing::get(revenue_summary_handler))
        .route("/analytics/clients", axum::routing::get(client_revenue_handler))
        .route("/analytics/monthly", axum::routing::get(monthly_revenue_handler))
        .route("/analytics/overdue", axum::routing::get(overdue_invoices_handler))
        // ── Tax Rates ──
        .route("/tax-rates", axum::routing::get(list_tax_rates_handler))
        .route("/tax-rates", axum::routing::post(create_tax_rate_handler))
        // ── Workspace Settings ──
        .route("/settings", axum::routing::get(get_settings_handler))
        .route("/settings", axum::routing::patch(update_settings_handler))
}

// ── Handler stubs ─────────────────────────────────────────────────────────
// Each handler returns a placeholder 501. Real implementations delegate
// to the core crate services behind auth middleware.

async fn health_handler() -> &'static str {
    "ok"
}

async fn register_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn login_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn refresh_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn list_workspaces_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn create_workspace_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn get_workspace_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn update_workspace_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn list_clients_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn create_client_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn get_client_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn update_client_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn delete_client_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn search_clients_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn list_invoices_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn create_invoice_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn get_invoice_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn update_invoice_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn transition_invoice_status_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn add_line_item_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn delete_line_item_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn list_payments_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn create_payment_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn get_payment_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn refund_payment_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn revenue_summary_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn client_revenue_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn monthly_revenue_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn overdue_invoices_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn list_tax_rates_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn create_tax_rate_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn get_settings_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}

async fn update_settings_handler() -> axum::response::StatusCode {
    axum::response::StatusCode::NOT_IMPLEMENTED
}
