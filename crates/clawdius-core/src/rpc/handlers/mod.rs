//! RPC request handlers

pub mod completion;

use async_trait::async_trait;

use super::types::{Request, Response};

pub use completion::CompletionHandler;

#[async_trait]
pub trait Handler: Send + Sync {
    async fn handle(&self, request: Request) -> Response;
}

pub struct SessionHandler;

#[async_trait]
impl Handler for SessionHandler {
    async fn handle(&self, request: Request) -> Response {
        Response::success(request.id, serde_json::json!({"status": "ok"}))
    }
}

pub struct ChatHandler;

#[async_trait]
impl Handler for ChatHandler {
    async fn handle(&self, request: Request) -> Response {
        Response::success(request.id, serde_json::json!({"response": "ok"}))
    }
}

pub struct FileHandler;

#[async_trait]
impl Handler for FileHandler {
    async fn handle(&self, request: Request) -> Response {
        Response::success(request.id, serde_json::json!({"status": "ok"}))
    }
}

pub struct ContextHandler;

#[async_trait]
impl Handler for ContextHandler {
    async fn handle(&self, request: Request) -> Response {
        Response::success(request.id, serde_json::json!({"status": "ok"}))
    }
}

pub struct StateHandler;

#[async_trait]
impl Handler for StateHandler {
    async fn handle(&self, request: Request) -> Response {
        if request.method == "state/checkpoint" {
            Response::success(request.id, serde_json::json!({"id": "checkpoint-1"}))
        } else {
            Response::success(request.id, serde_json::json!({"status": "ok"}))
        }
    }
}
