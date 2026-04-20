//! InvoiceNest Core — Business logic, database, and domain types.
//!
//! This crate contains the domain models, database schema (via SQLx),
//! authentication, and business logic for the InvoiceNest SaaS platform.

pub mod analytics;
pub mod api;
pub mod auth;
pub mod clients;
pub mod db;
pub mod invoices;
pub mod payments;

// Re-exports
pub use db::schema;

/// Application error type.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Payment error: {0}")]
    Payment(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl axum::response::IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_code, message) = match &self {
            AppError::NotFound(msg) => (axum::http::StatusCode::NOT_FOUND, "NOT_FOUND", msg.clone()),
            AppError::Unauthorized(msg) => (axum::http::StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg.clone()),
            AppError::Validation(msg) => (axum::http::StatusCode::BAD_REQUEST, "VALIDATION_ERROR", msg.clone()),
            AppError::Database(_) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR", "Database operation failed".to_string()),
            AppError::Payment(msg) => (axum::http::StatusCode::PAYMENT_REQUIRED, "PAYMENT_ERROR", msg.clone()),
            AppError::Internal(msg) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", msg.clone()),
        };

        let body = serde_json::json!({
            "error": error_code,
            "message": message,
            "request_id": uuid::Uuid::new_v4().to_string(),
        });

        (status, axum::Json(body)).into_response()
    }
}
