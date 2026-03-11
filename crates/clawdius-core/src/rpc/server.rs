//! JSON-RPC server implementation

use std::collections::HashMap;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufWriter};
use tokio::sync::RwLock;

use super::handlers::Handler;
use super::methods::Method;
use super::types::{Error as RpcError, Id, Request, Response};
use crate::error::Result;

/// RPC server for handling JSON-RPC requests
pub struct RpcServer {
    handlers: Arc<RwLock<HashMap<String, Arc<dyn Handler>>>>,
    #[allow(dead_code)]
    next_id: Arc<RwLock<i64>>,
}

impl RpcServer {
    /// Create a new RPC server
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(RwLock::new(0)),
        }
    }

    /// Register a handler for a method
    pub async fn register_handler(&self, method: impl Into<String>, handler: Arc<dyn Handler>) {
        let mut handlers = self.handlers.write().await;
        handlers.insert(method.into(), handler);
    }

    /// Get next request ID
    #[allow(dead_code)]
    async fn next_id(&self) -> Id {
        let mut id = self.next_id.write().await;
        *id += 1;
        Id::number(*id)
    }

    /// Handle a single request
    pub async fn handle_request(&self, request: Request) -> Response {
        let handlers = self.handlers.read().await;

        // Try to parse method
        let method: Option<Method> = request.method.parse().ok();

        match method {
            Some(_method) => {
                if let Some(handler) = handlers.get(&request.method) {
                    handler.handle(request).await
                } else {
                    Response::method_not_found(request.id, &request.method)
                }
            }
            None => Response::method_not_found(request.id, &request.method),
        }
    }

    /// Run the server on stdio (for VSCode extension)
    pub async fn run_stdio(&self) -> Result<()> {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();

        let mut reader = tokio::io::BufReader::new(stdin);
        let mut writer = BufWriter::new(stdout);

        let mut line = String::new();

        loop {
            line.clear();

            // Read a line (JSON-RPC request)
            let bytes_read = reader
                .read_line(&mut line)
                .await
                .map_err(|e| crate::Error::Rpc(e.to_string()))?;

            if bytes_read == 0 {
                // EOF, client disconnected
                break;
            }

            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Parse request
            let request: Request = match serde_json::from_str(line) {
                Ok(req) => req,
                Err(e) => {
                    let error = RpcError::parse_error(e.to_string());
                    let response = Response::error(Id::null(), error);
                    let json = response.to_json().unwrap_or_default();
                    writer.write_all(json.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    writer.flush().await?;
                    continue;
                }
            };

            // Handle request
            let response = self.handle_request(request).await;

            // Send response
            let json = response.to_json().unwrap_or_default();
            writer.write_all(json.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
        }

        Ok(())
    }
}

impl Default for RpcServer {
    fn default() -> Self {
        Self::new()
    }
}
