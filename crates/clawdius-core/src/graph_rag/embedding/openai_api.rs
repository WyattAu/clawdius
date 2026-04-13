//! OpenAI-compatible API embedder
//!
//! Supports any OpenAI-compatible embeddings endpoint including OpenAI,
//! Ollama (localhost:11434), and other compatible providers.

use super::EmbeddingGenerator;
use crate::error::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "text-embedding-3-small";
const DEFAULT_DIMENSION: usize = 1536;
const DEFAULT_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiApiConfig {
    pub api_key: String,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_model")]
    pub model: String,
    pub dimension: Option<usize>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_base_url() -> String {
    DEFAULT_BASE_URL.to_string()
}
fn default_model() -> String {
    DEFAULT_MODEL.to_string()
}
fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_SECS
}

impl Default for OpenAiApiConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: default_base_url(),
            model: default_model(),
            dimension: None,
            timeout_secs: default_timeout(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EmbeddingRequest<'a> {
    model: &'a str,
    input: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
    #[allow(dead_code)]
    model: String,
    #[allow(dead_code)]
    object: String,
    #[allow(dead_code)]
    usage: Option<Usage>,
}

#[derive(Debug, Clone, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
    #[allow(dead_code)]
    index: usize,
    #[allow(dead_code)]
    object: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Usage {
    #[allow(dead_code)]
    prompt_tokens: u64,
    #[allow(dead_code)]
    total_tokens: u64,
}

#[derive(Clone)]
pub struct OpenAiApiEmbedder {
    client: reqwest::Client,
    config: OpenAiApiConfig,
    dimension: usize,
}

impl OpenAiApiEmbedder {
    pub fn new(config: OpenAiApiConfig) -> Result<Self> {
        let dimension = config.dimension.unwrap_or(DEFAULT_DIMENSION);
        let timeout = std::time::Duration::from_secs(config.timeout_secs);

        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| Error::Config(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self {
            client,
            config,
            dimension,
        })
    }

    fn embeddings_url(&self) -> String {
        let base = self.config.base_url.trim_end_matches('/');
        format!("{base}/embeddings")
    }

    fn normalize(vec: &mut [f32]) {
        let norm: f32 = vec.iter().map(|x| x * x).sum();
        if norm > 0.0 {
            let scale = 1.0 / norm.sqrt();
            for val in vec.iter_mut() {
                *val *= scale;
            }
        }
    }

    async fn call_api(&self, text: &str) -> Result<Vec<f32>> {
        let url = self.embeddings_url();
        let body = EmbeddingRequest {
            model: &self.config.model,
            input: text,
        };

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    Error::Timeout(std::time::Duration::from_secs(self.config.timeout_secs))
                } else if e.is_connect() {
                    Error::Config(format!("Cannot connect to embeddings API at {url}: {e}"))
                } else {
                    Error::Llm(format!("Embeddings API request failed: {e}"))
                }
            })?;

        let status = response.status();
        let response_body = response
            .text()
            .await
            .map_err(|e| Error::Llm(format!("Failed to read embeddings API response: {e}")))?;

        if !status.is_success() {
            if status.as_u16() == 401 || status.as_u16() == 403 {
                return Err(Error::Auth(format!(
                    "Embeddings API authentication failed (HTTP {status}): {response_body}"
                )));
            }
            if status.as_u16() == 429 {
                return Err(Error::RateLimited {
                    retry_after_ms: 5000,
                });
            }
            return Err(Error::Llm(format!(
                "Embeddings API error (HTTP {status}): {response_body}"
            )));
        }

        let parsed: EmbeddingResponse = serde_json::from_str(&response_body)
            .map_err(|e| Error::Llm(format!("Failed to parse embeddings API response: {e}")))?;

        parsed
            .data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .ok_or_else(|| Error::Llm("Embeddings API returned no data".to_string()))
    }

    #[cfg(test)]
    fn embeddings_url_test(&self) -> String {
        self.embeddings_url()
    }
}

#[async_trait]
impl EmbeddingGenerator for OpenAiApiEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let mut embedding = self.call_api(text).await?;
        Self::normalize(&mut embedding);
        Ok(embedding)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

pub fn normalize(vec: &mut [f32]) {
    OpenAiApiEmbedder::normalize(vec);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OpenAiApiConfig::default();
        assert_eq!(config.base_url, "https://api.openai.com/v1");
        assert_eq!(config.model, "text-embedding-3-small");
        assert_eq!(config.timeout_secs, 30);
        assert!(config.dimension.is_none());
    }

    #[test]
    fn test_url_construction_openai() {
        let config = OpenAiApiConfig {
            api_key: "sk-test".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            model: "text-embedding-3-small".to_string(),
            dimension: None,
            timeout_secs: 30,
        };
        let embedder = OpenAiApiEmbedder::new(config).unwrap();
        assert_eq!(
            embedder.embeddings_url_test(),
            "https://api.openai.com/v1/embeddings"
        );
    }

    #[test]
    fn test_url_construction_trailing_slash() {
        let config = OpenAiApiConfig {
            api_key: "test".to_string(),
            base_url: "http://localhost:11434/v1/".to_string(),
            model: "nomic-embed-text".to_string(),
            dimension: Some(768),
            timeout_secs: 60,
        };
        let embedder = OpenAiApiEmbedder::new(config).unwrap();
        assert_eq!(
            embedder.embeddings_url_test(),
            "http://localhost:11434/v1/embeddings"
        );
    }

    #[test]
    fn test_url_construction_ollama() {
        let config = OpenAiApiConfig {
            api_key: "ollama".to_string(),
            base_url: "http://localhost:11434".to_string(),
            model: "nomic-embed-text".to_string(),
            dimension: Some(768),
            timeout_secs: 60,
        };
        let embedder = OpenAiApiEmbedder::new(config).unwrap();
        assert_eq!(
            embedder.embeddings_url_test(),
            "http://localhost:11434/embeddings"
        );
    }

    #[test]
    fn test_parse_response() {
        let json = r#"{
            "object": "list",
            "model": "text-embedding-3-small",
            "data": [
                {"object": "embedding", "index": 0, "embedding": [0.1, 0.2, 0.3]}
            ],
            "usage": {"prompt_tokens": 5, "total_tokens": 5}
        }"#;
        let parsed: EmbeddingResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.data.len(), 1);
        assert_eq!(parsed.data[0].embedding, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn test_parse_response_empty_data() {
        let json = r#"{
            "object": "list",
            "model": "text-embedding-3-small",
            "data": [],
            "usage": {"prompt_tokens": 0, "total_tokens": 0}
        }"#;
        let parsed: EmbeddingResponse = serde_json::from_str(json).unwrap();
        assert!(parsed.data.is_empty());
    }

    #[test]
    fn test_normalize() {
        let mut vec = vec![3.0, 4.0];
        OpenAiApiEmbedder::normalize(&mut vec);
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_zero_vector() {
        let mut vec = vec![0.0, 0.0, 0.0];
        OpenAiApiEmbedder::normalize(&mut vec);
        assert!(vec.iter().all(|&x| x == 0.0));
    }

    #[tokio::test]
    async fn test_embedder_dimension() {
        let config = OpenAiApiConfig {
            api_key: "test".to_string(),
            base_url: "http://localhost:9999".to_string(),
            model: "test-model".to_string(),
            dimension: Some(512),
            timeout_secs: 1,
        };
        let embedder = OpenAiApiEmbedder::new(config).unwrap();
        assert_eq!(embedder.dimension(), 512);
    }

    #[tokio::test]
    async fn test_error_connection_refused() {
        let config = OpenAiApiConfig {
            api_key: "test".to_string(),
            base_url: "http://localhost:1".to_string(),
            model: "test".to_string(),
            dimension: None,
            timeout_secs: 1,
        };
        let embedder = OpenAiApiEmbedder::new(config).unwrap();
        let result = embedder.embed("test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn test_real_openai_api() {
        let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
        let config = OpenAiApiConfig {
            api_key,
            base_url: "https://api.openai.com/v1".to_string(),
            model: "text-embedding-3-small".to_string(),
            dimension: None,
            timeout_secs: 30,
        };
        let embedder = OpenAiApiEmbedder::new(config).unwrap();
        let embedding = embedder.embed("hello world").await.unwrap();
        assert!(!embedding.is_empty());
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }
}
