use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcConfig {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
}

pub struct OidcClient {
    config: OidcConfig,
}

impl OidcClient {
    pub fn new(config: OidcConfig) -> Self {
        Self { config }
    }

    pub fn get_authorization_url(&self, state: &str) -> String {
        format!(
            "{}/authorize?client_id={}&redirect_uri={}&response_type=code&state={}",
            self.config.issuer,
            self.config.client_id,
            urlencoding::encode(&self.config.redirect_uri),
            state
        )
    }

    pub async fn exchange_code(&self, code: &str) -> Result<TokenResponse> {
        // Exchange authorization code for token
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/token", self.config.issuer))
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", &self.config.redirect_uri),
                ("client_id", &self.config.client_id),
            ])
            .send()
            .await?;

        Ok(response.json().await?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<i64>,
    pub refresh_token: Option<String>,
}
