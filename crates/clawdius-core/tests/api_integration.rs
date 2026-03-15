//! Integration tests for the REST API
//!
//! Tests the actor-pattern REST API implementation for session management.

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use clawdius_core::api::rest::{ApiState, CreateSessionRequest};
use clawdius_core::session::SessionStore;
use tempfile::TempDir;
use tower::ServiceExt; // for oneshot

fn create_test_app() -> (axum::Router, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let store = SessionStore::open(&db_path).expect("Failed to create session store");
    let state = ApiState::new(store);
    let router = clawdius_core::api::rest::create_router(state);
    (router, temp_dir)
}

#[tokio::test]
async fn test_health_endpoint() {
    let (app, _temp_dir) = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_readiness_endpoint() {
    let (app, _temp_dir) = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_list_sessions_empty() {
    let (app, _temp_dir) = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/sessions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_create_session() {
    let (app, _temp_dir) = create_test_app();

    let body = serde_json::to_string(&CreateSessionRequest {
        name: Some("Test Session".to_string()),
        model: Some("gpt-4".to_string()),
    })
    .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/sessions")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_get_session_not_found() {
    let (app, _temp_dir) = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/sessions/00000000-0000-0000-0000-000000000000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_session_invalid_id() {
    let (app, _temp_dir) = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/sessions/invalid-uuid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_list_tools() {
    let (app, _temp_dir) = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/tools")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_list_plugins() {
    let (app, _temp_dir) = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/plugins")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_chat_endpoint() {
    let (app, _temp_dir) = create_test_app();

    let body = r#"{"message": "Hello", "session_id": null}"#;

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/chat")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
