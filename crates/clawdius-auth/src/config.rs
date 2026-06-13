use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcProviderConfig {
    pub name: String,
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    #[serde(default = "default_scopes")]
    pub scopes: Vec<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub providers: Vec<OidcProviderConfig>,
    pub jwt_secret: String,
    #[serde(default = "default_session_duration")]
    pub session_duration_secs: u64,
    #[serde(default = "default_refresh_duration")]
    pub refresh_duration_secs: u64,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
}

fn default_session_duration() -> u64 {
    3600
}

fn default_refresh_duration() -> u64 {
    604_800
}

impl OidcProviderConfig {
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
