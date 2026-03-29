//! API Rate Limiter
//!
//! Lightweight per-key token-bucket rate limiter for the tenant management
//! and admin API endpoints. Each authenticated API key (or client IP as
//! fallback) gets its own bucket.
//!
//! Implemented as a `tower::Layer` so it composes naturally with the axum
//! router. Returns `429 Too Many Requests` with `Retry-After` header when
//! the bucket is exhausted.

#![deny(unsafe_code)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use tower::{Layer, Service};

/// Configuration for the API rate limiter.
#[derive(Debug, Clone)]
pub struct ApiRateLimitConfig {
    /// Maximum requests per minute per key (default: 60).
    pub requests_per_minute: u32,
    /// Burst capacity — maximum tokens in the bucket (default: 10).
    pub burst_capacity: u32,
}

impl Default for ApiRateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 60,
            burst_capacity: 10,
        }
    }
}

impl ApiRateLimitConfig {
    #[must_use]
    pub fn new(requests_per_minute: u32, burst_capacity: u32) -> Self {
        Self {
            requests_per_minute,
            burst_capacity,
        }
    }
}

// ---------------------------------------------------------------------------
// Token bucket
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Bucket {
    tokens: f64,
    max_tokens: f64,
    refill_per_sec: f64,
    last_refill: Instant,
}

impl Bucket {
    fn new(config: &ApiRateLimitConfig) -> Self {
        Self {
            tokens: f64::from(config.burst_capacity),
            max_tokens: f64::from(config.burst_capacity),
            refill_per_sec: f64::from(config.requests_per_minute) / 60.0,
            last_refill: Instant::now(),
        }
    }

    /// Refill tokens based on elapsed time, then try to consume one.
    /// Returns `Ok(())` if allowed, `Err(secs_until_available)` if limited.
    fn try_consume(&mut self) -> Result<(), f64> {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_per_sec).min(self.max_tokens);
        self.last_refill = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Ok(())
        } else {
            let needed = 1.0 - self.tokens;
            let secs = if self.refill_per_sec > 0.0 {
                needed / self.refill_per_sec
            } else {
                60.0
            };
            Err(secs.ceil())
        }
    }
}

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Inner {
    buckets: Arc<std::sync::Mutex<HashMap<String, Bucket>>>,
    config: ApiRateLimitConfig,
}

// ---------------------------------------------------------------------------
// Layer + Service
// ---------------------------------------------------------------------------

/// Axum/tower layer that rate-limits requests by API key or client IP.
#[derive(Debug, Clone)]
pub struct ApiRateLimitLayer {
    inner: Arc<Inner>,
}

impl ApiRateLimitLayer {
    #[must_use]
    pub fn new(config: ApiRateLimitConfig) -> Self {
        Self {
            inner: Arc::new(Inner {
                buckets: Arc::new(std::sync::Mutex::new(HashMap::new())),
                config,
            }),
        }
    }
}

impl<S> Layer<S> for ApiRateLimitLayer {
    type Service = ApiRateLimitMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ApiRateLimitMiddleware {
            inner,
            state: self.inner.clone(),
        }
    }
}

/// Middleware service that checks the rate limit before forwarding.
#[derive(Debug, Clone)]
pub struct ApiRateLimitMiddleware<S> {
    inner: S,
    state: Arc<Inner>,
}

impl<S> Service<Request<Body>> for ApiRateLimitMiddleware<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        // Extract a key for rate limiting: prefer API key hash, fall back to client IP.
        let key = extract_rate_limit_key(req.headers());
        let state = self.state.clone();

        let mut inner = self.inner.clone();
        Box::pin(async move {
            let allowed = {
                let mut buckets = state.buckets.lock().unwrap_or_else(|e| e.into_inner());
                let bucket = buckets
                    .entry(key.clone())
                    .or_insert_with(|| Bucket::new(&state.config));
                bucket.try_consume()
            };

            match allowed {
                Ok(()) => inner.call(req).await,
                Err(retry_after_secs) => {
                    let retry_header = retry_after_secs.to_string();
                    let response = (
                        StatusCode::TOO_MANY_REQUESTS,
                        [(axum::http::header::RETRY_AFTER, retry_header.as_str())],
                        axum::Json(serde_json::json!({
                            "status": "error",
                            "error": {
                                "code": "RATE_LIMITED",
                                "message": format!(
                                    "Too many requests. Retry after {retry_after_secs}s."
                                ),
                            }
                        })),
                    )
                        .into_response();
                    Ok(response)
                }
            }
        })
    }
}

/// Extract a rate-limit key from request headers.
///
/// Priority:
/// 1. Hash of the Bearer token (identifies API key / JWT subject)
/// 2. `X-Forwarded-For` first IP (reverse-proxy setups)
/// 3. `X-Real-IP`
/// 4. Fallback: `"unknown"`
fn extract_rate_limit_key(headers: &HeaderMap) -> String {
    // Try Bearer token hash first
    if let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                if !token.is_empty() {
                    // Simple hash for bucket key — doesn't need to be cryptographic
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};
                    let mut hasher = DefaultHasher::new();
                    token.hash(&mut hasher);
                    return format!("key:{}", hasher.finish());
                }
            }
        }
    }

    // Fall back to IP
    if let Some(ip) = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        return format!("ip:{ip}");
    }

    if let Some(ip) = headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        return format!("ip:{ip}");
    }

    "key:unknown".to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt; // for oneshot()

    fn default_layer() -> ApiRateLimitLayer {
        ApiRateLimitLayer::new(ApiRateLimitConfig {
            requests_per_minute: 60,
            burst_capacity: 3,
        })
    }

    fn make_request(auth_token: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder().method("GET").uri("/api/v1/tenants");
        if let Some(token) = auth_token {
            builder = builder.header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"));
        }
        builder.body(Body::empty()).unwrap()
    }

    // A dummy inner service that always returns 200 OK
    #[derive(Clone)]
    struct OkService;

    impl Service<Request<Body>> for OkService {
        type Response = Response;
        type Error = std::convert::Infallible;
        type Future = std::future::Ready<Result<Response, Self::Error>>;

        fn poll_ready(
            &mut self,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn call(&mut self, _req: Request<Body>) -> Self::Future {
            std::future::ready(Ok(Response::new(Body::empty())))
        }
    }

    #[tokio::test]
    async fn allows_requests_within_burst() {
        let svc = default_layer().layer(OkService);
        for _ in 0..3 {
            let resp = svc
                .clone()
                .oneshot(make_request(Some("test-key")))
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }
    }

    #[tokio::test]
    async fn returns_429_after_burst_exhausted() {
        let svc = default_layer().layer(OkService);
        // Exhaust burst (capacity = 3)
        for _ in 0..3 {
            let _ = svc
                .clone()
                .oneshot(make_request(Some("limited-key")))
                .await
                .unwrap();
        }
        // 4th request should be rate limited
        let resp = svc
            .clone()
            .oneshot(make_request(Some("limited-key")))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn separate_keys_have_separate_buckets() {
        let svc = default_layer().layer(OkService);
        // Exhaust bucket for key-a
        for _ in 0..3 {
            let _ = svc
                .clone()
                .oneshot(make_request(Some("key-a")))
                .await
                .unwrap();
        }
        // key-a should be limited
        let resp = svc
            .clone()
            .oneshot(make_request(Some("key-a")))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        // key-b should still work
        let resp = svc
            .clone()
            .oneshot(make_request(Some("key-b")))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rate_limited_response_includes_retry_after_header() {
        let svc = default_layer().layer(OkService);
        for _ in 0..3 {
            let _ = svc
                .clone()
                .oneshot(make_request(Some("retry-test")))
                .await
                .unwrap();
        }
        let resp = svc
            .clone()
            .oneshot(make_request(Some("retry-test")))
            .await
            .unwrap();
        assert!(resp.headers().contains_key(axum::http::header::RETRY_AFTER));
    }

    #[test]
    fn extract_key_from_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer my-secret-token"),
        );
        let key = extract_rate_limit_key(&headers);
        assert!(key.starts_with("key:"));
        assert!(!key.contains("my-secret-token")); // should be hashed, not plaintext
    }

    #[test]
    fn extract_key_from_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.50, 70.41.3.18"),
        );
        let key = extract_rate_limit_key(&headers);
        assert_eq!(key, "ip:203.0.113.50");
    }

    #[test]
    fn extract_key_from_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", HeaderValue::from_static("10.0.0.1"));
        let key = extract_rate_limit_key(&headers);
        assert_eq!(key, "ip:10.0.0.1");
    }

    #[test]
    fn extract_key_fallback() {
        let headers = HeaderMap::new();
        let key = extract_rate_limit_key(&headers);
        assert_eq!(key, "key:unknown");
    }

    #[test]
    fn bucket_refills_over_time() {
        let config = ApiRateLimitConfig {
            requests_per_minute: 600, // 10 per second
            burst_capacity: 2,
        };
        let mut bucket = Bucket::new(&config);

        // Exhaust
        assert!(bucket.try_consume().is_ok());
        assert!(bucket.try_consume().is_ok());
        assert!(bucket.try_consume().is_err());

        // Simulate time passing (1.5 seconds = 15 tokens at 10/sec)
        bucket.last_refill = Instant::now() - std::time::Duration::from_secs_f64(1.5);
        assert!(bucket.try_consume().is_ok());
    }
}
