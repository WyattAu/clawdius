use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub cors_origins: Vec<String>,
    pub rate_limit: Option<RateLimitConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub requests_per_minute: u32,
    pub burst: u32,
}

pub struct ApiGateway {
    config: ApiConfig,
}

impl ApiGateway {
    #[must_use]
    pub fn new(config: ApiConfig) -> Self {
        Self { config }
    }

    pub async fn start(&self) -> Result<()> {
        // Start HTTP server
        Ok(())
    }

    #[must_use]
    pub fn config(&self) -> &ApiConfig {
        &self.config
    }
}
