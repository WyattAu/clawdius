use serde::{Deserialize, Serialize};

/// Configuration for a single OpenID Connect identity provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcProviderConfig {
    /// Human-readable identifier used to select the provider in routes.
    pub name: String,
    /// Issuer URL used for discovery and token endpoints.
    pub issuer_url: String,
    /// OAuth/OIDC client ID registered with the provider.
    pub client_id: String,
    /// OAuth/OIDC client secret registered with the provider.
    pub client_secret: String,
    /// Redirect URL the provider returns to after authorization.
    pub redirect_url: String,
    /// OIDC scopes requested during authorization.
    #[serde(default = "default_scopes")]
    pub scopes: Vec<String>,
    /// Whether this provider is active and available for login.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "email".to_string(),
        "profile".to_string(),
    ]
}

fn default_true() -> bool {
    true
}

/// Top-level authentication configuration for the auth service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Registered OIDC identity providers.
    pub providers: Vec<OidcProviderConfig>,
    /// Secret used to sign and verify session JWTs.
    pub jwt_secret: String,
    /// Session token lifetime in seconds.
    #[serde(default = "default_session_duration")]
    pub session_duration_secs: u64,
    /// Refresh token lifetime in seconds.
    #[serde(default = "default_refresh_duration")]
    pub refresh_duration_secs: u64,
    /// Origins permitted for cross-origin authentication requests.
    #[serde(default)]
    pub allowed_origins: Vec<String>,
}

fn default_session_duration() -> u64 {
    3600
}

fn default_refresh_duration() -> u64 {
    604_800
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            jwt_secret: uuid::Uuid::new_v4().to_string(),
            session_duration_secs: default_session_duration(),
            refresh_duration_secs: default_refresh_duration(),
            allowed_origins: Vec::new(),
        }
    }
}

impl OidcProviderConfig {
    /// Build a configuration for an Okta tenant.
    #[must_use]
    pub fn okta(domain: &str, client_id: &str, client_secret: &str, redirect_url: &str) -> Self {
        Self {
            name: "Okta".to_string(),
            issuer_url: format!("https://{domain}"),
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            redirect_url: redirect_url.to_string(),
            scopes: default_scopes(),
            enabled: true,
        }
    }

    /// Build a configuration for an Azure Active Directory tenant.
    #[must_use]
    pub fn azure_ad(
        tenant_id: &str,
        client_id: &str,
        client_secret: &str,
        redirect_url: &str,
    ) -> Self {
        Self {
            name: "Azure AD".to_string(),
            issuer_url: format!("https://login.microsoftonline.com/{tenant_id}/v2.0"),
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            redirect_url: redirect_url.to_string(),
            scopes: default_scopes(),
            enabled: true,
        }
    }

    /// Build a configuration for GitHub as an OIDC provider.
    #[must_use]
    pub fn github(client_id: &str, client_secret: &str, redirect_url: &str) -> Self {
        Self {
            name: "GitHub".to_string(),
            issuer_url: "https://token.actions.githubusercontent.com".to_string(),
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            redirect_url: redirect_url.to_string(),
            scopes: vec![
                "openid".to_string(),
                "email".to_string(),
                "profile".to_string(),
                "read:user".to_string(),
            ],
            enabled: true,
        }
    }

    /// Build a configuration for a self-hosted GitLab instance.
    #[must_use]
    pub fn gitlab(domain: &str, client_id: &str, client_secret: &str, redirect_url: &str) -> Self {
        Self {
            name: "GitLab".to_string(),
            issuer_url: format!("https://{domain}/.well-known/openid-configuration"),
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            redirect_url: redirect_url.to_string(),
            scopes: default_scopes(),
            enabled: true,
        }
    }

    /// Build a configuration for Google as an OIDC provider.
    #[must_use]
    pub fn google(client_id: &str, client_secret: &str, redirect_url: &str) -> Self {
        Self {
            name: "Google".to_string(),
            issuer_url: "https://accounts.google.com".to_string(),
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            redirect_url: redirect_url.to_string(),
            scopes: vec![
                "openid".to_string(),
                "email".to_string(),
                "profile".to_string(),
            ],
            enabled: true,
        }
    }
}
