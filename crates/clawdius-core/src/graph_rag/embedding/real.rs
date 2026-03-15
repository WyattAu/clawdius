//! Real embeddings using sentence transformers (BERT-based models)

use crate::error::{Error, Result};
use async_trait::async_trait;
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use hf_hub::{api::sync::Api, Repo};
use std::path::Path;
use tokenizers::Tokenizer;
use tracing::info;

use super::EmbeddingGenerator;

pub struct SentenceEmbedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
    dimension: usize,
}

impl SentenceEmbedder {
    pub fn new(model_name: &str, model_path: Option<&Path>) -> Result<Self> {
        let device = Device::cuda_if_available(0).unwrap_or(Device::Cpu);

        info!("Initializing sentence embedder on device: {:?}", device);

        let (config_path, tokenizer_path, weights_path) =
            Self::download_or_load_model(model_name, model_path)?;

        let config_content = std::fs::read_to_string(&config_path)
            .map_err(|e| Error::Config(format!("Failed to read config: {e}")))?;
        let config: Config = serde_json::from_str(&config_content)
            .map_err(|e| Error::Config(format!("Failed to parse config: {e}")))?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| Error::Config(format!("Failed to load tokenizer: {e}")))?;

        let vb = VarBuilder::from_pth(&weights_path, DType::F32, &device)
            .map_err(|e| Error::Model(format!("Failed to load weights: {e}")))?;

        let model = BertModel::load(vb, &config)
            .map_err(|e| Error::Model(format!("Failed to create model: {e}")))?;

        let dimension = 384;

        info!(
            "Sentence embedder initialized with dimension: {}",
            dimension
        );

        Ok(Self {
            model,
            tokenizer,
            device,
            dimension,
        })
    }

    fn download_or_load_model(
        model_name: &str,
        model_path: Option<&Path>,
    ) -> Result<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf)> {
        if let Some(path) = model_path {
            if path.exists() {
                let config = path.join("config.json");
                let tokenizer = path.join("tokenizer.json");
                let weights = path.join("model.safetensors");

                if !weights.exists() {
                    let weights_alt = path.join("pytorch_model.bin");
                    return Ok((config, tokenizer, weights_alt));
                }

                return Ok((config, tokenizer, weights));
            }
        }

        info!("Downloading model '{}' from HuggingFace Hub...", model_name);

        let api = Api::new().map_err(|e| Error::Model(format!("Failed to create HF API: {e}")))?;

        let repo = Repo::model(model_name.to_string());
        let api_repo = api.repo(repo);

        let config = api_repo
            .get("config.json")
            .map_err(|e| Error::Model(format!("Failed to download config: {e}")))?;

        let tokenizer = api_repo
            .get("tokenizer.json")
            .map_err(|e| Error::Model(format!("Failed to download tokenizer: {e}")))?;

        let weights = if let Ok(weights) = api_repo.get("model.safetensors") {
            weights
        } else {
            api_repo
                .get("pytorch_model.bin")
                .map_err(|e| Error::Model(format!("Failed to download weights: {e}")))?
        };

        info!("Model downloaded successfully");

        Ok((config, tokenizer, weights))
    }

    fn tokenize(&self, text: &str) -> Result<(Tensor, Tensor)> {
        let tokenizer = self.tokenizer.clone();

        let encoding = tokenizer
            .encode(text, true)
            .map_err(|e| Error::Processing(format!("Tokenization failed: {e}")))?;

        let tokens = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();

        let token_tensor = Tensor::new(tokens, &self.device)
            .map_err(|e| Error::Processing(format!("Failed to create token tensor: {e}")))?
            .unsqueeze(0)
            .map_err(|e| Error::Processing(format!("Failed to unsqueeze: {e}")))?;

        let attention_tensor = Tensor::new(attention_mask, &self.device)
            .map_err(|e| Error::Processing(format!("Failed to create attention tensor: {e}")))?
            .unsqueeze(0)
            .map_err(|e| Error::Processing(format!("Failed to unsqueeze: {e}")))?;

        Ok((token_tensor, attention_tensor))
    }

    fn mean_pool_and_normalize(
        &self,
        embeddings: &Tensor,
        attention_mask: &Tensor,
    ) -> Result<Vec<f32>> {
        let mask_expanded = attention_mask
            .to_dtype(DType::F32)
            .map_err(|e| Error::Processing(format!("Failed to convert mask dtype: {e}")))?
            .unsqueeze(2)
            .map_err(|e| Error::Processing(format!("Failed to unsqueeze mask: {e}")))?
            .broadcast_as(embeddings.shape())
            .map_err(|e| Error::Processing(format!("Failed to broadcast mask: {e}")))?;

        let masked_embeddings = (embeddings * &mask_expanded)
            .map_err(|e| Error::Processing(format!("Failed to mask embeddings: {e}")))?;

        let sum_embeddings = masked_embeddings
            .sum(1)
            .map_err(|e| Error::Processing(format!("Failed to sum embeddings: {e}")))?;

        let sum_mask = mask_expanded
            .sum(1)
            .map_err(|e| Error::Processing(format!("Failed to sum mask: {e}")))?
            .clamp(1e-9, f64::MAX)
            .map_err(|e| Error::Processing(format!("Failed to clamp mask: {e}")))?;

        let mean_pooled = (&sum_embeddings / &sum_mask)
            .map_err(|e| Error::Processing(format!("Failed to compute mean: {e}")))?;

        let norm = mean_pooled
            .sqr()
            .map_err(|e| Error::Processing(format!("Failed to square: {e}")))?
            .sum_all()
            .map_err(|e| Error::Processing(format!("Failed to sum squares: {e}")))?
            .sqrt()
            .map_err(|e| Error::Processing(format!("Failed to compute sqrt: {e}")))?;

        let normalized = (&mean_pooled / &norm)
            .map_err(|e| Error::Processing(format!("Failed to normalize: {e}")))?;

        let vec = normalized
            .to_vec1()
            .map_err(|e| Error::Processing(format!("Failed to convert to vec: {e}")))?;

        Ok(vec)
    }

    pub fn embed_sync(&self, text: &str) -> Result<Vec<f32>> {
        let (tokens, attention_mask) = self.tokenize(text)?;

        let embeddings = self
            .model
            .forward(&tokens, &attention_mask)
            .map_err(|e| Error::Model(format!("Forward pass failed: {e}")))?;

        let pooled = self.mean_pool_and_normalize(&embeddings, &attention_mask)?;

        Ok(pooled)
    }

    pub fn embed_batch_sync(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        texts.iter().map(|text| self.embed_sync(text)).collect()
    }
}

#[async_trait]
impl EmbeddingGenerator for SentenceEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.embed_sync(text)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_sentence_embedder() {
        let embedder = SentenceEmbedder::new("sentence-transformers/all-MiniLM-L6-v2", None)
            .expect("Failed to create embedder");

        assert_eq!(embedder.dimension(), 384);

        let embedding = embedder
            .embed("hello world")
            .await
            .expect("Failed to embed");

        assert_eq!(embedding.len(), 384);

        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01);
    }

    #[tokio::test]
    #[ignore]
    async fn test_batch_embedding() {
        let embedder = SentenceEmbedder::new("sentence-transformers/all-MiniLM-L6-v2", None)
            .expect("Failed to create embedder");

        let texts = vec!["hello world", "foo bar", "test string"];
        let embeddings = embedder
            .embed_batch_sync(&texts)
            .expect("Failed to embed batch");

        assert_eq!(embeddings.len(), 3);
        for emb in embeddings {
            assert_eq!(emb.len(), 384);
        }
    }
}
