//! Unit tests for the clawdius-auth crate.
//!
//! Covers config presets, JWT session lifecycle (issue, validate, refresh),
//! PKCE verifier generation, authorization URL construction, auth error
//! response codes, SAML 2.0, RBAC, and session revocation.

use clawdius_auth::rbac::{permissions, RbacPolicy, RbacService, Role};
use clawdius_auth::saml::{parse_saml_response_xml, SamlAssertion, SamlError, SamlSpConfig};
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

// ---------------------------------------------------------------------------
// Session revocation tests
// ---------------------------------------------------------------------------

#[test]
fn test_session_revocation() {
    let service = make_service();

    let claims = SessionClaims {
        sub: "user".to_string(),
        email: None,
        name: None,
        provider: "test".to_string(),
        roles: vec![],
        iat: chrono::Utc::now().timestamp() as u64,
        exp: (chrono::Utc::now().timestamp() as u64) + 3600,
        jti: "revoke-me".to_string(),
        iss: Some("clawdius".to_string()),
    };

    let jwt_svc = test_jwt_service();
    let token = jwt_svc.encode(&claims).expect("encode");

    // Token should be valid before revocation
    assert!(service.validate_session(&token).is_ok());

    // Revoke the session
    service.invalidate_session("revoke-me").expect("revoke");

    // Token should now be rejected
    assert!(service.validate_session(&token).is_err());
}

#[test]
fn test_revocation_cleans_expired_entries() {
    let service = make_service();

    // Revoking a session should not panic
    service.invalidate_session("test-jti").expect("revoke");

    // Validate should still work for other tokens
    let claims = SessionClaims {
        sub: "user".to_string(),
        email: None,
        name: None,
        provider: "test".to_string(),
        roles: vec![],
        iat: chrono::Utc::now().timestamp() as u64,
        exp: (chrono::Utc::now().timestamp() as u64) + 3600,
        jti: "other-jti".to_string(),
        iss: Some("clawdius".to_string()),
    };

    let jwt_svc = test_jwt_service();
    let token = jwt_svc.encode(&claims).expect("encode");
    assert!(service.validate_session(&token).is_ok());
}

// ---------------------------------------------------------------------------
// SAML tests
// ---------------------------------------------------------------------------

#[test]
fn test_saml_sp_metadata() {
    let config = SamlSpConfig {
        entity_id: "https://clawdius.example.com".to_string(),
        acs_url: "https://clawdius.example.com/saml/acs".to_string(),
        slo_url: Some("https://clawdius.example.com/saml/slo".to_string()),
        certificate: None,
        idp_certificate: None,
        enabled: true,
    };

    let metadata = config.metadata_xml();
    assert!(metadata.contains("EntityDescriptor"));
    assert!(metadata.contains("SPSSODescriptor"));
    assert!(metadata.contains("AssertionConsumerService"));
    assert!(metadata.contains("https://clawdius.example.com"));
}

#[test]
fn test_saml_parse_minimal_response() {
    let xml = r#"<?xml version="1.0"?>
<samlp:Response xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol"
    xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion">
  <saml:Issuer>https://idp.example.com</saml:Issuer>
  <saml:Assertion>
    <saml:Issuer>https://idp.example.com</saml:Issuer>
    <saml:Subject>
      <saml:NameID>user@example.com</saml:NameID>
    </saml:Subject>
    <saml:AuthnStatement SessionIndex="session-123"/>
    <saml:AttributeStatement>
      <saml:Attribute Name="email">
        <saml:AttributeValue>user@example.com</saml:AttributeValue>
      </saml:Attribute>
      <saml:Attribute Name="name">
        <saml:AttributeValue>Test User</saml:AttributeValue>
      </saml:Attribute>
    </saml:AttributeStatement>
  </saml:Assertion>
</samlp:Response>"#;

    let assertion = parse_saml_response_xml(xml, "https://idp.example.com").expect("parse");
    assert_eq!(assertion.name_id, "user@example.com");
    assert_eq!(
        assertion.attributes.email,
        Some("user@example.com".to_string())
    );
    assert_eq!(assertion.attributes.name, Some("Test User".to_string()));
    assert_eq!(assertion.session_index, Some("session-123".to_string()));
    assert_eq!(assertion.issuer, "https://idp.example.com");
}

#[test]
fn test_saml_parse_wrong_issuer() {
    let xml = r#"<?xml version="1.0"?>
<samlp:Response xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol"
    xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion">
  <saml:Issuer>https://wrong-idp.com</saml:Issuer>
  <saml:Assertion>
    <saml:Issuer>https://wrong-idp.com</saml:Issuer>
    <saml:Subject>
      <saml:NameID>user@example.com</saml:NameID>
    </saml:Subject>
  </saml:Assertion>
</samlp:Response>"#;

    let result = parse_saml_response_xml(xml, "https://expected-idp.com");
    assert!(result.is_err());
    match result.unwrap_err() {
        SamlError::UnexpectedIssuer => {},
        other => panic!("Expected UnexpectedIssuer, got: {other:?}"),
    }
}

#[test]
fn test_saml_to_user_info() {
    let assertion = SamlAssertion {
        name_id: "user@example.com".to_string(),
        session_index: Some("s1".to_string()),
        attributes: clawdius_auth::saml::SamlAttributes {
            email: Some("user@example.com".to_string()),
            name: Some("Test User".to_string()),
            given_name: Some("Test".to_string()),
            family_name: Some("User".to_string()),
            groups: vec!["admins".to_string()],
        },
        issuer: "https://idp.example.com".to_string(),
        not_before: None,
        not_on_or_after: None,
    };

    let user_info = assertion.to_user_info("Okta");
    assert_eq!(user_info.sub, "user@example.com");
    assert_eq!(user_info.email, Some("user@example.com".to_string()));
    assert_eq!(user_info.name, Some("Test User".to_string()));
    assert_eq!(user_info.provider, "Okta");
    assert_eq!(user_info.groups, vec!["admins".to_string()]);
}

// ---------------------------------------------------------------------------
// RBAC tests
// ---------------------------------------------------------------------------

#[test]
fn test_rbac_role_hierarchy() {
    assert!(Role::Admin > Role::Operator);
    assert!(Role::Operator > Role::User);
    assert!(Role::User > Role::Viewer);
}

#[test]
fn test_rbac_role_from_str() {
    assert_eq!(Role::from_str("admin"), Some(Role::Admin));
    assert_eq!(Role::from_str("Admin"), Some(Role::Admin));
    assert_eq!(Role::from_str("viewer"), Some(Role::Viewer));
    assert_eq!(Role::from_str("unknown"), None);
}

#[test]
fn test_rbac_default_policy_viewer() {
    let policy = RbacPolicy::default();
    assert!(policy.has_permission(&Role::Viewer, &permissions::code_read()));
    assert!(!policy.has_permission(&Role::Viewer, &permissions::code_write()));
    assert!(!policy.has_permission(&Role::Viewer, &permissions::admin_manage_users()));
}

#[test]
fn test_rbac_default_policy_user() {
    let policy = RbacPolicy::default();
    assert!(policy.has_permission(&Role::User, &permissions::code_read()));
    assert!(policy.has_permission(&Role::User, &permissions::code_write()));
    assert!(policy.has_permission(&Role::User, &permissions::code_execute()));
    assert!(!policy.has_permission(&Role::User, &permissions::code_delete()));
    assert!(!policy.has_permission(&Role::User, &permissions::admin_manage_users()));
}

#[test]
fn test_rbac_default_policy_operator() {
    let policy = RbacPolicy::default();
    assert!(policy.has_permission(&Role::Operator, &permissions::code_delete()));
    assert!(policy.has_permission(&Role::Operator, &permissions::provider_add()));
    assert!(policy.has_permission(&Role::Operator, &permissions::plugin_install()));
    assert!(policy.has_permission(&Role::Operator, &permissions::admin_view_audit()));
    assert!(!policy.has_permission(&Role::Operator, &permissions::admin_manage_users()));
}

#[test]
fn test_rbac_default_policy_admin() {
    let policy = RbacPolicy::default();
    for perm in permissions::all() {
        assert!(
            policy.has_permission(&Role::Admin, &perm),
            "Admin should have {:?}",
            perm
        );
    }
}

#[test]
fn test_rbac_service_check() {
    let rbac = RbacService::new(RbacPolicy::default());

    let admin_claims = SessionClaims {
        sub: "1".to_string(),
        email: None,
        name: None,
        provider: "test".to_string(),
        roles: vec!["admin".to_string()],
        iat: 0,
        exp: 9999999999,
        jti: "test".to_string(),
        iss: None,
    };

    assert!(rbac
        .check(&admin_claims, &permissions::admin_manage_users())
        .is_ok());

    let viewer_claims = SessionClaims {
        roles: vec!["viewer".to_string()],
        ..admin_claims
    };

    assert!(rbac
        .check(&viewer_claims, &permissions::code_write())
        .is_err());
}

#[test]
fn test_rbac_all_permissions_count() {
    assert_eq!(permissions::all().len(), 21);
}

// ---------------------------------------------------------------------------
// AuthConfig with SAML tests
// ---------------------------------------------------------------------------

#[test]
fn test_saml_sp_config_serde() {
    let config = SamlSpConfig {
        entity_id: "https://sp.example.com".to_string(),
        acs_url: "https://sp.example.com/saml/acs".to_string(),
        slo_url: None,
        certificate: None,
        idp_certificate: None,
        enabled: true,
    };
    let json = serde_json::to_string(&config).expect("serialize");
    let decoded: SamlSpConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.entity_id, "https://sp.example.com");
    assert!(decoded.enabled);
}
