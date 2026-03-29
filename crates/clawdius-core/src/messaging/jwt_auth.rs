#![deny(unsafe_code)]

//! JWT Authentication for Management API
//!
//! Provides JWT token generation and validation for the tenant management
//! and other admin endpoints. Tokens are signed with HMAC-SHA256 and carry
//! a `sub` (subject), `exp` (expiration), and optional `role` claim.
//!
//! # Feature Gate
//!
//! Enabled with `--features jwt` on both `clawdius-core` and `clawdius-server`.
//!
//! # Usage
//!
//! ```ignore
//! use clawdius_core::messaging::jwt_auth::JwtAuth;
//!
//! let auth = JwtAuth::new("my-secret-key")?;
//! let token = auth.create_token("admin", Some("admin"), 3600)?;
//! let claims = auth.validate_token(&token)?;
//! assert_eq!(claims.sub, "admin");
//! ```

use serde::{Deserialize, Serialize};

/// Errors from JWT operations.
#[derive(Debug, thiserror::Error)]
pub enum JwtError {
    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    #[error("token has expired")]
    Expired,
    #[error("invalid token: {0}")]
    Invalid(String),
    #[error("missing required claim: {0}")]
    MissingClaim(String),
    #[error("JWT secret key is empty")]
    EmptySecret,
}

/// JWT claims carried in management API tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject — typically a user ID or role identifier.
    pub sub: String,
    /// Optional role (e.g., "admin", "operator").
    pub role: Option<String>,
    /// Issued-at timestamp (Unix seconds).
    pub iat: u64,
    /// Expiration timestamp (Unix seconds).
    pub exp: u64,
}

/// JWT authentication: signs and validates tokens with HMAC-SHA256.
#[derive(Clone)]
pub struct JwtAuth {
    encoding_key: jsonwebtoken::EncodingKey,
    decoding_key: jsonwebtoken::DecodingKey,
}

impl JwtAuth {
    /// Create a new JWT auth instance with the given HMAC secret.
    ///
    /// The secret should be at least 32 characters for production use.
    pub fn new(secret: &str) -> Result<Self, JwtError> {
        if secret.is_empty() {
            return Err(JwtError::EmptySecret);
        }
        Ok(Self {
            encoding_key: jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
        })
    }

    /// Create a signed JWT token.
    ///
    /// `subject` identifies the principal (user ID, role, etc.).
    /// `role` is an optional role claim.
    /// `ttl_secs` is the token lifetime in seconds (e.g., 3600 for 1 hour).
    pub fn create_token(
        &self,
        subject: &str,
        role: Option<&str>,
        ttl_secs: u64,
    ) -> Result<String, JwtError> {
        let now = chrono::Utc::now().timestamp() as u64;
        let claims = JwtClaims {
            sub: subject.to_string(),
            role: role.map(|r| r.to_string()),
            iat: now,
            exp: now + ttl_secs,
        };
        let header = jsonwebtoken::Header::default();
        Ok(jsonwebtoken::encode(&header, &claims, &self.encoding_key)?)
    }

    /// Validate a JWT token and return its claims.
    ///
    /// Returns `Err` if the token is malformed, expired, or the signature
    /// doesn't match.
    pub fn validate_token(&self, token: &str) -> Result<JwtClaims, JwtError> {
        let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        let data = jsonwebtoken::decode::<JwtClaims>(token, &self.decoding_key, &validation)
            .map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => JwtError::Expired,
                _ => JwtError::Invalid(e.to_string()),
            })?;
        Ok(data.claims)
    }
}

/// Check whether a string looks like a JWT (three dot-separated base64url segments).
pub fn looks_like_jwt(token: &str) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_auth() -> JwtAuth {
        JwtAuth::new("test-secret-key-for-testing-only").expect("auth ok")
    }

    #[test]
    fn create_and_validate_token() {
        let auth = test_auth();
        let token = auth
            .create_token("user-123", Some("admin"), 3600)
            .expect("create ok");
        let claims = auth.validate_token(&token).expect("validate ok");
        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.role.as_deref(), Some("admin"));
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn expired_token_fails() {
        let auth = test_auth();
        // Create a token that already expired
        let claims = JwtClaims {
            sub: "test".to_string(),
            role: None,
            iat: 0,
            exp: 1, // Jan 1 1970 — definitely expired
        };
        let header = jsonwebtoken::Header::default();
        let token = jsonwebtoken::encode(&header, &claims, &auth.encoding_key).expect("encode ok");
        let result = auth.validate_token(&token);
        assert!(result.is_err());
    }

    #[test]
    fn tampered_token_fails() {
        let auth = test_auth();
        let token = auth.create_token("user-1", None, 3600).expect("create ok");
        let tampered = format!("{}.{}", &token[..token.len() - 3], "xxx");
        let result = auth.validate_token(&tampered);
        assert!(result.is_err());
    }

    #[test]
    fn empty_secret_fails() {
        let result = JwtAuth::new("");
        assert!(result.is_err());
    }

    #[test]
    fn looks_like_jwt_valid() {
        // Real JWT-like structure
        assert!(looks_like_jwt("eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJhZG1pbiIsImlhdCI6MTcwMDAwMDAwMCwiZXhwIjoxNzAwMDAwMDAwfQ.xxx"));
        assert!(looks_like_jwt("a.b.c"));
        assert!(!looks_like_jwt("not-a-jwt"));
        assert!(!looks_like_jwt(""));
        assert!(!looks_like_jwt("a.b"));
        assert!(!looks_like_jwt("a.b.c.d"));
    }

    #[test]
    fn token_without_role() {
        let auth = test_auth();
        let token = auth.create_token("user-456", None, 600).expect("create ok");
        let claims = auth.validate_token(&token).expect("validate ok");
        assert_eq!(claims.sub, "user-456");
        assert!(claims.role.is_none());
    }
}
