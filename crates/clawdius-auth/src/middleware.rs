use crate::service::AuthService;
use crate::user::SessionClaims;
use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

/// Axum extractor that yields the claims of the authenticated user for a request.
pub struct AuthUser {
    /// Validated session claims extracted from the request's bearer token.
    pub claims: SessionClaims,
}

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_service = parts
            .extensions
            .get::<Arc<AuthService>>()
            .ok_or(AuthError::MissingAuthService)?;

        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingToken)?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(AuthError::InvalidTokenFormat)?;

        let claims = auth_service
            .validate_session(token)
            .map_err(|_| AuthError::InvalidToken)?;

        Ok(AuthUser { claims })
    }
}

/// Errors returned when authenticating an incoming request.
#[derive(Debug)]
pub enum AuthError {
    /// The auth service was not registered in request extensions.
    MissingAuthService,
    /// The Authorization header was absent from the request.
    MissingToken,
    /// The Authorization header was present but malformed.
    InvalidTokenFormat,
    /// The bearer token failed validation.
    InvalidToken,
    /// The bearer token has expired.
    ExpiredToken,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::MissingAuthService => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Auth service not configured",
            ),
            AuthError::MissingToken => (StatusCode::UNAUTHORIZED, "Missing authorization token"),
            AuthError::InvalidTokenFormat => (
                StatusCode::UNAUTHORIZED,
                "Invalid token format (expected Bearer)",
            ),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid or expired token"),
            AuthError::ExpiredToken => (StatusCode::UNAUTHORIZED, "Token expired"),
        };

        (status, message).into_response()
    }
}
