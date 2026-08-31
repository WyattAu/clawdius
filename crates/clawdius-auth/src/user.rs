use serde::{Deserialize, Serialize};

/// Identity information for an authenticated user, sourced from the OIDC provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// Subject identifier unique to the user at the identity provider.
    pub sub: String,
    /// Verified email address, if provided by the identity provider.
    pub email: Option<String>,
    /// Full display name, if provided by the identity provider.
    pub name: Option<String>,
    /// Given (first) name, if provided by the identity provider.
    pub given_name: Option<String>,
    /// Family (last) name, if provided by the identity provider.
    pub family_name: Option<String>,
    /// URL to the user's profile picture, if provided.
    pub picture: Option<String>,
    /// Name of the identity provider that authenticated this user.
    pub provider: String,
    /// Group memberships reported by the identity provider.
    #[serde(default)]
    pub groups: Vec<String>,
}

/// JWT claims carried by an authenticated session token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionClaims {
    /// Subject identifier unique to the user at the identity provider.
    pub sub: String,
    /// Verified email address, if available.
    pub email: Option<String>,
    /// Full display name, if available.
    pub name: Option<String>,
    /// Name of the identity provider that authenticated this user.
    pub provider: String,
    /// Roles derived from group membership for authorization checks.
    #[serde(default)]
    pub roles: Vec<String>,
    /// Issued-at timestamp (Unix seconds).
    pub iat: u64,
    /// Expiration timestamp (Unix seconds).
    pub exp: u64,
    /// Unique JWT identifier for this token.
    pub jti: String,
    /// Issuer claim (required by tokenkit JWT validation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>,
}
