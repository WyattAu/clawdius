//! API key authentication middleware for Axum

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;

#[derive(Clone)]
pub struct ApiKeyAuth {
    valid_keys: HashMap<String, String>,
    enabled: bool,
}

impl ApiKeyAuth {
    pub fn new(keys: HashMap<String, String>) -> Self {
        let enabled = !keys.is_empty();
        Self {
            valid_keys: keys,
            enabled,
        }
    }

    pub fn from_config(api_keys: Option<HashMap<String, String>>) -> Self {
        match api_keys {
            Some(keys) => Self::new(keys),
            None => Self {
                valid_keys: HashMap::new(),
                enabled: false,
            },
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

const SKIP_PATHS: &[&str] = &["/api/v1/health", "/api/v1/ready"];

fn should_skip(path: &str) -> bool {
    SKIP_PATHS.iter().any(|skip| path == *skip)
}

fn extract_bearer_token(headers: &header::HeaderMap) -> Option<String> {
    let auth = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let prefix = "Bearer ";
    if auth.starts_with(prefix) {
        Some(auth[prefix.len()..].to_string())
    } else {
        None
    }
}

pub async fn auth_middleware(
    axum::extract::State(auth): axum::extract::State<ApiKeyAuth>,
    request: Request,
    next: Next,
) -> Response {
    if !auth.enabled || should_skip(request.uri().path()) {
        return next.run(request).await;
    }

    match extract_bearer_token(request.headers()) {
        None => unauthorized_response("Missing authorization header"),
        Some(token) => {
            if auth.valid_keys.values().any(|v| v == &token) {
                next.run(request).await
            } else {
                forbidden_response("Invalid API key")
            }
        },
    }
}

fn unauthorized_response(message: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Bearer")],
        axum::Json(serde_json::json!({
            "error": "unauthorized",
            "message": message,
        })),
    )
        .into_response()
}

fn forbidden_response(message: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        axum::Json(serde_json::json!({
            "error": "forbidden",
            "message": message,
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request as HttpRequest, StatusCode},
        middleware,
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    fn make_keys() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("test-key".to_string(), "secret123".to_string());
        m
    }

    async fn ok_handler() -> &'static str {
        "ok"
    }

    fn auth_router(keys: Option<HashMap<String, String>>) -> Router {
        let auth = ApiKeyAuth::from_config(keys);
        let app = Router::new()
            .route("/api/v1/health", get(|| async { "healthy" }))
            .route("/api/v1/ready", get(|| async { "ready" }))
            .route("/api/v1/test", get(ok_handler))
            .with_state(auth.clone());
        if auth.is_enabled() {
            app.layer(middleware::from_fn_with_state(auth, auth_middleware))
        } else {
            app
        }
    }

    fn make_request(path: &str, bearer: Option<&str>) -> HttpRequest<Body> {
        let mut builder = HttpRequest::builder().method("GET").uri(path);
        if let Some(token) = bearer {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        builder.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn valid_key_accepted() {
        let app = auth_router(Some(make_keys()));
        let req = make_request("/api/v1/test", Some("secret123"));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn invalid_key_rejected() {
        let app = auth_router(Some(make_keys()));
        let req = make_request("/api/v1/test", Some("wrong-key"));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn missing_key_rejected() {
        let app = auth_router(Some(make_keys()));
        let req = make_request("/api/v1/test", None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn health_accessible_without_auth() {
        let app = auth_router(Some(make_keys()));
        let req = make_request("/api/v1/health", None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
