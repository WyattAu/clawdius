//! SSO (Single Sign-On) Integration
//!
//! Supports SAML 2.0 and OIDC providers for enterprise authentication.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// SSO configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SSOConfig {
    /// Enabled SSO providers
    pub providers: Vec<SSOProvider>,
    /// Default provider (if multiple)
    pub default_provider: Option<String>,
    /// Session timeout in seconds
    pub session_timeout_secs: u64,
    /// Require MFA
    pub require_mfa: bool,
    /// Allowed domains (email domain restrictions)
    pub allowed_domains: Vec<String>,
}

impl Default for SSOConfig {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            default_provider: None,
            session_timeout_secs: 3600 * 8, // 8 hours
            require_mfa: false,
            allowed_domains: Vec::new(),
        }
    }
}

/// SSO Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SSOProvider {
    /// SAML 2.0 provider
    Saml(SAMLConfig),
    /// OIDC/OAuth2 provider
    Oidc(OAuthProvider),
}

/// SAML 2.0 configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SAMLConfig {
    /// Provider name
    pub name: String,
    /// Identity Provider (`IdP`) metadata URL
    pub idp_metadata_url: Option<String>,
    /// `IdP` entity ID
    pub idp_entity_id: String,
    /// `IdP` SSO URL
    pub idp_sso_url: String,
    /// `IdP` SLO (Logout) URL
    pub idp_slo_url: Option<String>,
    /// `IdP` X509 certificate (PEM format)
    pub idp_certificate: String,
    /// Service Provider entity ID
    pub sp_entity_id: String,
    /// Service Provider ACS URL
    pub sp_acs_url: String,
    /// Service Provider SLS URL
    pub sp_sls_url: Option<String>,
    /// Attribute mappings
    pub attribute_mappings: HashMap<String, String>,
    /// Want assertions signed
    pub want_assertions_signed: bool,
    /// Want responses signed
    pub want_responses_signed: bool,
    /// Signature algorithm
    pub signature_algorithm: String,
}

impl SAMLConfig {
    /// Create a new SAML configuration
    pub fn new(name: impl Into<String>, idp_entity_id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            idp_metadata_url: None,
            idp_entity_id: idp_entity_id.into(),
            idp_sso_url: String::new(),
            idp_slo_url: None,
            idp_certificate: String::new(),
            sp_entity_id: String::new(),
            sp_acs_url: String::new(),
            sp_sls_url: None,
            attribute_mappings: HashMap::new(),
            want_assertions_signed: true,
            want_responses_signed: true,
            signature_algorithm: "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256".to_string(),
        }
    }

    /// Set `IdP` SSO URL
    pub fn with_sso_url(mut self, url: impl Into<String>) -> Self {
        self.idp_sso_url = url.into();
        self
    }

    /// Set `IdP` certificate
    pub fn with_certificate(mut self, cert: impl Into<String>) -> Self {
        self.idp_certificate = cert.into();
        self
    }

    /// Set SP entity ID
    pub fn with_sp_entity_id(mut self, id: impl Into<String>) -> Self {
        self.sp_entity_id = id.into();
        self
    }

    /// Set SP ACS URL
    pub fn with_acs_url(mut self, url: impl Into<String>) -> Self {
        self.sp_acs_url = url.into();
        self
    }

    /// Map an attribute
    pub fn map_attribute(
        mut self,
        saml_attr: impl Into<String>,
        local_attr: impl Into<String>,
    ) -> Self {
        self.attribute_mappings
            .insert(saml_attr.into(), local_attr.into());
        self
    }

    /// Okta SAML configuration helper
    pub fn okta(domain: impl Into<String>, app_id: impl Into<String>) -> Self {
        let domain = domain.into();
        let app_id = app_id.into();
        Self::new("Okta", format!("http://www.okta.com/{app_id}"))
            .with_sso_url(format!("https://{domain}/app/{app_id}/sso/saml"))
            .map_attribute("email", "email")
            .map_attribute("firstName", "first_name")
            .map_attribute("lastName", "last_name")
            .map_attribute("groups", "groups")
    }

    /// Azure AD SAML configuration helper
    pub fn azure_ad(tenant_id: impl Into<String>, app_id: impl Into<String>) -> Self {
        let tenant_id = tenant_id.into();
        let _app_id = app_id.into();
        Self::new("Azure AD", format!("https://sts.windows.net/{tenant_id}/"))
            .with_sso_url(format!(
                "https://login.microsoftonline.com/{tenant_id}/saml2?SAMLRequest="
            ))
            .map_attribute(
                "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress",
                "email",
            )
            .map_attribute(
                "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/name",
                "name",
            )
            .map_attribute(
                "http://schemas.microsoft.com/ws/2008/06/identity/claims/groups",
                "groups",
            )
    }
}

/// OIDC/OAuth2 provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProvider {
    /// Provider name
    pub name: String,
    /// Client ID
    pub client_id: String,
    /// Client secret
    pub client_secret: String,
    /// Authorization URL
    pub auth_url: String,
    /// Token URL
    pub token_url: String,
    /// `UserInfo` URL
    pub userinfo_url: Option<String>,
    /// JWKS URL for token verification
    pub jwks_url: Option<String>,
    /// Issuer URL
    pub issuer: String,
    /// Scopes to request
    pub scopes: Vec<String>,
    /// Redirect URI
    pub redirect_uri: String,
    /// PKCE enabled
    pub pkce_enabled: bool,
    /// Additional parameters
    pub additional_params: HashMap<String, String>,
    /// Claim mappings
    pub claim_mappings: HashMap<String, String>,
}

impl OAuthProvider {
    /// Create a new OAuth provider
    pub fn new(
        name: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            auth_url: String::new(),
            token_url: String::new(),
            userinfo_url: None,
            jwks_url: None,
            issuer: String::new(),
            scopes: vec![
                "openid".to_string(),
                "email".to_string(),
                "profile".to_string(),
            ],
            redirect_uri: String::new(),
            pkce_enabled: true,
            additional_params: HashMap::new(),
            claim_mappings: HashMap::new(),
        }
    }

    /// Set endpoints
    pub fn with_endpoints(
        mut self,
        auth_url: impl Into<String>,
        token_url: impl Into<String>,
        issuer: impl Into<String>,
    ) -> Self {
        self.auth_url = auth_url.into();
        self.token_url = token_url.into();
        self.issuer = issuer.into();
        self
    }

    /// Set redirect URI
    pub fn with_redirect_uri(mut self, uri: impl Into<String>) -> Self {
        self.redirect_uri = uri.into();
        self
    }

    /// Add scope
    pub fn with_scope(mut self, scope: impl Into<String>) -> Self {
        self.scopes.push(scope.into());
        self
    }

    /// Map a claim
    pub fn map_claim(
        mut self,
        jwt_claim: impl Into<String>,
        local_attr: impl Into<String>,
    ) -> Self {
        self.claim_mappings
            .insert(jwt_claim.into(), local_attr.into());
        self
    }

    /// Google OAuth configuration
    pub fn google(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self::new("Google", client_id, client_secret)
            .with_endpoints(
                "https://accounts.google.com/o/oauth2/v2/auth",
                "https://oauth2.googleapis.com/token",
                "https://accounts.google.com",
            )
            .with_scope("https://www.googleapis.com/auth/userinfo.email")
            .with_scope("https://www.googleapis.com/auth/userinfo.profile")
            .map_claim("email", "email")
            .map_claim("name", "name")
            .map_claim("picture", "avatar_url")
    }

    /// GitHub OAuth configuration
    pub fn github(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self::new("GitHub", client_id, client_secret)
            .with_endpoints(
                "https://github.com/login/oauth/authorize",
                "https://github.com/login/oauth/access_token",
                "https://github.com",
            )
            .map_claim("login", "username")
            .map_claim("email", "email")
            .map_claim("name", "name")
            .map_claim("avatar_url", "avatar_url")
    }

    /// Okta OIDC configuration
    pub fn okta_oidc(
        domain: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self {
        let domain = domain.into();
        Self::new("Okta", client_id, client_secret)
            .with_endpoints(
                format!("https://{domain}/oauth2/v1/authorize"),
                format!("https://{domain}/oauth2/v1/token"),
                format!("https://{domain}"),
            )
            .map_claim("email", "email")
            .map_claim("name", "name")
            .map_claim("groups", "groups")
    }

    /// Azure AD OIDC configuration
    pub fn azure_ad_oidc(
        tenant_id: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self {
        let tenant_id = tenant_id.into();
        Self::new("Azure AD", client_id, client_secret)
            .with_endpoints(
                format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/authorize"),
                format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token"),
                format!("https://login.microsoftonline.com/{tenant_id}/v2.0"),
            )
            .with_scope("https://graph.microsoft.com/.default")
            .map_claim("email", "email")
            .map_claim("name", "name")
            .map_claim("groups", "groups")
    }
}

/// SSO authenticated user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SSOUser {
    /// Unique user ID from `IdP`
    pub id: String,
    /// Email address
    pub email: String,
    /// Display name
    pub name: Option<String>,
    /// Username
    pub username: Option<String>,
    /// Avatar URL
    pub avatar_url: Option<String>,
    /// Groups/roles from `IdP`
    pub groups: Vec<String>,
    /// Custom attributes
    pub attributes: HashMap<String, String>,
    /// Provider name
    pub provider: String,
    /// Authentication time
    pub authenticated_at: chrono::DateTime<chrono::Utc>,
    /// Session expiry
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

impl SSOUser {
    /// Check if session is expired
    #[must_use]
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now() > self.expires_at
    }

    /// Check if user is in a group
    #[must_use]
    pub fn in_group(&self, group: &str) -> bool {
        self.groups.iter().any(|g| g == group)
    }
}

/// SSO manager
pub struct SSOManager {
    config: SSOConfig,
    sessions: HashMap<String, SSOUser>,
}

impl SSOManager {
    /// Create a new SSO manager
    #[must_use]
    pub fn new(config: SSOConfig) -> Self {
        Self {
            config,
            sessions: HashMap::new(),
        }
    }

    /// Get SSO configuration
    #[must_use]
    pub fn config(&self) -> &SSOConfig {
        &self.config
    }

    /// Get a provider by name
    #[must_use]
    pub fn get_provider(&self, name: &str) -> Option<&SSOProvider> {
        self.config.providers.iter().find(|p| match p {
            SSOProvider::Saml(c) => c.name == name,
            SSOProvider::Oidc(c) => c.name == name,
        })
    }

    /// List available providers
    #[must_use]
    pub fn list_providers(&self) -> Vec<&str> {
        self.config
            .providers
            .iter()
            .map(|p| match p {
                SSOProvider::Saml(c) => c.name.as_str(),
                SSOProvider::Oidc(c) => c.name.as_str(),
            })
            .collect()
    }

    /// Validate user session
    #[must_use]
    pub fn validate_session(&self, session_id: &str) -> Option<&SSOUser> {
        self.sessions.get(session_id).filter(|u| !u.is_expired())
    }

    /// Create a session for a user
    pub fn create_session(&mut self, user: SSOUser) -> String {
        let session_id = uuid::Uuid::new_v4().to_string();
        self.sessions.insert(session_id.clone(), user);
        session_id
    }

    /// Invalidate a session
    pub fn invalidate_session(&mut self, session_id: &str) {
        self.sessions.remove(session_id);
    }

    /// Cleanup expired sessions
    pub fn cleanup_expired(&mut self) {
        self.sessions.retain(|_, u| !u.is_expired());
    }

    /// Check if email domain is allowed
    #[must_use]
    pub fn is_domain_allowed(&self, email: &str) -> bool {
        if self.config.allowed_domains.is_empty() {
            return true;
        }
        let domain = email.split('@').next_back().unwrap_or("");
        self.config.allowed_domains.iter().any(|d| d == domain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saml_okta_config() {
        let config = SAMLConfig::okta("example.okta.com", "app123");
        assert_eq!(config.name, "Okta");
        assert!(config.idp_sso_url.contains("example.okta.com"));
    }

    #[test]
    fn test_oauth_google_config() {
        let config = OAuthProvider::google("client123", "secret123");
        assert_eq!(config.name, "Google");
        assert!(config.auth_url.contains("google"));
    }

    #[test]
    fn test_sso_user_expiry() {
        let user = SSOUser {
            id: "123".to_string(),
            email: "test@example.com".to_string(),
            name: None,
            username: None,
            avatar_url: None,
            groups: vec![],
            attributes: HashMap::new(),
            provider: "test".to_string(),
            authenticated_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() - chrono::Duration::seconds(1),
        };
        assert!(user.is_expired());
    }

    #[test]
    fn test_sso_user_in_group() {
        let user = SSOUser {
            id: "123".to_string(),
            email: "test@example.com".to_string(),
            name: None,
            username: None,
            avatar_url: None,
            groups: vec!["admin".to_string(), "developer".to_string()],
            attributes: HashMap::new(),
            provider: "test".to_string(),
            authenticated_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        };
        assert!(user.in_group("admin"));
        assert!(!user.in_group("guest"));
    }
}
