use axum::routing::{get, post};
use axum::Router;
use clawdius_web::app::App;
use clawdius_web::server;
use leptos::prelude::*;
use leptos_axum::render_app_to_stream;
use serde_json::json;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

// API handlers

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

async fn send_message_handler(
    axum::Json(req): axum::Json<server::SendMessageRequest>,
) -> axum::Json<serde_json::Value> {
    let response = server::send_message(req).await;
    axum::Json(json!({
        "response": response.response,
        "session_id": response.session_id,
    }))
}

/// Prometheus metrics endpoint.
async fn metrics_handler() -> impl axum::response::IntoResponse {
    (
        [("content-type", "text/plain; version=0.0.4")],
        clawdius_core::metrics::render_metrics(),
    )
}

/// SSE streaming endpoint for chat messages.
/// Returns a text/event-stream with partial responses.
async fn chat_stream_handler() -> axum::response::Response {
    use axum::response::IntoResponse;
    use axum::response::Sse;
    use futures::stream::Stream;
    use std::convert::Infallible;
    use std::time::Duration;

    let stream = async_stream::stream! {
        // Initial connection event
        yield Ok::<_, Infallible>(axum::response::sse::Event::default()
            .event("connected")
            .data("{}"));

        // Simulated streaming response
        for i in 0..5 {
            tokio::time::sleep(Duration::from_millis(200)).await;
            yield Ok::<_, Infallible>(axum::response::sse::Event::default()
                .event("token")
                .data(json!({
                    "content": format!("token_{} ", i),
                    "index": i,
                }).to_string()));
        }

        // Done event
        yield Ok::<_, Infallible>(axum::response::sse::Event::default()
            .event("done")
            .data(json!({"total_tokens": 5}).to_string()));
    };

    Sse::new(stream).into_response()
}

fn app_router() -> Router {
    let cors = CorsLayer::permissive();

    let router = Router::new()
        // API routes
        .route("/api/health", get(health_handler))
        .route("/api/models", get(models_handler))
        .route("/api/sessions", get(sessions_handler))
        .route("/api/messages/send", post(send_message_handler))
        // SSE streaming endpoint
        .route("/api/chat/stream", get(chat_stream_handler))
        // Prometheus metrics
        .route("/metrics", get(metrics_handler));

    // Mount OIDC auth routes (if auth feature is enabled)
    #[cfg(feature = "auth")]
    let router = {
        let auth_config = clawdius_auth::AuthConfig::default();
        match clawdius_auth::AuthService::new(auth_config) {
            Ok(service) => {
                let auth_arc = std::sync::Arc::new(service);
                eprintln!("OIDC auth routes mounted");
                router
                    .merge(clawdius_auth::auth_routes(std::sync::Arc::clone(&auth_arc)))
                    .layer(axum::Extension(auth_arc))
            }
            Err(e) => {
                eprintln!("Failed to initialize OIDC auth: {e}");
                router
            }
        }
    };

    router
        // Leptos SSR fallback
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
