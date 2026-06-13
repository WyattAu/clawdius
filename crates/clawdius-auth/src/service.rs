use crate::config::{AuthConfig, OidcProviderConfig};
use crate::user::{SessionClaims, UserInfo};
use anyhow::{Context, Result};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use parking_lot::RwLock;
use std::collections::HashMap;

pub struct AuthService {
    config: AuthConfig,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    pkce_verifiers: RwLock<HashMap<String, String>>,
}

pub struct TokenResult {
    pub access_token: String,
    pub session_token: String,
    pub refresh_token: Option<String>,
    pub user: UserInfo,
}

impl AuthService {
    pub fn new(config: AuthConfig) -> Result<Self> {
        let encoding_key = EncodingKey::from_secret(config.jwt_secret.as_bytes());
        let decoding_key = DecodingKey::from_secret(config.jwt_secret.as_bytes());

        Ok(Self {
            config,
            encoding_key,
            decoding_key,
            pkce_verifiers: RwLock::new(HashMap::new()),
        })
    }

    pub fn authorization_url(&self, provider_name: &str) -> Result<(String, String, String)> {
        let provider = self.get_provider(provider_name)?;

        let state = uuid::Uuid::new_v4().to_string();
        let verifier = Self::generate_pkce_verifier();

        self.pkce_verifiers
            .write()
            .insert(state.clone(), verifier.clone());

        let auth_url = format!(
            "{}/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            provider.issuer_url,
            provider.client_id,
            urlencoding::encode(&provider.redirect_url),
            urlencoding::encode(&provider.scopes.join(" ")),
            state,
        );

        Ok((auth_url, state, verifier))
    }

    pub async fn exchange_code(
        &self,
        provider_name: &str,
        code: &str,
        state: &str,
    ) -> Result<TokenResult> {
        let provider = self.get_provider(provider_name)?;

        let _verifier = self
            .pkce_verifiers
            .write()
            .remove(state)
            .with_context(|| "No PKCE verifier found for state")?;

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/token", provider.issuer_url))
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", provider.redirect_url.as_str()),
                ("client_id", provider.client_id.as_str()),
                ("client_secret", provider.client_secret.as_str()),
            ])
            .send()
            .await
            .context("Token exchange request failed")?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Token exchange failed: {body}");
        }

        let token_response: serde_json::Value = response.json().await?;

        let access_token = token_response["access_token"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let id_token = token_response["id_token"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let refresh_token = token_response["refresh_token"].as_str().map(String::from);

        let user_info = self.parse_id_token(&id_token, provider_name)?;

        let session_token = self.create_session_token(&user_info)?;
        let refresh = refresh_token.or_else(|| Some(self.create_refresh_token(&user_info)));

        Ok(TokenResult {
            access_token,
            session_token,
            refresh_token: refresh,
            user: user_info,
        })
    }

    pub fn validate_session(&self, token: &str) -> Result<SessionClaims> {
        let token_data = decode::<SessionClaims>(token, &self.decoding_key, &Validation::default())
            .context("Invalid session token")?;

        Ok(token_data.claims)
    }

    pub fn refresh_session(&self, token: &str) -> Result<String> {
        let claims = self.validate_session(token)?;
        let now = chrono::Utc::now().timestamp() as u64;

        let new_claims = SessionClaims {
            iat: now,
            exp: now + self.config.session_duration_secs,
            jti: uuid::Uuid::new_v4().to_string(),
            ..claims
        };

        let token = encode(&Header::default(), &new_claims, &self.encoding_key)?;
        Ok(token)
    }

    fn get_provider(&self, name: &str) -> Result<&OidcProviderConfig> {
        self.config
            .providers
            .iter()
            .find(|p| p.name == name && p.enabled)
            .with_context(|| format!("OIDC provider '{name}' not found or disabled"))
    }

    fn create_session_token(&self, user: &UserInfo) -> Result<String> {
        let now = chrono::Utc::now().timestamp() as u64;
        let claims = SessionClaims {
            sub: user.sub.clone(),
            email: user.email.clone(),
            name: user.name.clone(),
            provider: user.provider.clone(),
            roles: user.groups.clone(),
            iat: now,
            exp: now + self.config.session_duration_secs,
            jti: uuid::Uuid::new_v4().to_string(),
        };

        Ok(encode(&Header::default(), &claims, &self.encoding_key)?)
    }

    fn create_refresh_token(&self, user: &UserInfo) -> String {
        let now = chrono::Utc::now().timestamp() as u64;
        let claims = SessionClaims {
            sub: user.sub.clone(),
            email: user.email.clone(),
            name: user.name.clone(),
            provider: user.provider.clone(),
            roles: user.groups.clone(),
            iat: now,
            exp: now + self.config.refresh_duration_secs,
            jti: uuid::Uuid::new_v4().to_string(),
        };

        encode(&Header::default(), &claims, &self.encoding_key).unwrap_or_default()
    }

    fn parse_id_token(&self, _id_token: &str, provider: &str) -> Result<UserInfo> {
        Ok(UserInfo {
            sub: uuid::Uuid::new_v4().to_string(),
            email: None,
            name: None,
            given_name: None,
            family_name: None,
            picture: None,
            provider: provider.to_string(),
            groups: vec![],
        })
    }

    fn generate_pkce_verifier() -> String {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes).unwrap_or_default();
        URL_SAFE_NO_PAD.encode(bytes)
    }
}
