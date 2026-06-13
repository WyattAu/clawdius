use axum::routing::get;
use axum::Router;
use clawdius_web::app::App;
use clawdius_web::server;
use leptos::prelude::*;
use leptos_axum::render_app_to_stream;
use serde_json::json;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

async fn health_handler() -> axum::Json<serde_json::Value> {
    let status = server::get_health_status();
    axum::Json(json!({
        "status": status.status,
        "version": status.version,
    }))
}

async fn models_handler() -> axum::Json<serde_json::Value> {
    let models = server::list_models();
    axum::Json(json!({ "models": models }))
}

async fn sessions_handler() -> axum::Json<serde_json::Value> {
    let sessions = server::list_sessions();
    axum::Json(json!({ "sessions": sessions }))
}

fn app_router() -> Router {
    let cors = CorsLayer::permissive();

    Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/models", get(models_handler))
        .route("/api/sessions", get(sessions_handler))
        .fallback(render_app_to_stream(|| view! { <App /> }))
        .layer(cors)
}

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Clawdius web server starting on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind to address");

    axum::serve(listener, app_router().into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for shutdown signal");
    println!("Shutting down gracefully...");
}
