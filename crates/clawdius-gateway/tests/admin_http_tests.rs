//! HTTP-level integration tests for admin API endpoints.
//!
//! Uses `tower::ServiceExt` to send requests through the axum router
//! without spawning a real server. Covers all 13 endpoints.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::items_after_statements
)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use clawdius_core::billing::BillingManager;
use clawdius_core::usage::TenantUsageTracker;
use clawdius_gateway::admin::{admin_router, AdminState};
use std::sync::Arc;
use tower::ServiceExt;

/// Build a test admin state with fresh billing and usage trackers.
fn test_admin_state() -> Arc<AdminState> {
    Arc::new(AdminState {
        billing: Arc::new(BillingManager::new()),
        usage: Arc::new(TenantUsageTracker::new()),
        api_key: "test-admin-key".to_string(),
    })
}

/// Helper: send a GET request and return (status, body text).
async fn get(app: &axum::Router, path: &str) -> (StatusCode, String) {
    let req = Request::builder().uri(path).body(Body::empty()).unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

/// Helper: send a POST request with JSON body and return (status, body text).
async fn post_json(app: &axum::Router, path: &str, json_body: &str) -> (StatusCode, String) {
    let req = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(json_body.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

/// Helper: send a PUT request with JSON body.
async fn put_json(app: &axum::Router, path: &str, json_body: &str) -> (StatusCode, String) {
    let req = Request::builder()
        .method("PUT")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(json_body.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

/// Helper: send a DELETE request.
async fn delete(app: &axum::Router, path: &str) -> (StatusCode, String) {
    let req = Request::builder()
        .method("DELETE")
        .uri(path)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

fn app() -> axum::Router {
    admin_router(test_admin_state())
}

// ═══════════════════════════════════════════════════════════════
// Health endpoint
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_health_check_returns_healthy() {
    let a = app();
    let (status, body) = get(&a, "/api/admin/health").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"healthy\""));
    assert!(body.contains("\"ok\":true"));
}

// ═══════════════════════════════════════════════════════════════
// System info endpoint
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_system_info_empty() {
    let a = app();
    let (status, body) = get(&a, "/api/admin/system/info").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["total_tenants"], 0);
    assert_eq!(v["data"]["active_subscriptions"], 0);
    assert_eq!(v["data"]["uptime_secs"], 0);
    assert_eq!(v["data"]["stripe_enabled"], false);
}

#[tokio::test]
async fn test_system_info_with_tenants() {
    let a = app();
    // Create two tenants first
    post_json(
        &a,
        "/api/admin/tenants",
        r#"{"tenant_id":"t1","tier":"pro"}"#,
    )
    .await;
    post_json(
        &a,
        "/api/admin/tenants",
        r#"{"tenant_id":"t2","tier":"free"}"#,
    )
    .await;

    let (status, body) = get(&a, "/api/admin/system/info").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["data"]["total_tenants"], 2);
    assert_eq!(v["data"]["active_subscriptions"], 2); // both active
}

// ═══════════════════════════════════════════════════════════════
// Create tenant
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_create_tenant_returns_201() {
    let a = app();
    let (status, body) = post_json(
        &a,
        "/api/admin/tenants",
        r#"{"tenant_id":"tenant-1","tier":"pro"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["tenant_id"], "tenant-1");
    assert_eq!(v["data"]["tier"], "Pro");
    assert_eq!(v["data"]["status"], "active");
}

#[tokio::test]
async fn test_create_tenant_default_free() {
    let a = app();
    let (status, body) = post_json(&a, "/api/admin/tenants", r#"{"tenant_id":"free-1"}"#).await;
    assert_eq!(status, StatusCode::CREATED);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["data"]["tier"], "Free");
}

#[tokio::test]
async fn test_create_tenant_with_quota() {
    let a = app();
    let (status, body) = post_json(
        &a,
        "/api/admin/tenants",
        r#"{"tenant_id":"q-1","tier":"team","quota":{"monthly_token_limit":500000,"per_request_token_limit":50000}}"#,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["data"]["tier"], "Team");
}

// ═══════════════════════════════════════════════════════════════
// List tenants
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_list_tenants_empty() {
    let a = app();
    let (status, body) = get(&a, "/api/admin/tenants").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["data"]["tenants"].as_array().unwrap().len(), 0);
    assert_eq!(v["data"]["total"], 0);
}

#[tokio::test]
async fn test_list_tenants_after_create() {
    let a = app();
    post_json(&a, "/api/admin/tenants", r#"{"tenant_id":"list-1"}"#).await;
    post_json(&a, "/api/admin/tenants", r#"{"tenant_id":"list-2"}"#).await;

    let (status, body) = get(&a, "/api/admin/tenants").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["data"]["total"], 2);
}

#[tokio::test]
async fn test_list_tenants_filter_by_tier() {
    let a = app();
    post_json(
        &a,
        "/api/admin/tenants",
        r#"{"tenant_id":"flt-1","tier":"pro"}"#,
    )
    .await;
    post_json(
        &a,
        "/api/admin/tenants",
        r#"{"tenant_id":"flt-2","tier":"free"}"#,
    )
    .await;
    post_json(
        &a,
        "/api/admin/tenants",
        r#"{"tenant_id":"flt-3","tier":"pro"}"#,
    )
    .await;

    let (status, body) = get(&a, "/api/admin/tenants?tier=pro").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["data"]["total"], 2);
}

#[tokio::test]
async fn test_list_tenants_pagination() {
    let a = app();
    for i in 0..5 {
        let id = format!("pag-{i}");
        let body = format!(r#"{{"tenant_id":"{id}"}}"#);
        post_json(&a, "/api/admin/tenants", &body).await;
    }

    let (status, body) = get(&a, "/api/admin/tenants?limit=2&offset=1").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let tenants = v["data"]["tenants"].as_array().unwrap();
    assert_eq!(tenants.len(), 2); // limit=2
    assert_eq!(v["data"]["total"], 5);
    assert_eq!(v["data"]["offset"], 1);
}

// ═══════════════════════════════════════════════════════════════
// Get tenant
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_get_tenant_found() {
    let a = app();
    post_json(
        &a,
        "/api/admin/tenants",
        r#"{"tenant_id":"get-1","tier":"team"}"#,
    )
    .await;

    let (status, body) = get(&a, "/api/admin/tenants/get-1").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["data"]["tenant_id"], "get-1");
    assert_eq!(v["data"]["tier"], "Team");
}

#[tokio::test]
async fn test_get_tenant_not_found() {
    let a = app();
    let (status, body) = get(&a, "/api/admin/tenants/nonexistent").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["ok"], false);
    assert!(body.contains("tenant not found"));
}

// ═══════════════════════════════════════════════════════════════
// Delete tenant
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_delete_tenant_success() {
    let a = app();
    post_json(&a, "/api/admin/tenants", r#"{"tenant_id":"del-1"}"#).await;

    let (status, body) = delete(&a, "/api/admin/tenants/del-1").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["data"]["deleted"], true);

    // Verify it's canceled now
    let (status2, body2) = get(&a, "/api/admin/tenants/del-1").await;
    assert_eq!(status2, StatusCode::OK);
    let v2: serde_json::Value = serde_json::from_str(&body2).unwrap();
    assert_eq!(v2["data"]["status"], "canceled");
}

#[tokio::test]
async fn test_delete_tenant_not_found() {
    let a = app();
    let (status, _body) = delete(&a, "/api/admin/tenants/ghost").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ═══════════════════════════════════════════════════════════════
// Usage
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_get_usage_tenant_not_found() {
    let a = app();
    let (status, body) = get(&a, "/api/admin/tenants/ghost/usage").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains("tenant not found"));
}

#[tokio::test]
async fn test_reset_usage() {
    let a = app();
    // Create a tenant with quota so usage tracking works
    post_json(
        &a,
        "/api/admin/tenants",
        r#"{"tenant_id":"reset-1","quota":{"monthly_token_limit":1000}}"#,
    )
    .await;

    let (status, body) = post_json(&a, "/api/admin/tenants/reset-1/usage/reset", "").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["data"]["reset"], true);
}

// ═══════════════════════════════════════════════════════════════
// Quota
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_get_quota_found() {
    let a = app();
    post_json(
        &a,
        "/api/admin/tenants",
        r#"{"tenant_id":"qf-1","tier":"enterprise"}"#,
    )
    .await;

    let (status, body) = get(&a, "/api/admin/tenants/qf-1/quota").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(v["data"]["monthly_token_limit"].as_u64().unwrap() > 0);
    assert_eq!(v["data"]["per_request_token_limit"], 100_000);
    assert_eq!(v["data"]["rate_limit_per_minute"], 60);
}

#[tokio::test]
async fn test_get_quota_not_found() {
    let a = app();
    let (status, _body) = get(&a, "/api/admin/tenants/ghost/quota").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_set_quota() {
    let a = app();
    let (status, body) = put_json(
        &a,
        "/api/admin/tenants/sq-1/quota",
        r#"{"monthly_token_limit":500000,"per_request_token_limit":25000,"monthly_cost_limit":50.0,"rate_limit_per_minute":30}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["data"]["quota_set"], true);
}

// ═══════════════════════════════════════════════════════════════
// Subscription
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_get_subscription_found() {
    let a = app();
    post_json(
        &a,
        "/api/admin/tenants",
        r#"{"tenant_id":"sub-1","tier":"pro"}"#,
    )
    .await;

    let (status, body) = get(&a, "/api/admin/tenants/sub-1/subscription").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["data"]["tenant_id"], "sub-1");
    assert_eq!(v["data"]["tier"], "Pro");
}

#[tokio::test]
async fn test_get_subscription_not_found() {
    let a = app();
    let (status, _body) = get(&a, "/api/admin/tenants/ghost/subscription").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_change_plan_success() {
    let a = app();
    post_json(
        &a,
        "/api/admin/tenants",
        r#"{"tenant_id":"cp-1","tier":"free"}"#,
    )
    .await;

    let (status, body) = put_json(
        &a,
        "/api/admin/tenants/cp-1/subscription/plan",
        r#"{"new_tier":"pro"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["data"]["tier"], "Pro");

    // Verify the change persisted
    let (s2, b2) = get(&a, "/api/admin/tenants/cp-1").await;
    assert_eq!(s2, StatusCode::OK);
    let v2: serde_json::Value = serde_json::from_str(&b2).unwrap();
    assert_eq!(v2["data"]["tier"], "Pro");
}

#[tokio::test]
async fn test_change_plan_invalid_tier() {
    let a = app();
    post_json(
        &a,
        "/api/admin/tenants",
        r#"{"tenant_id":"cp-2","tier":"free"}"#,
    )
    .await;

    let (status, body) = put_json(
        &a,
        "/api/admin/tenants/cp-2/subscription/plan",
        r#"{"new_tier":"invalid_tier"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("invalid tier"));
}

#[tokio::test]
async fn test_cancel_subscription_success() {
    let a = app();
    post_json(
        &a,
        "/api/admin/tenants",
        r#"{"tenant_id":"cs-1","tier":"team"}"#,
    )
    .await;

    let (status, body) = post_json(&a, "/api/admin/tenants/cs-1/subscription/cancel", "").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["data"]["canceled"], true);
}

#[tokio::test]
async fn test_cancel_subscription_not_found() {
    let a = app();
    let (status, _body) = post_json(&a, "/api/admin/tenants/ghost/subscription/cancel", "").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ═══════════════════════════════════════════════════════════════
// Edge cases
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_unknown_route_returns_404() {
    let a = app();
    let (status, _body) = get(&a, "/api/admin/nonexistent").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_create_duplicate_tenant_overwrites() {
    let a = app();
    // Create first
    let (s1, _b1) = post_json(
        &a,
        "/api/admin/tenants",
        r#"{"tenant_id":"dup-1","tier":"free"}"#,
    )
    .await;
    assert_eq!(s1, StatusCode::CREATED);

    // Create again with different tier -- should overwrite
    let (s2, b2) = post_json(
        &a,
        "/api/admin/tenants",
        r#"{"tenant_id":"dup-1","tier":"pro"}"#,
    )
    .await;
    assert_eq!(s2, StatusCode::CREATED);
    let v: serde_json::Value = serde_json::from_str(&b2).unwrap();
    assert_eq!(v["data"]["tier"], "Pro"); // Updated
}

#[tokio::test]
async fn test_all_tiers_create() {
    let a = app();
    // parse_tier matches lowercase; response uses Title Case
    let tiers = [
        ("free", "Free"),
        ("pro", "Pro"),
        ("team", "Team"),
        ("enterprise", "Enterprise"),
    ];
    for (input_tier, expected_tier) in &tiers {
        let body = format!(r#"{{"tenant_id":"tier-{input_tier}","tier":"{input_tier}"}}"#,);
        let (status, resp) = post_json(&a, "/api/admin/tenants", &body).await;
        assert_eq!(status, StatusCode::CREATED);
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(v["data"]["tier"], *expected_tier);
    }

    // Verify all in list
    let (_s, body) = get(&a, "/api/admin/tenants").await;
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["data"]["total"], 4);
}
