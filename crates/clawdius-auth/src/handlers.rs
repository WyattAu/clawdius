use crate::middleware::AuthUser;
use crate::service::AuthService;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Redirect},
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use std::sync::Arc;

pub fn auth_routes(auth_service: Arc<AuthService>) -> Router {
    Router::new()
        .route("/login/{provider}", get(login_handler))
        .route("/callback", get(callback_handler))
        .route("/logout", post(logout_handler))
        .route("/me", get(me_handler))
        .route("/refresh", post(refresh_handler))
        .with_state(auth_service)
}

async fn login_handler(
    Path(provider): Path<String>,
    State(service): State<Arc<AuthService>>,
) -> impl IntoResponse {
    match service.authorization_url(&provider) {
        Ok((url, _state, _verifier)) => Redirect::temporary(&url).into_response(),
        Err(e) => {
            tracing::error!("Failed to generate auth URL for {provider}: {e}");
            (
                StatusCode::BAD_REQUEST,
                format!("Auth provider '{provider}' not available"),
            )
                .into_response()
        },
    }
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: String,
    state: String,
}

async fn callback_handler(
    State(service): State<Arc<AuthService>>,
    Query(query): Query<CallbackQuery>,
) -> impl IntoResponse {
    match service
        .exchange_code("default", &query.code, &query.state)
        .await
    {
        Ok(result) => Json(serde_json::json!({
            "session_token": result.session_token,
            "refresh_token": result.refresh_token,
            "user": result.user,
        }))
        .into_response(),
        Err(e) => {
            tracing::error!("Auth callback failed: {e}");
            (StatusCode::UNAUTHORIZED, "Authentication failed").into_response()
        },
    }
}

async fn logout_handler() -> impl IntoResponse {
    Json(serde_json::json!({"status": "logged_out"}))
}

async fn me_handler(auth: AuthUser) -> impl IntoResponse {
    Json(serde_json::json!({
        "sub": auth.claims.sub,
        "email": auth.claims.email,
        "name": auth.claims.name,
        "provider": auth.claims.provider,
        "roles": auth.claims.roles,
    }))
}

async fn refresh_handler(
    State(service): State<Arc<AuthService>>,
    body: Json<serde_json::Value>,
) -> impl IntoResponse {
    let refresh_token = body["refresh_token"].as_str().unwrap_or("");

    match service.refresh_session(refresh_token) {
        Ok(new_token) => Json(serde_json::json!({
            "session_token": new_token,
        }))
        .into_response(),
        Err(e) => {
            tracing::error!("Token refresh failed: {e}");
            (StatusCode::UNAUTHORIZED, "Refresh failed").into_response()
        },
    }
}
