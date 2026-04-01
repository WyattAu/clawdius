//! LSP Client Implementation
//!
//! Client for communicating with Language Servers.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{oneshot, Mutex, RwLock};

use super::protocol::*;

/// LSP client configuration.
#[derive(Debug, Clone)]
pub struct LspClientConfig {
    /// Command to start the language server
    pub command: String,
    /// Arguments for the command
    pub args: Vec<String>,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Working directory
    pub cwd: Option<PathBuf>,
    /// Initialization timeout in milliseconds
    pub timeout_ms: u64,
}

impl Default for LspClientConfig {
    fn default() -> Self {
        Self {
            command: String::new(),
            args: Vec::new(),
            env: HashMap::new(),
            cwd: None,
            timeout_ms: 30_000, // 30 seconds - rust-analyzer can be slow to initialize
        }
    }
}

impl LspClientConfig {
    /// Creates a new config for a language server.
    #[must_use]
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            ..Default::default()
        }
    }

    /// Adds arguments.
    #[must_use]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Sets the working directory.
    #[must_use]
    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Sets the timeout in milliseconds.
    #[must_use]
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
}

/// LSP client for a single language server.
pub struct LspClient {
    /// Configuration
    config: LspClientConfig,
    /// Request ID counter
    request_id: AtomicU64,
    /// Server process
    process: Option<Child>,
    /// Stdin for sending messages (wrapped for interior mutability)
    stdin: Option<Arc<Mutex<ChildStdin>>>,
    /// Pending requests (wrapped for sharing with reader task)
    pending: Arc<RwLock<HashMap<u64, oneshot::Sender<LspResponse>>>>,
    /// Server capabilities
    capabilities: RwLock<Option<ServerCapabilities>>,
    /// Running flag
    running: Arc<std::sync::atomic::AtomicBool>,
}

/// Server capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    /// Text document sync
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_document_sync: Option<TextDocumentSyncCapability>,
    /// Completion provider
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_provider: Option<CompletionCapability>,
    /// Hover provider
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover_provider: Option<bool>,
    /// Definition provider
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition_provider: Option<bool>,
    /// References provider
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references_provider: Option<bool>,
    /// Document symbol provider
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_symbol_provider: Option<bool>,
    /// Workspace symbol provider
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_symbol_provider: Option<bool>,
    /// Code action provider
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_action_provider: Option<bool>,
}

/// Text document sync capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDocumentSyncCapability {
    /// Sync kind
    pub change: Option<i32>,
}

/// Completion capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionCapability {
    /// Trigger characters
    #[serde(default)]
    pub trigger_characters: Vec<String>,
}

/// LSP request.
#[derive(Debug, Clone, Serialize)]
struct LspRequest<T> {
    jsonrpc: String,
    id: u64,
    method: String,
    params: T,
}

/// LSP response.
#[derive(Debug, Clone, Deserialize)]
struct LspResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    id: u64,
    result: Option<serde_json::Value>,
    #[allow(dead_code)]
    error: Option<LspResponseError>,
}

/// LSP response error.
#[derive(Debug, Clone, Deserialize)]
struct LspResponseError {
    #[allow(dead_code)]
    code: i32,
    #[allow(dead_code)]
    message: String,
}

/// Initialize params.
#[derive(Debug, Clone, Serialize)]
struct InitializeParams {
    capabilities: ClientCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    root_uri: Option<String>,
}

/// Client capabilities.
#[derive(Debug, Clone, Default, Serialize)]
struct ClientCapabilities {
    text_document: Option<TextDocumentClientCapabilities>,
}

/// Text document client capabilities.
#[derive(Debug, Clone, Default, Serialize)]
struct TextDocumentClientCapabilities {
    completion: Option<CompletionClientCapabilities>,
    hover: Option<HoverClientCapabilities>,
    definition: Option<GenericCapability>,
    references: Option<GenericCapability>,
    document_symbol: Option<GenericCapability>,
    code_action: Option<GenericCapability>,
}

/// Generic capability.
#[derive(Debug, Clone, Default, Serialize)]
struct GenericCapability {
    dynamic_registration: bool,
}

/// Completion client capabilities.
#[derive(Debug, Clone, Default, Serialize)]
struct CompletionClientCapabilities {
    dynamic_registration: bool,
    completion_item: Option<CompletionItemCapability>,
}

/// Completion item capability.
#[derive(Debug, Clone, Default, Serialize)]
struct CompletionItemCapability {
    snippet_support: bool,
    documentation_format: Vec<String>,
}

/// Hover client capabilities.
#[derive(Debug, Clone, Default, Serialize)]
struct HoverClientCapabilities {
    content_format: Vec<String>,
}

/// Did open text document params.
#[derive(Debug, Clone, Serialize)]
struct DidOpenTextDocumentParams {
    text_document: TextDocumentItem,
}

/// Text document item.
#[derive(Debug, Clone, Serialize)]
struct TextDocumentItem {
    uri: String,
    language_id: String,
    version: i32,
    text: String,
}

/// Did change text document params.
#[derive(Debug, Clone, Serialize)]
struct DidChangeTextDocumentParams {
    text_document: VersionedTextDocumentIdentifier,
    content_changes: Vec<TextDocumentContentChangeEvent>,
}

/// Versioned text document identifier.
#[derive(Debug, Clone, Serialize)]
struct VersionedTextDocumentIdentifier {
    uri: String,
    version: i32,
}

/// Text document content change event.
#[derive(Debug, Clone, Serialize)]
struct TextDocumentContentChangeEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    range: Option<Range>,
    text: String,
}

impl LspClient {
    /// Creates a new LSP client.
    #[must_use]
    pub fn new(config: LspClientConfig) -> Self {
        Self {
            config,
            request_id: AtomicU64::new(1),
            process: None,
            stdin: None,
            pending: Arc::new(RwLock::new(HashMap::new())),
            capabilities: RwLock::new(None),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Starts the language server.
    ///
    /// # Errors
    ///
    /// Returns an error if the server cannot be started.
    pub async fn start(&mut self, root_uri: Option<&str>) -> Result<()> {
        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.args);

        for (key, value) in &self.config.env {
            cmd.env(key, value);
        }

        if let Some(cwd) = &self.config.cwd {
            cmd.current_dir(cwd);
        }

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take().map(|s| Arc::new(Mutex::new(s)));
        let stdout = child.stdout.take();

        self.stdin = stdin;
        self.process = Some(child);
        self.running.store(true, Ordering::SeqCst);

        // Start message reader
        if let Some(stdout) = stdout {
            self.start_reader(stdout);
        }

        // Initialize
        self.initialize(root_uri).await?;

        Ok(())
    }

    /// Stops the language server.
    pub async fn stop(&mut self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);

        if let Some(mut process) = self.process.take() {
            let _ = process.kill().await;
        }

        self.stdin = None;
        Ok(())
    }

    /// Returns whether the client is connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.running.load(Ordering::SeqCst) && self.process.is_some()
    }

    /// Returns server capabilities.
    #[must_use]
    pub async fn capabilities(&self) -> Option<ServerCapabilities> {
        self.capabilities.read().await.clone()
    }

    /// Opens a text document.
    ///
    /// # Errors
    ///
    /// Returns an error if the notification fails.
    pub async fn open_document(&self, uri: &str, language_id: &str, text: &str) -> Result<()> {
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.to_string(),
                language_id: language_id.to_string(),
                version: 1,
                text: text.to_string(),
            },
        };

        self.send_notification("textDocument/didOpen", params).await
    }

    /// Updates a text document.
    ///
    /// # Errors
    ///
    /// Returns an error if the notification fails.
    pub async fn change_document(&self, uri: &str, version: i32, text: &str) -> Result<()> {
        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.to_string(),
                version,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                text: text.to_string(),
            }],
        };

        self.send_notification("textDocument/didChange", params)
            .await
    }

    /// Requests completion.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn completion(&self, uri: &str, position: Position) -> Result<CompletionList> {
        let params = TextDocumentPositionParams::new(uri, position);
        let response = self.send_request("textDocument/completion", params).await?;

        match response.result {
            Some(result) => {
                // Handle both CompletionList and Vec<CompletionItem>
                if let Ok(list) = serde_json::from_value::<CompletionList>(result.clone()) {
                    Ok(list)
                } else if let Ok(items) = serde_json::from_value::<Vec<CompletionItem>>(result) {
                    Ok(CompletionList::from_items(items))
                } else {
                    Ok(CompletionList::empty())
                }
            },
            None => Ok(CompletionList::empty()),
        }
    }

    /// Requests hover information.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn hover(&self, uri: &str, position: Position) -> Result<Option<Hover>> {
        let params = TextDocumentPositionParams::new(uri, position);
        let response = self.send_request("textDocument/hover", params).await?;

        match response.result {
            Some(result) => Ok(serde_json::from_value(result).ok()),
            None => Ok(None),
        }
    }

    /// Requests go to definition.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn definition(&self, uri: &str, position: Position) -> Result<Vec<Location>> {
        let params = TextDocumentPositionParams::new(uri, position);
        let response = self.send_request("textDocument/definition", params).await?;

        match response.result {
            Some(result) => {
                // Handle single location or array
                if let Ok(loc) = serde_json::from_value::<Location>(result.clone()) {
                    Ok(vec![loc])
                } else if let Ok(locs) = serde_json::from_value::<Vec<Location>>(result) {
                    Ok(locs)
                } else {
                    Ok(Vec::new())
                }
            },
            None => Ok(Vec::new()),
        }
    }

    /// Requests find references.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn references(
        &self,
        uri: &str,
        position: Position,
        include_declaration: bool,
    ) -> Result<Vec<Location>> {
        let params = serde_json::json!({
            "textDocument": { "uri": uri },
            "position": position,
            "context": { "includeDeclaration": include_declaration }
        });

        let response = self.send_request("textDocument/references", params).await?;

        match response.result {
            Some(result) => Ok(serde_json::from_value(result).unwrap_or_default()),
            None => Ok(Vec::new()),
        }
    }

    /// Requests document symbols.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn document_symbols(&self, uri: &str) -> Result<Vec<DocumentSymbol>> {
        let params = serde_json::json!({
            "textDocument": { "uri": uri }
        });

        let response = self
            .send_request("textDocument/documentSymbol", params)
            .await?;

        match response.result {
            Some(result) => Ok(serde_json::from_value(result).unwrap_or_default()),
            None => Ok(Vec::new()),
        }
    }

    /// Requests code actions.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn code_actions(
        &self,
        uri: &str,
        range: Range,
        diagnostics: Vec<Diagnostic>,
    ) -> Result<Vec<CodeAction>> {
        let params = serde_json::json!({
            "textDocument": { "uri": uri },
            "range": range,
            "context": { "diagnostics": diagnostics }
        });

        let response = self.send_request("textDocument/codeAction", params).await?;

        match response.result {
            Some(result) => Ok(serde_json::from_value(result).unwrap_or_default()),
            None => Ok(Vec::new()),
        }
    }

    /// Initializes the connection.
    async fn initialize(&mut self, root_uri: Option<&str>) -> Result<()> {
        let capabilities = ClientCapabilities {
            text_document: Some(TextDocumentClientCapabilities {
                completion: Some(CompletionClientCapabilities {
                    dynamic_registration: false,
                    completion_item: Some(CompletionItemCapability {
                        snippet_support: true,
                        documentation_format: vec!["markdown".to_string(), "plaintext".to_string()],
                    }),
                }),
                hover: Some(HoverClientCapabilities {
                    content_format: vec!["markdown".to_string(), "plaintext".to_string()],
                }),
                definition: Some(GenericCapability {
                    dynamic_registration: false,
                }),
                references: Some(GenericCapability {
                    dynamic_registration: false,
                }),
                document_symbol: Some(GenericCapability {
                    dynamic_registration: false,
                }),
                code_action: Some(GenericCapability {
                    dynamic_registration: false,
                }),
            }),
        };

        let params = InitializeParams {
            capabilities,
            root_uri: root_uri.map(String::from),
        };

        let response = self.send_request("initialize", params).await?;

        if let Some(result) = response.result {
            let init_result: serde_json::Value = result;
            if let Some(caps) = init_result.get("capabilities") {
                let caps: ServerCapabilities = serde_json::from_value(caps.clone())?;
                *self.capabilities.write().await = Some(caps);
            }
        }

        // Send initialized notification
        self.send_notification("initialized", serde_json::json!({}))
            .await?;

        Ok(())
    }

    /// Sends a request.
    async fn send_request<T: Serialize>(&self, method: &str, params: T) -> Result<LspResponse> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();

        self.pending.write().await.insert(id, tx);

        let request = LspRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let json = serde_json::to_string(&request)?;
        let message = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);

        if let Some(stdin) = &self.stdin {
            let mut stdin = stdin.lock().await;
            stdin.write_all(message.as_bytes()).await?;
            stdin.flush().await?;
        }

        let timeout_duration = tokio::time::Duration::from_millis(self.config.timeout_ms);
        let response = tokio::time::timeout(timeout_duration, rx)
            .await
            .map_err(|_| Error::Timeout(std::time::Duration::from_millis(self.config.timeout_ms)))?
            .map_err(|_| Error::Other("LSP response channel closed".to_string()))?;

        Ok(response)
    }

    /// Sends a notification.
    async fn send_notification<T: Serialize>(&self, method: &str, params: T) -> Result<()> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        let json = serde_json::to_string(&notification)?;
        let message = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);

        if let Some(stdin) = &self.stdin {
            let mut stdin = stdin.lock().await;
            stdin.write_all(message.as_bytes()).await?;
            stdin.flush().await?;
        }

        Ok(())
    }

    /// Starts the message reader.
    fn start_reader(&self, stdout: ChildStdout) {
        let pending = Arc::clone(&self.pending);
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut reader = BufReader::new(stdout);

            while running.load(Ordering::SeqCst) {
                // Read headers
                let mut content_length: Option<usize> = None;

                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line).await {
                        Ok(0) => {
                            // EOF
                            return;
                        },
                        Ok(_) => {
                            let line = line.trim();
                            if line.is_empty() {
                                // Empty line marks end of headers
                                break;
                            }
                            if let Some(stripped) = line.strip_prefix("Content-Length: ") {
                                content_length = stripped.parse().ok();
                            }
                        },
                        Err(e) => {
                            tracing::error!("LSP reader error reading header: {}", e);
                            return;
                        },
                    }
                }

                // Read content
                if let Some(len) = content_length {
                    let mut buffer = vec![0u8; len];
                    match reader.read_exact(&mut buffer).await {
                        Ok(_) => match serde_json::from_slice::<LspResponse>(&buffer) {
                            Ok(response) => {
                                if let Some(tx) = pending.write().await.remove(&response.id) {
                                    let _ = tx.send(response);
                                }
                            },
                            Err(e) => {
                                tracing::debug!(
                                    "Failed to parse LSP response: {} - content: {}",
                                    e,
                                    String::from_utf8_lossy(&buffer)
                                );
                            },
                        },
                        Err(e) => {
                            tracing::error!("LSP reader error reading content: {}", e);
                            return;
                        },
                    }
                }
            }
        });
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config() {
        let config = LspClientConfig::new("rust-analyzer").with_args(vec!["--stdio".to_string()]);

        assert_eq!(config.command, "rust-analyzer");
        assert_eq!(config.args, vec!["--stdio"]);
    }

    #[test]
    fn test_client_creation() {
        let client = LspClient::new(LspClientConfig::new("test"));
        assert!(!client.is_connected());
    }
}
