//! OAuth flows for subscription-plan providers (Anthropic Claude
//! Pro/Max, OpenAI ChatGPT subscriptions). Design:
//! `docs/provider-oauth-design.md` (in the clawdius repo).
//!
//! Phase 2 groundwork: PKCE, authorize URL builders, token exchange, and
//! token-set extraction. The interactive browser step is the caller's
//! job — desktop opens a browser; headless prints the URL and reads the
//! pasted authorization code.

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::config::OAuthTokenSet;

/// Provider-specific OAuth endpoints and client parameters.
#[derive(Debug, Clone)]
pub struct OAuthEndpoints {
    /// Authorization endpoint (user consents here).
    pub authorize_url: &'static str,
    /// Token exchange endpoint.
    pub token_url: &'static str,
    /// OAuth client_id registered for CLI-class tools.
    pub client_id: &'static str,
    /// Scopes requested.
    pub scopes: &'static str,
    /// Where to redirect after consent (loopback capture).
    pub redirect_port: u16,
}

/// Anthropic Claude subscription flow (Claude Code-compatible endpoints).
#[must_use]
pub fn anthropic_endpoints() -> OAuthEndpoints {
    OAuthEndpoints {
        authorize_url: "https://claude.ai/oauth/authorize",
        token_url: "https://console.anthropic.com/v1/oauth/token",
        client_id: "9d1c250a-e61b-44d6-9723-64f5e60d698b",
        scopes: "org:create_api_key user:profile user:inference",
        redirect_port: 54545,
    }
}

/// OpenAI ChatGPT subscription flow (Codex CLI-compatible endpoints).
#[must_use]
pub fn openai_endpoints() -> OAuthEndpoints {
    OAuthEndpoints {
        authorize_url: "https://auth.openai.com/oauth/authorize",
        token_url: "https://auth.openai.com/oauth/token",
        client_id: "app_EMoamEEZ73f0CkXaXp7hrann",
        scopes: "openid profile email offline_access",
        redirect_port: 1455,
    }
}

/// PKCE verifier plus the derived S256 challenge (RFC 7636).
#[derive(Debug, Clone)]
pub struct Pkce {
    /// High-entropy verifier sent on the token exchange.
    pub verifier: String,
    /// BASE64URL(SHA256(verifier)) sent on the authorize request.
    pub challenge: String,
}

impl Pkce {
    /// Generate a verifier from OS entropy and derive the S256 challenge.
    ///
    /// # Errors
    ///
    /// Errors when the OS entropy source fails.
    pub fn generate() -> crate::Result<Self> {
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes)
            .map_err(|e| crate::Error::Config(format!("OS entropy failed: {e}")))?;
        let verifier = base64_url_safe(&bytes);
        let challenge = base64_url_safe(&Sha256::digest(verifier.as_bytes()));
        Ok(Self {
            verifier,
            challenge,
        })
    }
}

/// URL-safe base64 without padding (RFC 7636 appendix A).
#[must_use]
pub fn base64_url_safe(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

/// The consent URL the user must visit.
#[must_use]
pub fn authorize_url(endpoints: &OAuthEndpoints, pkce: &Pkce, redirect_port: u16) -> String {
    format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256",
        endpoints.authorize_url,
        endpoints.client_id,
        redirect_uri(redirect_port),
        urlencode(endpoints.scopes),
        pkce.challenge,
    )
}

/// The loopback redirect URI for a port.
#[must_use]
pub fn redirect_uri(port: u16) -> String {
    format!("http://localhost:{port}/callback")
}

/// Exchange an authorization code for a provider token response
/// (`access_token` / `refresh_token` / `expires_in` naming).
///
/// # Errors
///
/// Transport and provider errors surface as strings.
pub async fn exchange_code(
    endpoints: &OAuthEndpoints,
    code: &str,
    verifier: &str,
    redirect_port: u16,
) -> std::result::Result<OAuthTokenSet, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let redirect = redirect_uri(redirect_port);
    let form = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect.as_str()),
        ("client_id", endpoints.client_id),
        ("code_verifier", verifier),
    ];
    let response = client
        .post(endpoints.token_url)
        .form(&form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = response.status();
    let body: Value = response
        .json()
        .await
        .map_err(|e| format!("invalid token response: {e}"))?;
    if !status.is_success() {
        return Err(format!("token exchange failed: HTTP {status}: {body}",));
    }
    token_set_from_response(&body).ok_or_else(|| "token response missing access_token".to_string())
}

/// Refresh an expired access token using a refresh token
/// (`grant_type=refresh_token`). Returns the updated token set.
///
/// # Errors
///
/// Transport and provider errors surface as strings.
pub async fn refresh_token(
    endpoints: &OAuthEndpoints,
    refresh_token: &str,
) -> std::result::Result<OAuthTokenSet, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let form = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", endpoints.client_id),
    ];
    let response = client
        .post(endpoints.token_url)
        .form(&form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = response.status();
    let body: Value = response
        .json()
        .await
        .map_err(|e| format!("invalid refresh response: {e}"))?;
    if !status.is_success() {
        return Err(format!("refresh failed: HTTP {status}: {body}"));
    }
    token_set_from_response(&body)
        .ok_or_else(|| "refresh response missing access_token".to_string())
}

/// Extract an [`OAuthTokenSet`] from a provider token response.
#[must_use]
pub fn token_set_from_response(response: &Value) -> Option<OAuthTokenSet> {
    let access = response.get("access_token")?.as_str()?.to_string();
    let expires_in = response.get("expires_in").and_then(Value::as_i64);
    let expires_at =
        expires_in.map(|secs| (chrono::Utc::now() + chrono::Duration::seconds(secs)).to_rfc3339());
    Some(OAuthTokenSet {
        access_token: access,
        refresh_token: response
            .get("refresh_token")
            .and_then(Value::as_str)
            .map(String::from),
        expires_at,
        updated_at: Some(chrono::Utc::now().to_rfc3339()),
    })
}

fn urlencode(s: &str) -> String {
    s.replace(' ', "%20")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_sha256_of_verifier() {
        let pkce = Pkce::generate().unwrap();
        // RFC 7636 appendix B vectors: S256(code_verifier) == code_challenge
        let expected = base64_url_safe(&Sha256::digest(pkce.verifier.as_bytes()));
        assert_eq!(pkce.challenge, expected);
        assert!(pkce.verifier.len() >= 43, "verifier must be >= 43 chars");
    }

    #[test]
    fn authorize_url_carries_pkce_params() {
        let endpoints = anthropic_endpoints();
        let pkce = Pkce::generate().unwrap();
        let url = authorize_url(&endpoints, &pkce, endpoints.redirect_port);
        assert!(url.starts_with(endpoints.authorize_url));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains(&format!("client_id={}", endpoints.client_id)));
        assert!(url.contains("scope=org:create_api_key"));
    }

    #[test]
    fn token_set_extraction_maps_provider_fields() {
        let response = serde_json::json!({
            "access_token": "at-123",
            "refresh_token": "rt-456",
            "expires_in": 3600
        });
        let set = token_set_from_response(&response).unwrap();
        assert_eq!(set.access_token, "at-123");
        assert_eq!(set.refresh_token.as_deref(), Some("rt-456"));
        assert!(set.expires_at.is_some());
        assert!(!set.expired());
    }

    #[test]
    fn token_set_expiry_detection() {
        let past = chrono::Utc::now() - chrono::Duration::hours(1);
        let set = OAuthTokenSet {
            access_token: "x".into(),
            refresh_token: None,
            expires_at: Some(past.to_rfc3339()),
            updated_at: None,
        };
        assert!(set.expired());
    }
}
