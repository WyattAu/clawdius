//! Local/self-hosted LLM provider (Ollama-compatible API)

use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::llm::providers::LlmClient;
use crate::llm::{ChatMessage, ChatRole};
use crate::{Error, Result};

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    message: OllamaMessage,
}

#[derive(Debug, Deserialize)]
struct OllamaStreamResponse {
    message: Option<OllamaMessage>,
    #[allow(dead_code)]
    done: bool,
}

/// Model information from Ollama API
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelInfo {
    pub name: String,
    #[serde(default)]
    pub modified_at: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub digest: Option<String>,
}

/// List of models response from Ollama API
#[derive(Debug, Deserialize)]
struct ModelsResponse {
    models: Vec<ModelInfo>,
}

pub struct LocalLlmProvider {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl LocalLlmProvider {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            base_url,
            model,
            client: reqwest::Client::new(),
        }
    }

    pub fn llama(host: &str) -> Self {
        Self::new(format!("http://{host}:11434"), "llama3.2".to_string())
    }

    pub fn mistral(host: &str) -> Self {
        Self::new(format!("http://{host}:11434"), "mistral".to_string())
    }

    /// Create provider with deepseek-coder model
    pub fn deepseek_coder(host: &str) -> Self {
        Self::new(format!("http://{host}:11434"), "deepseek-coder".to_string())
    }

    /// Create provider with codellama model
    pub fn codellama(host: &str) -> Self {
        Self::new(format!("http://{host}:11434"), "codellama".to_string())
    }

    /// Create provider with phi-3 model
    pub fn phi3(host: &str) -> Self {
        Self::new(format!("http://{host}:11434"), "phi3".to_string())
    }

    /// Create provider with qwen model
    pub fn qwen(host: &str) -> Self {
        Self::new(format!("http://{host}:11434"), "qwen2.5".to_string())
    }

    /// List available models from the Ollama server
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let response = self
            .client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .map_err(|e| Error::Llm(format!("Failed to list models: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Llm(format!(
                "Failed to list models: {status} - {body}"
            )));
        }

        let models: ModelsResponse = response
            .json()
            .await
            .map_err(|e| Error::Llm(format!("Failed to parse models response: {e}")))?;

        Ok(models.models)
    }

    /// Pull a model from Ollama registry
    pub async fn pull_model(&self, model_name: &str) -> Result<()> {
        let response = self
            .client
            .post(format!("{}/api/pull", self.base_url))
            .json(&serde_json::json!({ "name": model_name, "stream": false }))
            .send()
            .await
            .map_err(|e| Error::Llm(format!("Failed to pull model: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Llm(format!(
                "Failed to pull model: {status} - {body}"
            )));
        }

        Ok(())
    }

    /// Check if the Ollama server is reachable
    pub async fn health_check(&self) -> Result<bool> {
        let response = self
            .client
            .get(format!("{}/api/version", self.base_url))
            .send()
            .await
            .map_err(|e| Error::Llm(format!("Health check failed: {e}")))?;

        Ok(response.status().is_success())
    }
}

#[async_trait]
impl LlmClient for LocalLlmProvider {
    async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String> {
        let ollama_messages: Vec<OllamaMessage> = messages
            .into_iter()
            .map(|m| OllamaMessage {
                role: match m.role {
                    ChatRole::System => "system".to_string(),
                    ChatRole::User => "user".to_string(),
                    ChatRole::Assistant => "assistant".to_string(),
                },
                content: m.content,
            })
            .collect();

        let request = OllamaRequest {
            model: self.model.clone(),
            messages: ollama_messages,
            stream: false,
        };

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Llm(format!("Failed to send request: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Llm(format!(
                "Request failed with status {status}: {body}"
            )));
        }

        let json: OllamaResponse = response
            .json()
            .await
            .map_err(|e| Error::Llm(format!("Failed to parse response: {e}")))?;

        Ok(json.message.content)
    }

    async fn chat_stream(&self, messages: Vec<ChatMessage>) -> Result<mpsc::Receiver<String>> {
        let (tx, rx) = mpsc::channel(100);

        let ollama_messages: Vec<OllamaMessage> = messages
            .into_iter()
            .map(|m| OllamaMessage {
                role: match m.role {
                    ChatRole::System => "system".to_string(),
                    ChatRole::User => "user".to_string(),
                    ChatRole::Assistant => "assistant".to_string(),
                },
                content: m.content,
            })
            .collect();

        let request = OllamaRequest {
            model: self.model.clone(),
            messages: ollama_messages,
            stream: true,
        };

        let base_url = self.base_url.clone();
        let client = self.client.clone();

        tokio::spawn(async move {
            match client
                .post(format!("{base_url}/api/chat"))
                .json(&request)
                .send()
                .await
            {
                Ok(response) => {
                    if !response.status().is_success() {
                        let status = response.status();
                        let _ = tx.send(format!("[Error: HTTP {status}]")).await;
                        return;
                    }

                    let mut stream = response.bytes_stream();
                    while let Some(chunk_result) = stream.next().await {
                        match chunk_result {
                            Ok(chunk) => {
                                // Ollama sends newline-delimited JSON
                                let chunk_str = String::from_utf8_lossy(&chunk);
                                for line in chunk_str.lines() {
                                    if line.is_empty() {
                                        continue;
                                    }
                                    if let Ok(stream_response) =
                                        serde_json::from_str::<OllamaStreamResponse>(line)
                                    {
                                        if let Some(msg) = stream_response.message {
                                            if tx.send(msg.content).await.is_err() {
                                                return;
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(format!("[Error: {e}]")).await;
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(format!("[Error: {e}]")).await;
                }
            }
        });

        Ok(rx)
    }

    fn count_tokens(&self, text: &str) -> usize {
        // Detect if content looks like code (has common code patterns)
        let is_code = text.contains("fn ")
            || text.contains("function ")
            || text.contains("class ")
            || text.contains("def ")
            || text.contains("import ")
            || text.contains("export ")
            || text.contains("const ")
            || text.contains("let ")
            || text.contains("var ");

        // For code, use character-based approximation (more tokens than whitespace)
        // For natural language, use word-based with adjustment
        if is_code {
            // Code typically has ~3-4 chars per token
            let char_count = text.chars().count();
            let punct_count = text.chars().filter(|c| c.is_ascii_punctuation()).count();
            
            // Base: 4 chars per token, punctuation adds extra
            ((char_count as f64 / 4.0).ceil() as usize) + (punct_count / 3).max(1)
        } else {
            // Natural language: ~4 chars per token + punctuation adjustment
            let words = text.split_whitespace().count();
            let punct_count = text.chars().filter(|c| c.is_ascii_punctuation()).count();
            
            words + (punct_count / 4)
        }
    }
}
