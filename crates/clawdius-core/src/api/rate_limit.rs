use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use tokio::sync::Mutex;

use crate::api::gateway::RateLimitConfig;

const SKIP_PATHS: &[&str] = &["/api/v1/health", "/api/v1/ready", "/metrics"];

struct TokenBucket {
    tokens: f64,
    last_update: Instant,
}

impl TokenBucket {
    fn new(burst: u32) -> Self {
        Self {
            tokens: f64::from(burst),
            last_update: Instant::now(),
        }
    }
}

#[derive(Clone)]
pub struct ApiRateLimiter {
    config: RateLimitConfig,
    buckets: Arc<Mutex<HashMap<String, TokenBucket>>>,
}

impl ApiRateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn try_acquire(&self, key: &str) -> Result<(), u64> {
        let mut buckets = self.buckets.lock().await;
        let bucket = buckets
            .entry(key.to_string())
            .or_insert_with(|| TokenBucket::new(self.config.burst));

        let now = Instant::now();
        let elapsed = now.duration_since(bucket.last_update).as_secs_f64();
        let tokens_per_second = f64::from(self.config.requests_per_minute) / 60.0;
        bucket.tokens =
            (bucket.tokens + elapsed * tokens_per_second).min(f64::from(self.config.burst));
        bucket.last_update = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(())
        } else {
            let deficit = 1.0 - bucket.tokens;
            let ms_per_token = 60_000.0 / f64::from(self.config.requests_per_minute);
            Err((deficit * ms_per_token).ceil() as u64)
        }
    }
}

fn extract_client_identity(headers: &header::HeaderMap) -> String {
    if let Some(auth) = headers.get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return format!(
                    "key:{}",
                    blake3::hash(token.as_bytes()).to_hex().as_str()[..16].to_string()
                );
            }
        }
    }
    "anonymous".to_string()
}

fn should_skip(path: &str) -> bool {
    SKIP_PATHS.iter().any(|skip| path == *skip)
}

pub async fn rate_limit_middleware(
    axum::extract::State(limiter): axum::extract::State<ApiRateLimiter>,
    request: Request,
    next: Next,
) -> Response {
    if should_skip(request.uri().path()) {
        return next.run(request).await;
    }

    let identity = extract_client_identity(request.headers());

    match limiter.try_acquire(&identity).await {
        Ok(()) => next.run(request).await,
        Err(retry_after) => (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, retry_after.to_string())],
            axum::Json(serde_json::json!({
                "error": "rate_limited",
                "message": "Too many requests. Please retry later.",
                "retry_after_seconds": retry_after,
            })),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request as HttpRequest, middleware, routing::get, Router};
    use tower::ServiceExt;

    fn test_config(burst: u32) -> RateLimitConfig {
        RateLimitConfig {
            requests_per_minute: 60,
            burst,
        }
    }

    async fn ok_handler() -> &'static str {
        "ok"
    }

    fn app(burst: u32) -> Router {
        let limiter = ApiRateLimiter::new(test_config(burst));
        Router::new()
            .route("/api/v1/test", get(ok_handler))
            .route("/api/v1/health", get(ok_handler))
            .route("/metrics", get(ok_handler))
            .layer(middleware::from_fn_with_state(
                limiter,
                rate_limit_middleware,
            ))
    }

    fn make_request(path: &str, bearer: Option<&str>) -> HttpRequest<Body> {
        let mut builder = HttpRequest::builder().method("GET").uri(path);
        if let Some(token) = bearer {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        builder.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn requests_under_limit_succeed() {
        let app = app(5);
        for _ in 0..5 {
            let req = make_request("/api/v1/test", Some("test-key"));
            let resp = app.clone().oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }
    }

    #[tokio::test]
    async fn requests_over_limit_get_429() {
        let app = app(3);
        for _ in 0..3 {
            let req = make_request("/api/v1/test", Some("test-key"));
            let resp = app.clone().oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }
        let req = make_request("/api/v1/test", Some("test-key"));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn health_endpoint_bypasses_rate_limit() {
        let app = app(1);
        let req = make_request("/api/v1/health", None);
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let req = make_request("/api/v1/health", None);
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let req = make_request("/api/v1/health", None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
