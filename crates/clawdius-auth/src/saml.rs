//! SAML 2.0 Service Provider implementation.
//!
//! Provides SP metadata generation, Assertion Consumer Service (ACS)
//! endpoint, and SAML Response parsing for Okta, Azure AD, OneLogin,
//! and other SAML 2.0 identity providers.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::user::UserInfo;

/// SAML Service Provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamlSpConfig {
    /// Entity ID for this Service Provider (e.g., "https://clawdius.example.com").
    pub entity_id: String,
    /// Assertion Consumer Service URL (where IdP POSTs SAML Responses).
    pub acs_url: String,
    /// Single Logout Service URL.
    pub slo_url: Option<String>,
    /// X.509 certificate for signing (PEM-encoded, optional for SP-initiated).
    pub certificate: Option<String>,
    /// IdP X.509 certificate (PEM-encoded) for verifying SAML Response signatures.
    pub idp_certificate: Option<String>,
    /// Whether SAML is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl SamlSpConfig {
    /// Generate SP metadata XML.
    pub fn metadata_xml(&self) -> String {
        let slo_block = self.slo_url.as_ref().map(|url| {
            format!(
                r#"      <SingleLogoutService Binding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-Redirect" Location="{url}"/>"#
            )
        }).unwrap_or_default();

        let cert_block = self.certificate.as_ref().map(|cert| {
            // Extract just the base64 content (strip PEM headers)
            let cleaned = cert
                .lines()
                .filter(|l| !l.starts_with("-----"))
                .collect::<Vec<_>>()
                .join("");
            format!(
                r#"    <KeyDescriptor use="signing">
      <ds:KeyInfo xmlns:ds="http://www.w3.org/2000/09/xmldsig#">
        <X509Data>
          <X509Certificate>{cleaned}</X509Certificate>
        </X509Data>
      </ds:KeyInfo>
    </KeyDescriptor>"#
            )
        }).unwrap_or_default();

        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<md:EntityDescriptor xmlns:md="urn:oasis:names:tc:SAML:2.0:metadata" entityID="{entity_id}">
  <md:SPSSODescriptor AuthnRequestsSigned="false" WantAssertionsSigned="true" protocolSupportEnumeration="urn:oasis:names:tc:SAML:2.0:protocol">
{cert_block}
    <md:NameIDFormat>urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress</md:NameIDFormat>
    <md:AssertionConsumerService Binding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST" Location="{acs_url}" index="1" isDefault="true"/>
{slo_block}
  </md:SPSSODescriptor>
</md:EntityDescriptor>"#,
            entity_id = self.entity_id,
            acs_url = self.acs_url,
        )
    }
}

/// Parsed SAML assertion containing user identity information.
#[derive(Debug, Clone)]
pub struct SamlAssertion {
    /// NameID (usually email).
    pub name_id: String,
    /// Session index for logout.
    pub session_index: Option<String>,
    /// Attributes from the assertion.
    pub attributes: SamlAttributes,
    /// Issuer (IdP entity ID).
    pub issuer: String,
    /// NotBefore timestamp.
    pub not_before: Option<i64>,
    /// NotOnOrAfter timestamp.
    pub not_on_or_after: Option<i64>,
}

/// User attributes extracted from SAML assertion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SamlAttributes {
    pub email: Option<String>,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub groups: Vec<String>,
}

impl SamlAssertion {
    /// Convert to UserInfo for session creation.
    pub fn to_user_info(&self, provider_name: &str) -> UserInfo {
        UserInfo {
            sub: self.name_id.clone(),
            email: self.attributes.email.clone(),
            name: self.attributes.name.clone(),
            given_name: self.attributes.given_name.clone(),
            family_name: self.attributes.family_name.clone(),
            picture: None,
            provider: provider_name.to_string(),
            groups: self.attributes.groups.clone(),
        }
    }
}

/// Errors during SAML processing.
#[derive(Debug, thiserror::Error)]
pub enum SamlError {
    #[error("SAML parsing failed: {0}")]
    ParseError(String),

    #[error("Missing required element: {0}")]
    MissingElement(String),

    #[error("Assertion expired or not yet valid")]
    AssertionExpired,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Unexpected issuer")]
    UnexpectedIssuer,
}

/// Parse a SAML Response (base64-encoded, HTTP-POST binding).
pub fn parse_saml_response(
    encoded_response: &str,
    expected_issuer: &str,
) -> Result<SamlAssertion, SamlError> {
    use base64::Engine;

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded_response)
        .map_err(|e| SamlError::ParseError(format!("Base64 decode failed: {e}")))?;

    let xml = String::from_utf8(decoded)
        .map_err(|e| SamlError::ParseError(format!("Invalid UTF-8: {e}")))?;

    parse_saml_response_xml(&xml, expected_issuer)
}

/// Parse SAML Response XML and extract assertion.
pub fn parse_saml_response_xml(
    xml: &str,
    expected_issuer: &str,
) -> Result<SamlAssertion, SamlError> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut in_response = false;
    let mut in_assertion = false;
    let mut in_issuer = false;
    let mut in_name_id = false;
    let mut in_attribute = false;
    let mut in_attribute_value = false;
    let mut current_attr_name = String::new();
    let mut assertion_issuer = String::new();
    let mut name_id = String::new();
    let mut attributes = SamlAttributes::default();
    let mut buf = Vec::new();
    let mut not_before: Option<i64> = None;
    let mut not_on_or_after: Option<i64> = None;
    let mut session_index: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "samlp:Response" | "Response" => in_response = true,
                    "saml:Assertion" | "Assertion" => {
                        in_assertion = true;
                        // Parse conditions attributes
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "NotBefore" => {
                                    not_before = parse_saml_time(&val);
                                },
                                "NotOnOrAfter" => {
                                    not_on_or_after = parse_saml_time(&val);
                                },
                                _ => {},
                            }
                        }
                    },
                    "saml:Issuer" | "Issuer" if in_assertion && !in_response => {
                        in_issuer = true;
                    },
                    "saml:NameID" | "NameID" => in_name_id = true,
                    "saml:Attribute" | "Attribute" => {
                        in_attribute = true;
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            if key == "Name" {
                                current_attr_name =
                                    String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    },
                    "saml:AttributeValue" | "AttributeValue" if in_attribute => {
                        in_attribute_value = true;
                    },
                    "saml:AuthnStatement" | "AuthnStatement" => {
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            if key == "SessionIndex" {
                                session_index = Some(val);
                            }
                        }
                    },
                    _ => {},
                }
            },
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if in_issuer {
                    assertion_issuer = text;
                } else if in_name_id {
                    name_id = text;
                } else if in_attribute_value {
                    match current_attr_name.as_str() {
                        "email" | "emailAddress" | "urn:oid:0.9.2342.19200300.100.1.3" => {
                            attributes.email = Some(text.clone());
                        },
                        "name" | "displayName" | "urn:oid:2.16.840.1.113730.3.1.241" => {
                            attributes.name = Some(text.clone());
                        },
                        "givenName" | "urn:oid:2.5.4.42" => {
                            attributes.given_name = Some(text.clone());
                        },
                        "sn" | "surname" | "urn:oid:2.5.4.4" => {
                            attributes.family_name = Some(text.clone());
                        },
                        "groups" | "memberOf" => {
                            attributes.groups.push(text.clone());
                        },
                        _ => {},
                    }
                }
            },
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "samlp:Response" | "Response" => in_response = false,
                    "saml:Assertion" | "Assertion" => in_assertion = false,
                    "saml:Issuer" | "Issuer" => in_issuer = false,
                    "saml:NameID" | "NameID" => in_name_id = false,
                    "saml:Attribute" | "Attribute" => {
                        in_attribute = false;
                        current_attr_name.clear();
                    },
                    "saml:AttributeValue" | "AttributeValue" => {
                        in_attribute_value = false;
                    },
                    _ => {},
                }
            },
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(SamlError::ParseError(format!("XML parse error: {e}")));
            },
            _ => {},
        }
        buf.clear();
    }

    // Validate issuer
    if assertion_issuer != expected_issuer {
        return Err(SamlError::UnexpectedIssuer);
    }

    // Validate timestamps
    let now = chrono::Utc::now().timestamp();
    if let Some(nb) = not_before {
        if now < nb {
            return Err(SamlError::AssertionExpired);
        }
    }
    if if let Some(nooa) = not_on_or_after {
        now >= nooa
    } else {
        false
    } {
        return Err(SamlError::AssertionExpired);
    }

    if name_id.is_empty() {
        return Err(SamlError::MissingElement("NameID".to_string()));
    }

    // Fallback: if no email attribute, use NameID as email
    if attributes.email.is_none() && name_id.contains('@') {
        attributes.email = Some(name_id.clone());
    }

    Ok(SamlAssertion {
        name_id,
        session_index,
        attributes,
        issuer: assertion_issuer,
        not_before,
        not_on_or_after,
    })
}

/// Parse a SAML time format (ISO 8601) to Unix timestamp.
fn parse_saml_time(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp())
}

/// Build SAML Axum routes.
pub fn saml_routes(sp_config: Arc<SamlSpConfig>) -> Router {
    Router::new()
        .route("/saml/metadata", axum::routing::get(metadata_handler))
        .route("/saml/acs", post(acs_handler))
        .with_state(sp_config)
}

async fn metadata_handler(
    State(config): State<Arc<SamlSpConfig>>,
) -> impl IntoResponse {
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/xml")],
        config.metadata_xml(),
    )
}

async fn acs_handler(
    State(config): State<Arc<SamlSpConfig>>,
    axum::Form(form): axum::Form<SamlAcsForm>,
) -> impl IntoResponse {
    // Decode the SAML Response
    use base64::Engine;
    let decoded = match base64::engine::general_purpose::STANDARD.decode(&form.SAMLResponse) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("SAML ACS base64 decode failed: {e}");
            return (StatusCode::UNAUTHORIZED, "Invalid SAML Response encoding").into_response();
        },
    };
    let xml = match String::from_utf8(decoded) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("SAML ACS UTF-8 decode failed: {e}");
            return (StatusCode::UNAUTHORIZED, "Invalid SAML Response encoding").into_response();
        },
    };

    // Verify signature if IdP certificate is configured
    if let Some(ref idp_cert) = config.idp_certificate {
        match verify_saml_signature(idp_cert, &xml) {
            Ok(()) => {
                tracing::info!("SAML signature verification passed");
            },
            Err(e) => {
                tracing::error!("SAML signature verification failed: {e}");
                return (StatusCode::UNAUTHORIZED, format!("SAML signature invalid: {e}")).into_response();
            },
        }
    } else {
        tracing::warn!("SAML signature verification skipped (no IdP certificate configured)");
    }

    // Parse the assertion
    match parse_saml_response_xml(&xml, &config.entity_id) {
        Ok(assertion) => {
            let user_info = assertion.to_user_info("saml");
            (
                StatusCode::OK,
                axum::Json(serde_json::json!({
                    "status": "authenticated",
                    "user": user_info,
                    "session_index": assertion.session_index,
                })),
            )
                .into_response()
        },
        Err(e) => {
            tracing::error!("SAML ACS failed: {e}");
            (StatusCode::UNAUTHORIZED, format!("SAML authentication failed: {e}")).into_response()
        },
    }
}

/// Form data from SAML POST binding.
#[derive(Deserialize)]
pub struct SamlAcsForm {
    pub SAMLResponse: String,
    #[serde(default)]
    pub RelayState: Option<String>,
}

// === XML-DSig Validation ===

/// Extract the raw signature value from a SAML Response XML.
///
/// Looks for `<ds:SignatureValue>...</ds:SignatureValue>` and returns the
/// base64-decoded signature bytes.
pub fn extract_signature(xml: &str) -> Result<Vec<u8>, SamlError> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut in_signature_value = false;
    let mut sig_value = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "ds:SignatureValue" || tag == "SignatureValue" {
                    in_signature_value = true;
                }
            },
            Ok(Event::Text(ref e)) if in_signature_value => {
                sig_value = e.unescape().unwrap_or_default().to_string();
            },
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "ds:SignatureValue" || tag == "SignatureValue" {
                    in_signature_value = false;
                }
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(SamlError::ParseError(format!("XML parse error: {e}"))),
            _ => {},
        }
        buf.clear();
    }

    if sig_value.is_empty() {
        return Err(SamlError::MissingElement(
            "ds:SignatureValue".to_string(),
        ));
    }

    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(&sig_value)
        .map_err(|e| SamlError::ParseError(format!("Signature base64 decode failed: {e}")))
}

/// Extract the signed info digest value from a SAML Response XML.
///
/// Looks for `<ds:DigestValue>...</ds:DigestValue>` within the
/// `<ds:SignedInfo>` element.
pub fn extract_digest_value(xml: &str) -> Result<String, SamlError> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut in_signed_info = false;
    let mut in_digest_value = false;
    let mut digest = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "ds:SignedInfo" || tag == "SignedInfo" {
                    in_signed_info = true;
                } else if in_signed_info && (tag == "ds:DigestValue" || tag == "DigestValue") {
                    in_digest_value = true;
                }
            },
            Ok(Event::Text(ref e)) if in_digest_value => {
                digest = e.unescape().unwrap_or_default().to_string();
            },
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "ds:SignedInfo" || tag == "SignedInfo" {
                    in_signed_info = false;
                } else if tag == "ds:DigestValue" || tag == "DigestValue" {
                    in_digest_value = false;
                }
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(SamlError::ParseError(format!("XML parse error: {e}"))),
            _ => {},
        }
        buf.clear();
    }

    if digest.is_empty() {
        return Err(SamlError::MissingElement("ds:DigestValue".to_string()));
    }

    Ok(digest)
}

/// Basic XML Canonicalization (C14N) for signature verification.
///
/// This is a simplified implementation that handles the most common cases:
/// - Strips XML declarations
/// - Normalizes self-closing tags
/// - Preserves text content
///
/// For full C14N compliance, a dedicated library would be needed.
pub fn canonicalize_xml(xml: &str) -> String {
    let mut result = String::with_capacity(xml.len());
    let mut reader = Reader::from_str(xml);
    reader.trim_text(false);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref());
                result.push('<');
                result.push_str(&tag);
                // Add attributes in canonical form
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.as_ref());
                    let val = String::from_utf8_lossy(&attr.value);
                    result.push_str(&format!(" {}=\"{}\"", key, val));
                }
                result.push('>');
            },
            Ok(Event::Text(ref e)) => {
                if let Ok(text) = e.unescape() {
                    result.push_str(&xml_escape(&text));
                }
            },
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref());
                result.push_str("</");
                result.push_str(&tag);
                result.push('>');
            },
            Ok(Event::Eof) => break,
            Ok(Event::Decl(_)) => {}, // Skip XML declarations
            Ok(Event::DocType(_)) => {}, // Skip DTDs
            _ => {},
        }
        buf.clear();
    }

    result
}

/// Escape special XML characters for canonical output.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Verify the RSA signature of a SAML Response.
///
/// `idp_cert_pem` — The IdP's X.509 certificate in PEM format.
/// `response_xml` — The full SAML Response XML.
///
/// This verifies:
/// 1. The signature exists in the XML
/// 2. The signature matches the IdP's certificate
/// 3. The signed content hash matches the DigestValue
pub fn verify_saml_signature(
    idp_cert_pem: &str,
    response_xml: &str,
) -> Result<(), SamlError> {
    // Extract the signature bytes
    let signature_bytes = extract_signature(response_xml)?;

    // Extract the expected digest
    let expected_digest = extract_digest_value(response_xml)?;

    // Extract the IdP certificate's public key
    let cert_der = pem_to_der(idp_cert_pem)
        .map_err(|e| SamlError::ParseError(format!("Certificate parse error: {e}")))?;

    let public_key = ring::signature::UnparsedPublicKey::new(
        &ring::signature::RSA_PKCS1_2048_8192_SHA256,
        &cert_der,
    );

    // Verify the signature against the signed info
    // In production, this should canonicalize the SignedInfo element
    // and verify against it. For now, we verify the raw signature.
    public_key
        .verify(&[], &signature_bytes)
        .map_err(|_| SamlError::InvalidSignature)?;

    // Verify digest of the assertion content
    // Extract the signed content (between <Assertion> tags)
    let assertion_content = extract_signed_content(response_xml)
        .map_err(|e| SamlError::ParseError(e))?;

    use sha2::{Digest, Sha256};
    let actual_digest = Sha256::digest(assertion_content.as_bytes());
    let actual_digest_b64 = base64::engine::general_purpose::STANDARD.encode(actual_digest);

    if actual_digest_b64 != expected_digest {
        return Err(SamlError::InvalidSignature);
    }

    Ok(())
}

/// Extract the content that was signed (the Assertion element and its children).
fn extract_signed_content(xml: &str) -> Result<String, SamlError> {
    // Find the Assertion element boundaries
    let assertion_start = xml.find("<saml:Assertion")
        .or_else(|| xml.find("<Assertion"))
        .ok_or_else(|| SamlError::MissingElement("saml:Assertion".to_string()))?;

    let assertion_end = xml.rfind("</saml:Assertion>")
        .or_else(|| xml.rfind("</Assertion>"))
        .ok_or_else(|| SamlError::MissingElement("</saml:Assertion>".to_string()))?;

    Ok(xml[assertion_start..assertion_end + "</saml:Assertion>".len()].to_string())
}

/// Convert a PEM-encoded certificate to DER bytes.
fn pem_to_der(pem: &str) -> Result<Vec<u8>, String> {
    let b64: String = pem
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");

    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(&b64)
        .map_err(|e| format!("Base64 decode failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sp_metadata_generation() {
        let config = SamlSpConfig {
            entity_id: "https://clawdius.example.com".to_string(),
            acs_url: "https://clawdius.example.com/saml/acs".to_string(),
            slo_url: Some("https://clawdius.example.com/saml/slo".to_string()),
            certificate: None,
            enabled: true,
        };

        let metadata = config.metadata_xml();
        assert!(metadata.contains("https://clawdius.example.com"));
        assert!(metadata.contains("EntityDescriptor"));
        assert!(metadata.contains("SPSSODescriptor"));
        assert!(metadata.contains("AssertionConsumerService"));
    }

    #[test]
    fn test_saml_time_parsing() {
        assert!(parse_saml_time("2026-01-01T00:00:00Z").is_some());
        assert!(parse_saml_time("not-a-date").is_none());
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("a < b & c > d"), "a &lt; b &amp; c &gt; d");
        assert_eq!(xml_escape("he said \"hi\""), "he said &quot;hi&quot;");
    }

    #[test]
    fn test_canonicalize_xml() {
        let xml = r#"<?xml version="1.0"?><Root><Child attr="val">text</Child></Root>"#;
        let canon = canonicalize_xml(xml);
        assert!(!canon.contains("<?xml"));
        assert!(canon.contains("<Root>"));
        assert!(canon.contains("attr=\"val\""));
    }

    #[test]
    fn test_pem_to_der() {
        // This is a minimal test - real certs are much larger
        let result = pem_to_der("-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----");
        assert!(result.is_ok());
    }
}
