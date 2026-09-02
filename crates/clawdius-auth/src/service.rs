use crate::config::{AuthConfig, OidcProviderConfig};
use crate::user::{SessionClaims, UserInfo};
use anyhow::{Context, Result};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use parking_lot::RwLock;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokenkit::service::{JwtAlgorithm, JwtConfig, JwtService};

/// Cached OIDC discovery document.
#[derive(Debug, Clone)]
struct OidcDiscovery {
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
    issuer: String,
}

/// Cached JWKS keys.
#[derive(Debug, Clone)]
struct JwksCache {
    keys: Vec<jsonwebtoken::jwk::Jwk>,
    fetched_at: std::time::Instant,
}

/// Stateful service that orchestrates OIDC authorization flows and JWT sessions.
pub struct AuthService {
    config: AuthConfig,
    jwt_service: JwtService,
    pkce_verifiers: RwLock<HashMap<String, String>>,
    /// Maps state param -> provider name for callback routing.
    state_providers: RwLock<HashMap<String, String>>,
    discovery_cache: RwLock<HashMap<String, OidcDiscovery>>,
    jwks_cache: RwLock<HashMap<String, JwksCache>>,
    http_client: reqwest::Client,
    /// Revoked session JTIs with their expiration times.
    revoked_sessions: RwLock<HashMap<String, std::time::Instant>>,
}

/// Tokens issued to a client after a successful authorization code exchange.
pub struct TokenResult {
    /// Access token from the identity provider.
    pub access_token: String,
    /// Signed session JWT consumed by protected endpoints.
    pub session_token: String,
    /// Refresh token used to obtain a new session token, when issued.
    pub refresh_token: Option<String>,
    /// Identity information for the authenticated user.
    pub user: UserInfo,
}

impl AuthService {
    /// Create a new service from the provided configuration.
    pub fn new(config: AuthConfig) -> Result<Self> {
        let jwt_config = JwtConfig {
            algorithm: JwtAlgorithm::HS256,
            secret: zeroize::Zeroizing::new(config.jwt_secret.clone()),
            issuer: Some("clawdius".to_string()),
            audience: None,
            access_token_ttl: config.session_duration_secs as i64,
            refresh_token_ttl: config.refresh_duration_secs as i64,
        };

        let jwt_service = JwtService::new(jwt_config);

        Ok(Self {
            config,
            jwt_service,
            pkce_verifiers: RwLock::new(HashMap::new()),
            state_providers: RwLock::new(HashMap::new()),
            discovery_cache: RwLock::new(HashMap::new()),
            jwks_cache: RwLock::new(HashMap::new()),
            http_client: reqwest::Client::new(),
            revoked_sessions: RwLock::new(HashMap::new()),
        })
    }

    /// Build the provider authorization URL and return it with the state and PKCE verifier.
    pub fn authorization_url(&self, provider_name: &str) -> Result<(String, String, String)> {
        let provider = self.get_provider(provider_name)?;

        let state = uuid::Uuid::new_v4().to_string();
        let verifier = Self::generate_pkce_verifier();

        self.pkce_verifiers
            .write()
            .insert(state.clone(), verifier.clone());
        self.state_providers
            .write()
            .insert(state.clone(), provider_name.to_string());

        let auth_url = format!(
            "{}/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
            provider.issuer_url,
            provider.client_id,
            urlencoding::encode(&provider.redirect_url),
            urlencoding::encode(&provider.scopes.join(" ")),
            state,
            Self::pkce_challenge(&verifier),
        );

        Ok((auth_url, state, verifier))
    }

    /// Exchange an authorization code for access, session, and refresh tokens.
    pub async fn exchange_code(
        &self,
        provider_name: &str,
        code: &str,
        state: &str,
    ) -> Result<TokenResult> {
        let provider = self.get_provider(provider_name)?;

        let verifier = self
            .pkce_verifiers
            .write()
            .remove(state)
            .with_context(|| "No PKCE verifier found for state")?;

        let discovery = self.discover(&provider).await?;

        let client = &self.http_client;
        let response = client
            .post(&discovery.token_endpoint)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", provider.redirect_url.as_str()),
                ("client_id", provider.client_id.as_str()),
                ("client_secret", provider.client_secret.as_str()),
                ("code_verifier", &verifier),
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

        let user_info = self.validate_id_token(&id_token, &provider, &discovery).await?;

        let session_token = self.create_session_token(&user_info)?;
        let refresh = refresh_token.or_else(|| Some(self.create_refresh_token(&user_info)));

        Ok(TokenResult {
            access_token,
            session_token,
            refresh_token: refresh,
            user: user_info,
        })
    }

    /// Decode and validate a session token, returning its claims.
    pub fn validate_session(&self, token: &str) -> Result<SessionClaims> {
        let claims: SessionClaims = self
            .jwt_service
            .decode(token)
            .context("Invalid session token")?;

        // Check if the session has been revoked
        if self.is_revoked(&claims.jti) {
            anyhow::bail!("Session has been revoked");
        }

        Ok(claims)
    }

    /// Issue a new session token with an extended expiration from an existing token.
    pub fn refresh_session(&self, token: &str) -> Result<String> {
        let claims = self.validate_session(token)?;
        let now = chrono::Utc::now().timestamp() as u64;

        let new_claims = SessionClaims {
            iat: now,
            exp: now + self.config.session_duration_secs,
            jti: uuid::Uuid::new_v4().to_string(),
            iss: Some("clawdius".to_string()),
            ..claims
        };

        let token = self.jwt_service.encode(&new_claims)?;
        Ok(token)
    }

    /// Invalidate a session (logout) by adding its JTI to the revocation list.
    pub fn invalidate_session(&self, jti: &str) -> Result<()> {
        self.revoked_sessions.write().insert(
            jti.to_string(),
            std::time::Instant::now() + std::time::Duration::from_secs(3600),
        );
        tracing::info!("Session {} revoked", jti);
        Ok(())
    }

    /// Check if a session JTI has been revoked.
    fn is_revoked(&self, jti: &str) -> bool {
        let mut revoked = self.revoked_sessions.write();
        // Clean up expired entries while checking
        let now = std::time::Instant::now();
        revoked.retain(|_, expiry| *expiry > now);
        revoked.contains_key(jti)
    }

    fn get_provider(&self, name: &str) -> Result<&OidcProviderConfig> {
        self.config
            .providers
            .iter()
            .find(|p| p.name == name && p.enabled)
            .with_context(|| format!("OIDC provider '{name}' not found or disabled"))
    }

    /// Perform OIDC discovery to discover endpoints.
    async fn discover(&self, provider: &OidcProviderConfig) -> Result<OidcDiscovery> {
        {
            let cache = self.discovery_cache.read();
            if let Some(cached) = cache.get(&provider.issuer_url) {
                return Ok(cached.clone());
            }
        }

        let discovery_url = format!(
            "{}/.well-known/openid-configuration",
            provider.issuer_url.trim_end_matches('/')
        );

        let resp = self
            .http_client
            .get(&discovery_url)
            .send()
            .await
            .context("OIDC discovery request failed")?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "OIDC discovery failed ({}): {}",
                resp.status(),
                discovery_url
            );
        }

        #[derive(Deserialize)]
        struct DiscoveryDoc {
            authorization_endpoint: String,
            token_endpoint: String,
            jwks_uri: String,
            issuer: String,
        }

        let doc: DiscoveryDoc = resp.json().await.context("Failed to parse OIDC discovery")?;

        let discovery = OidcDiscovery {
            authorization_endpoint: doc.authorization_endpoint,
            token_endpoint: doc.token_endpoint,
            jwks_uri: doc.jwks_uri,
            issuer: doc.issuer,
        };

        self.discovery_cache
            .write()
            .insert(provider.issuer_url.clone(), discovery.clone());

        Ok(discovery)
    }

    /// Fetch JWKS keys from the provider, with caching.
    async fn fetch_jwks(&self, jwks_uri: &str) -> Result<Vec<jsonwebtoken::jwk::Jwk>> {
        {
            let cache = self.jwks_cache.read();
            if let Some(cached) = cache.get(jwks_uri) {
                // Cache for 1 hour
                if cached.fetched_at.elapsed() < std::time::Duration::from_secs(3600) {
                    return Ok(cached.keys.clone());
                }
            }
        }

        let resp = self
            .http_client
            .get(jwks_uri)
            .send()
            .await
            .context("JWKS fetch failed")?;

        if !resp.status().is_success() {
            anyhow::bail!("JWKS fetch failed: {}", resp.status());
        }

        #[derive(Deserialize)]
        struct JwksResponse {
            keys: Vec<jsonwebtoken::jwk::Jwk>,
        }

        let jwks: JwksResponse = resp.json().await.context("Failed to parse JWKS")?;

        self.jwks_cache.write().insert(
            jwks_uri.to_string(),
            JwksCache {
                keys: jwks.keys.clone(),
                fetched_at: std::time::Instant::now(),
            },
        );

        Ok(jwks.keys)
    }

    /// Validate an OIDC id_token using JWKS signature verification.
    async fn validate_id_token(
        &self,
        id_token: &str,
        provider: &OidcProviderConfig,
        discovery: &OidcDiscovery,
    ) -> Result<UserInfo> {
        if id_token.is_empty() {
            anyhow::bail!("No id_token received from provider");
        }

        // Decode header to get kid
        let header = decode_header(id_token).context("Failed to decode JWT header")?;

        let kid = header.kid.as_deref().unwrap_or("");

        // Fetch JWKS and find the matching key
        let keys = self.fetch_jwks(&discovery.jwks_uri).await?;

        let jwk = keys
            .iter()
            .find(|k| {
                if let Some(ref kid_match) = k.common.key_id {
                    kid_match == kid
                } else {
                    // If no kid in header and only one key, use it
                    keys.len() == 1
                }
            })
            .context(format!("No matching JWK found for kid='{kid}'"))?;

        // Build decoding key from JWK
        let decoding_key = DecodingKey::from_jwk(jwk).context("Failed to build DecodingKey from JWK")?;

        // Validate the token
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[&provider.client_id]);
        validation.set_issuer(&[&discovery.issuer]);
        validation.set_required_spec_claims(&["exp", "iss"]);

        #[derive(Deserialize)]
        struct IdTokenClaims {
            sub: String,
            #[serde(default)]
            email: Option<String>,
            #[serde(default)]
            name: Option<String>,
            #[serde(default)]
            given_name: Option<String>,
            #[serde(default)]
            family_name: Option<String>,
            #[serde(default)]
            picture: Option<String>,
            #[serde(default)]
            groups: Option<Vec<String>>,
        }

        let token_data = decode::<IdTokenClaims>(id_token, &decoding_key, &validation)
            .context("id_token validation failed")?;

        Ok(UserInfo {
            sub: token_data.claims.sub,
            email: token_data.claims.email,
            name: token_data.claims.name,
            given_name: token_data.claims.given_name,
            family_name: token_data.claims.family_name,
            picture: token_data.claims.picture,
            provider: provider.name.clone(),
            groups: token_data.claims.groups.unwrap_or_default(),
        })
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
            iss: Some("clawdius".to_string()),
        };

        Ok(self.jwt_service.encode(&claims)?)
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
            iss: Some("clawdius".to_string()),
        };

        self.jwt_service.encode(&claims).unwrap_or_default()
    }

    /// Look up the provider name associated with a state parameter.
    pub fn provider_for_state(&self, state: &str) -> Option<String> {
        self.state_providers.read().get(state).cloned()
    }

    /// Generate a PKCE code verifier (URL-safe base64, 32 bytes).
    fn generate_pkce_verifier() -> String {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes).unwrap_or_default();
        URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Compute PKCE code challenge (SHA-256 of verifier, base64url-encoded).
    fn pkce_challenge(verifier: &str) -> String {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(verifier.as_bytes());
        URL_SAFE_NO_PAD.encode(hash)
    }
}
