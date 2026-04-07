//! Plugin signing using Ed25519 (hash-then-sign with SHA3-256)

use anyhow::{Context, Result};
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha3::{Digest, Sha3_256};

/// A plugin author's keypair for signing WASM modules
pub struct PluginKeyPair {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

impl PluginKeyPair {
    /// Generate a new random keypair
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut rand_core::OsRng);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    /// Get the public key as base64 string (for publishing to marketplace)
    #[must_use]
    pub fn public_key_base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.verifying_key.to_bytes())
    }

    /// Get the signing key as base64 string (for local storage)
    #[must_use]
    pub fn signing_key_base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.signing_key.to_bytes())
    }
}

/// Sign WASM module bytes
///
/// Returns base64-encoded Ed25519 signature of the SHA3-256 hash of the bytes.
pub fn sign_plugin(wasm_bytes: &[u8], keypair: &PluginKeyPair) -> String {
    let hash = Sha3_256::digest(wasm_bytes);
    let signature = keypair.signing_key.sign(&hash);
    base64::engine::general_purpose::STANDARD.encode(signature.to_bytes())
}

/// Verify a signed WASM module
///
/// `public_key_base64` is the base64-encoded Ed25519 public key.
/// `signature_base64` is the base64-encoded Ed25519 signature.
pub fn verify_plugin(
    wasm_bytes: &[u8],
    signature_base64: &str,
    public_key_base64: &str,
) -> Result<()> {
    let hash = Sha3_256::digest(wasm_bytes);

    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_base64)
        .context("Invalid base64 signature")?;
    let signature =
        Signature::from_slice(&sig_bytes).map_err(|e| anyhow::anyhow!("Invalid signature: {e}"))?;

    let pk_bytes = base64::engine::general_purpose::STANDARD
        .decode(public_key_base64)
        .context("Invalid base64 public key")?;
    let pk_array: [u8; 32] = pk_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid public key length: expected 32 bytes"))?;
    let verifying_key = VerifyingKey::from_bytes(&pk_array)
        .map_err(|e| anyhow::anyhow!("Invalid public key: {e}"))?;

    verifying_key
        .verify(&hash, &signature)
        .map_err(|e| anyhow::anyhow!("Signature verification failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_keypair() {
        let kp = PluginKeyPair::generate();
        let pub_b64 = kp.public_key_base64();
        let sec_b64 = kp.signing_key_base64();
        assert!(!pub_b64.is_empty());
        assert!(!sec_b64.is_empty());
        assert_ne!(pub_b64, sec_b64);

        let decoded: Vec<u8> = base64::engine::general_purpose::STANDARD
            .decode(&pub_b64)
            .unwrap();
        assert_eq!(decoded.len(), 32);
    }

    #[test]
    fn test_sign_and_verify_roundtrip() {
        let kp = PluginKeyPair::generate();
        let wasm = b"\x00asm\x01\x00\x00\x00";
        let sig = sign_plugin(wasm, &kp);
        let pub_key = kp.public_key_base64();
        assert!(verify_plugin(wasm, &sig, &pub_key).is_ok());
    }

    #[test]
    fn test_verify_wrong_key_fails() {
        let kp1 = PluginKeyPair::generate();
        let kp2 = PluginKeyPair::generate();
        let wasm = b"\x00asm\x01\x00\x00\x00";
        let sig = sign_plugin(wasm, &kp1);
        let wrong_key = kp2.public_key_base64();
        assert!(verify_plugin(wasm, &sig, &wrong_key).is_err());
    }

    #[test]
    fn test_verify_tampered_bytes_fails() {
        let kp = PluginKeyPair::generate();
        let wasm = b"\x00asm\x01\x00\x00\x00";
        let sig = sign_plugin(wasm, &kp);
        let pub_key = kp.public_key_base64();
        let tampered = b"\x00asm\x01\x00\x00\x01";
        assert!(verify_plugin(tampered, &sig, &pub_key).is_err());
    }

    #[test]
    fn test_verify_invalid_base64_fails() {
        let wasm = b"\x00asm\x01\x00\x00\x00";
        let result = verify_plugin(wasm, "!!!not-base64!!!", "also-not-base64");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("base64"), "error should mention base64: {msg}");
    }
}
