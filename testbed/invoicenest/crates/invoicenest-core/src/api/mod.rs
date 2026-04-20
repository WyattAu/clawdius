//! API layer types, middleware, and request/response helpers.
//!
//! This module provides:
//! - A typed [`ApiError`] that maps to HTTP status codes
//! - Extraction of [`AuthContext`] from JWT via middleware
//! - Pagination and filtering parameter types
//! - Standard JSON envelope wrappers

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Authenticated context extracted from a valid JWT.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub role: String,
}

/// Typed API error with an associated HTTP status code.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Rate limited")]
    RateLimited,

    #[error("Internal error: {0}")]
    Internal(String),
}

impl ApiError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden(_) => StatusCode::FORBIDDEN,
            ApiError::Validation(_) => StatusCode::BAD_REQUEST,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn error_code(&self) -> &str {
        match self {
            ApiError::NotFound(_) => "NOT_FOUND",
            ApiError::Unauthorized => "UNAUTHORIZED",
            ApiError::Forbidden(_) => "FORBIDDEN",
            ApiError::Validation(_) => "VALIDATION_ERROR",
            ApiError::Conflict(_) => "CONFLICT",
            ApiError::RateLimited => "RATE_LIMITED",
            ApiError::Internal(_) => "INTERNAL_ERROR",
        }
    }
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = serde_json::json!({
            "error": self.error_code(),
            "message": self.to_string(),
        });

        (self.status_code(), axum::Json(body)).into_response()
    }
}

/// Standard paginated list response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, total: i64, page: i64, per_page: i64) -> Self {
        Self {
            data,
            total,
            page,
            per_page,
        }
    }
}

/// Query parameters for pagination.
#[derive(Debug, Clone, Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

fn default_page() -> i64 {
    1
}

fn default_per_page() -> i64 {
    25
}

impl PaginationParams {
    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.per_page
    }

    pub fn limit(&self) -> i64 {
        self.per_page.min(100)
    }
}

/// Standard single-item API response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

impl<T> ApiResponse<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}
