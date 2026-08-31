//! Unit tests for the clawdius-auth crate.
//!
//! Covers config presets, JWT session lifecycle (issue, validate, refresh),
//! PKCE verifier generation, authorization URL construction, and auth error
//! response codes.

use clawdius_auth::{
    AuthConfig, AuthError, AuthService, OidcProviderConfig, SessionClaims, UserInfo,
};
use tokenkit::service::{JwtAlgorithm, JwtConfig, JwtService};

// ---------------------------------------------------------------------------
// Config tests
// ---------------------------------------------------------------------------

#[test]
fn test_auth_config_default() {
    let config = AuthConfig::default();
    assert!(config.providers.is_empty());
    assert!(!config.jwt_secret.is_empty());
    assert_eq!(config.session_duration_secs, 3600);
    assert_eq!(config.refresh_duration_secs, 604_800);
}

#[test]
fn test_auth_config_default_unique_secrets() {
    let c1 = AuthConfig::default();
    let c2 = AuthConfig::default();
    assert_ne!(
        c1.jwt_secret, c2.jwt_secret,
        "Each default config must generate a unique secret"
    );
}

#[test]
fn test_okta_provider() {
    let p = OidcProviderConfig::okta("example.okta.com", "cid", "secret", "https://app/cb");
    assert_eq!(p.name, "Okta");
    assert_eq!(p.issuer_url, "https://example.okta.com");
    assert!(p.enabled);
    assert!(p.scopes.contains(&"openid".to_string()));
}

#[test]
fn test_azure_ad_provider() {
    let p = OidcProviderConfig::azure_ad("tenant-123", "cid", "secret", "https://app/cb");
    assert_eq!(p.name, "Azure AD");
    assert!(p.issuer_url.contains("tenant-123"));
    assert!(p.issuer_url.contains("v2.0"));
}

#[test]
fn test_github_provider() {
    let p = OidcProviderConfig::github("cid", "secret", "https://app/cb");
    assert_eq!(p.name, "GitHub");
    assert!(p.scopes.contains(&"read:user".to_string()));
}

#[test]
fn test_gitlab_provider() {
    let p = OidcProviderConfig::gitlab("gitlab.com", "cid", "secret", "https://app/cb");
    assert_eq!(p.name, "GitLab");
    assert!(p.issuer_url.contains(".well-known/openid-configuration"));
}

#[test]
fn test_google_provider() {
    let p = OidcProviderConfig::google("cid", "secret", "https://app/cb");
    assert_eq!(p.name, "Google");
    assert_eq!(p.issuer_url, "https://accounts.google.com");
}

#[test]
fn test_config_serde_roundtrip() {
    let config = AuthConfig {
        providers: vec![OidcProviderConfig::github("c", "s", "r")],
        jwt_secret: "test-secret".to_string(),
        session_duration_secs: 1800,
        refresh_duration_secs: 86_400,
        allowed_origins: vec!["https://app.com".to_string()],
    };
    let json = serde_json::to_string(&config).expect("serialize");
    let deserialized: AuthConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.jwt_secret, "test-secret");
    assert_eq!(deserialized.session_duration_secs, 1800);
    assert_eq!(deserialized.providers.len(), 1);
}

// ---------------------------------------------------------------------------
// Service lifecycle tests
// ---------------------------------------------------------------------------

const TEST_SECRET: &str = "test-jwt-secret-key-for-testing";

fn make_service() -> AuthService {
    let config = AuthConfig {
        providers: vec![OidcProviderConfig::okta(
            "example.okta.com",
            "client-id",
            "client-secret",
            "https://localhost/callback",
        )],
        jwt_secret: TEST_SECRET.to_string(),
        session_duration_secs: 3600,
        refresh_duration_secs: 604_800,
        allowed_origins: vec![],
    };
    AuthService::new(config).expect("AuthService creation should succeed")
}

fn test_jwt_service() -> JwtService {
    JwtService::new(JwtConfig {
        algorithm: JwtAlgorithm::HS256,
        secret: zeroize::Zeroizing::new(TEST_SECRET.to_string()),
        issuer: Some("clawdius".to_string()),
        ..Default::default()
    })
}

#[test]
fn test_service_creation() {
    let _service = make_service();
}

#[test]
fn test_authorization_url() {
    let service = make_service();
    let (url, state, verifier) = service
        .authorization_url("Okta")
        .expect("Should build authorization URL");

    assert!(url.contains("example.okta.com/authorize"));
    assert!(url.contains("client_id=client-id"));
    assert!(url.contains("response_type=code"));
    assert!(url.contains("state="));
    assert!(!state.is_empty());
    assert!(!verifier.is_empty());
}

#[test]
fn test_authorization_url_unknown_provider() {
    let service = make_service();
    let result = service.authorization_url("NonExistent");
    assert!(result.is_err());
}

#[test]
fn test_authorization_url_disabled_provider() {
    let mut config = AuthConfig::default();
    let mut provider = OidcProviderConfig::okta("test.okta.com", "c", "s", "r");
    provider.enabled = false;
    config.providers.push(provider);
    config.jwt_secret = "secret".to_string();
    let service = AuthService::new(config).expect("service");
    let result = service.authorization_url("Okta");
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// JWT session tests
// ---------------------------------------------------------------------------

#[test]
fn test_session_token_issue_and_validate() {
    let service = make_service();

    let user = UserInfo {
        sub: "user-123".to_string(),
        email: Some("user@example.com".to_string()),
        name: Some("Test User".to_string()),
        given_name: None,
        family_name: None,
        picture: None,
        provider: "Okta".to_string(),
        groups: vec!["admins".to_string()],
    };

    let claims = SessionClaims {
        sub: user.sub.clone(),
        email: user.email.clone(),
        name: user.name.clone(),
        provider: user.provider.clone(),
        roles: user.groups.clone(),
        iat: chrono::Utc::now().timestamp() as u64,
        exp: (chrono::Utc::now().timestamp() as u64) + 3600,
        jti: uuid::Uuid::new_v4().to_string(),
        iss: Some("clawdius".to_string()),
    };

    let jwt_svc = test_jwt_service();
    let token = jwt_svc.encode(&claims).expect("encode token");

    let validated = service.validate_session(&token).expect("validate session");
    assert_eq!(validated.sub, "user-123");
    assert_eq!(validated.email, Some("user@example.com".to_string()));
    assert_eq!(validated.provider, "Okta");
    assert_eq!(validated.roles, vec!["admins".to_string()]);
}

#[test]
fn test_validate_session_invalid_token() {
    let service = make_service();
    let result = service.validate_session("invalid.jwt.token");
    assert!(result.is_err());
}

#[test]
fn test_validate_session_wrong_secret() {
    let service = make_service();

    let claims = SessionClaims {
        sub: "user".to_string(),
        email: None,
        name: None,
        provider: "test".to_string(),
        roles: vec![],
        iat: 0,
        exp: chrono::Utc::now().timestamp() as u64 + 3600,
        jti: "test".to_string(),
        iss: Some("clawdius".to_string()),
    };

    let wrong_svc = JwtService::new(JwtConfig {
        algorithm: JwtAlgorithm::HS256,
        secret: zeroize::Zeroizing::new("wrong-secret".to_string()),
        ..Default::default()
    });
    let token = wrong_svc.encode(&claims).expect("encode");

    let result = service.validate_session(&token);
    assert!(
        result.is_err(),
        "Token signed with wrong key must be rejected"
    );
}

#[test]
fn test_validate_session_expired() {
    let service = make_service();

    let claims = SessionClaims {
        sub: "user".to_string(),
        email: None,
        name: None,
        provider: "test".to_string(),
        roles: vec![],
        iat: 1,
        exp: 2, // Expired in the distant past.
        jti: "expired".to_string(),
        iss: Some("clawdius".to_string()),
    };

    let jwt_svc = test_jwt_service();
    let token = jwt_svc.encode(&claims).expect("encode");

    let result = service.validate_session(&token);
    assert!(result.is_err(), "Expired token must be rejected");
}

#[test]
fn test_refresh_session() {
    let service = make_service();

    let claims = SessionClaims {
        sub: "refresh-user".to_string(),
        email: Some("refresh@example.com".to_string()),
        name: None,
        provider: "Okta".to_string(),
        roles: vec![],
        iat: chrono::Utc::now().timestamp() as u64,
        exp: (chrono::Utc::now().timestamp() as u64) + 3600,
        jti: uuid::Uuid::new_v4().to_string(),
        iss: Some("clawdius".to_string()),
    };

    let jwt_svc = test_jwt_service();
    let original_token = jwt_svc.encode(&claims).expect("encode");

    let new_token = service
        .refresh_session(&original_token)
        .expect("refresh should succeed");

    let validated = service
        .validate_session(&new_token)
        .expect("new token should validate");
    assert_eq!(validated.sub, "refresh-user");
    assert_eq!(validated.email, Some("refresh@example.com".to_string()));
    assert_ne!(validated.jti, claims.jti, "New JTI must differ");
}

#[test]
fn test_refresh_session_invalid_token() {
    let service = make_service();
    let result = service.refresh_session("garbage");
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// UserInfo and SessionClaims tests
// ---------------------------------------------------------------------------

#[test]
fn test_user_info_serde_roundtrip() {
    let user = UserInfo {
        sub: "sub-1".to_string(),
        email: Some("a@b.c".to_string()),
        name: Some("Name".to_string()),
        given_name: Some("First".to_string()),
        family_name: Some("Last".to_string()),
        picture: Some("https://pic".to_string()),
        provider: "Okta".to_string(),
        groups: vec!["g1".to_string(), "g2".to_string()],
    };
    let json = serde_json::to_string(&user).expect("serialize");
    let decoded: UserInfo = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.sub, "sub-1");
    assert_eq!(decoded.groups.len(), 2);
}

#[test]
fn test_session_claims_serde_roundtrip() {
    let claims = SessionClaims {
        sub: "s".to_string(),
        email: None,
        name: None,
        provider: "p".to_string(),
        roles: vec!["admin".to_string()],
        iat: 1000,
        exp: 4600,
        jti: "jti-1".to_string(),
        iss: None,
    };
    let json = serde_json::to_string(&claims).expect("serialize");
    let decoded: SessionClaims = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.iat, 1000);
    assert_eq!(decoded.exp, 4600);
    assert_eq!(decoded.jti, "jti-1");
}

#[test]
fn test_session_claims_default_roles() {
    let json = r#"{"sub":"s","email":null,"name":null,"provider":"p","iat":0,"exp":0,"jti":"j"}"#;
    let decoded: SessionClaims = serde_json::from_str(json).expect("deserialize");
    assert!(decoded.roles.is_empty());
}

// ---------------------------------------------------------------------------
// AuthError tests
// ---------------------------------------------------------------------------

#[test]
fn test_auth_error_debug() {
    let errors = vec![
        AuthError::MissingAuthService,
        AuthError::MissingToken,
        AuthError::InvalidTokenFormat,
        AuthError::InvalidToken,
        AuthError::ExpiredToken,
    ];
    for e in &errors {
        let _ = format!("{e:?}");
    }
    assert_eq!(errors.len(), 5);
}
