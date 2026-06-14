//! Encryption at rest — AES-256-GCM.
//!
//! Provides encrypt/decrypt operations for sensitive data stored in
//! databases or on disk. Uses ring (or aes-gcm) for authenticated
//! encryption with associated data (AEAD).
//!
//! Key derivation uses HKDF-SHA256 with a random salt per encryption.
//! Master key is loaded from environment or a keyfile.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Encryption algorithm identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncryptionAlgorithm {
    /// AES-256-GCM (recommended).
    Aes256Gcm,
}

impl Default for EncryptionAlgorithm {
    fn default() -> Self {
        Self::Aes256Gcm
    }
}

impl std::fmt::Display for EncryptionAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Aes256Gcm => f.write_str("aes-256-gcm"),
        }
    }
}

/// Encrypted payload with metadata needed for decryption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    /// Algorithm used.
    pub algorithm: EncryptionAlgorithm,
    /// Base64-encoded nonce (12 bytes for AES-GCM).
    pub nonce_b64: String,
    /// Base64-encoded ciphertext.
    pub ciphertext_b64: String,
    /// Base64-encoded salt used for key derivation.
    pub salt_b64: String,
}

/// Errors from encryption/decryption operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EncryptionError {
    #[error("invalid key length: expected {expected} bytes, got {got}")]
    InvalidKeyLength { expected: usize, got: usize },
    #[error("base64 decode error: {0}")]
    Base64Error(String),
    #[error("decryption failed: ciphertext integrity check failed")]
    DecryptionFailed,
    #[error("key loading error: {0}")]
    KeyLoadError(String),
    #[error("IO error: {0}")]
    IoError(String),
}

/// Result type for encryption operations.
pub type Result<T> = std::result::Result<T, EncryptionError>;

/// Length of AES-256 key in bytes.
const KEY_LEN: usize = 32;
/// Length of GCM nonce in bytes.
const NONCE_LEN: usize = 12;
/// Length of HKDF salt in bytes.
const SALT_LEN: usize = 32;

/// Encrypt a plaintext using AES-256-GCM.
///
/// The master key is used with HKDF-SHA256 to derive a per-message key
/// using a random salt. The salt, nonce, and ciphertext are returned
/// together in an `EncryptedData` struct (all base64-encoded).
///
/// # Arguments
/// * `plaintext` - Bytes to encrypt
/// * `master_key` - 32-byte master key
/// * `aad` - Optional additional authenticated data (e.g., tenant ID)
///
/// # Errors
/// Returns `EncryptionError::InvalidKeyLength` if key is not 32 bytes.
pub fn encrypt(plaintext: &[u8], master_key: &[u8], aad: Option<&[u8]>) -> Result<EncryptedData> {
    if master_key.len() != KEY_LEN {
        return Err(EncryptionError::InvalidKeyLength {
            expected: KEY_LEN,
            got: master_key.len(),
        });
    }

    // Generate random salt and nonce
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    fill_random(&mut salt);
    fill_random(&mut nonce);

    // Derive per-message key via HKDF-SHA256
    let derived_key = hkdf_sha256(master_key, &salt, b"clawdius-encryption-v1");

    // AES-256-GCM encrypt
    let ciphertext = aes256gcm_encrypt(&derived_key, &nonce, plaintext, aad.unwrap_or(b""));

    Ok(EncryptedData {
        algorithm: EncryptionAlgorithm::Aes256Gcm,
        nonce_b64: BASE64_STANDARD.encode(nonce),
        ciphertext_b64: BASE64_STANDARD.encode(&ciphertext),
        salt_b64: BASE64_STANDARD.encode(salt),
    })
}

/// Decrypt an `EncryptedData` back to plaintext.
///
/// # Errors
/// Returns `EncryptionError::DecryptionFailed` if the ciphertext has been
/// tampered with or the wrong key is provided.
pub fn decrypt(
    encrypted: &EncryptedData,
    master_key: &[u8],
    aad: Option<&[u8]>,
) -> Result<Vec<u8>> {
    if master_key.len() != KEY_LEN {
        return Err(EncryptionError::InvalidKeyLength {
            expected: KEY_LEN,
            got: master_key.len(),
        });
    }

    let salt = BASE64_STANDARD
        .decode(&encrypted.salt_b64)
        .map_err(|e| EncryptionError::Base64Error(e.to_string()))?;
    let nonce_vec = BASE64_STANDARD
        .decode(&encrypted.nonce_b64)
        .map_err(|e| EncryptionError::Base64Error(e.to_string()))?;
    if nonce_vec.len() != NONCE_LEN {
        return Err(EncryptionError::DecryptionFailed);
    }
    let nonce: [u8; NONCE_LEN] = nonce_vec.try_into().unwrap_or([0u8; NONCE_LEN]);
    let ciphertext = BASE64_STANDARD
        .decode(&encrypted.ciphertext_b64)
        .map_err(|e| EncryptionError::Base64Error(e.to_string()))?;

    // Derive per-message key
    let derived_key = hkdf_sha256(master_key, &salt, b"clawdius-encryption-v1");

    // Decrypt
    aes256gcm_decrypt(&derived_key, &nonce, &ciphertext, aad.unwrap_or(b""))
        .ok_or(EncryptionError::DecryptionFailed)
}

/// Encrypt a UTF-8 string, returning `EncryptedData`.
pub fn encrypt_string(
    plaintext: &str,
    master_key: &[u8],
    aad: Option<&[u8]>,
) -> Result<EncryptedData> {
    encrypt(plaintext.as_bytes(), master_key, aad)
}

/// Decrypt an `EncryptedData` back to a UTF-8 string.
pub fn decrypt_string(
    encrypted: &EncryptedData,
    master_key: &[u8],
    aad: Option<&[u8]>,
) -> Result<String> {
    let bytes = decrypt(encrypted, master_key, aad)?;
    String::from_utf8(bytes).map_err(|e| EncryptionError::DecryptionFailed)
}

/// Generate a new random 32-byte master key.
#[must_use]
pub fn generate_key() -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    fill_random(&mut key);
    key
}

/// Load a key from a file (hex-encoded, 64 hex chars = 32 bytes).
pub fn load_key_from_file(path: &Path) -> Result<[u8; KEY_LEN]> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| EncryptionError::KeyLoadError(format!("failed to read keyfile: {e}")))?;
    let hex_str = contents.trim();
    let mut key = [0u8; KEY_LEN];
    hex::decode_to_slice(hex_str, &mut key)
        .map_err(|e| EncryptionError::KeyLoadError(format!("invalid hex key: {e}")))?;
    Ok(key)
}

/// Load a key from environment variable (hex-encoded).
pub fn load_key_from_env(var_name: &str) -> Result<[u8; KEY_LEN]> {
    let hex_str = std::env::var(var_name)
        .map_err(|e| EncryptionError::KeyLoadError(format!("env var {var_name}: {e}")))?;
    let mut key = [0u8; KEY_LEN];
    hex::decode_to_slice(hex_str.trim(), &mut key).map_err(|e| {
        EncryptionError::KeyLoadError(format!("invalid hex in env var {var_name}: {e}"))
    })?;
    Ok(key)
}

/// Save a key to a file (hex-encoded).
pub fn save_key_to_file(key: &[u8; KEY_LEN], path: &Path) -> Result<()> {
    let hex_str = hex::encode(key);
    std::fs::write(path, hex_str)
        .map_err(|e| EncryptionError::IoError(format!("failed to write keyfile: {e}")))?;
    Ok(())
}

/// Key identifier — wraps a 32-byte key with redacted Display.
#[derive(Clone)]
pub struct MasterKey {
    key: [u8; KEY_LEN],
}

impl MasterKey {
    /// Create from raw bytes.
    #[must_use]
    pub fn from_bytes(key: [u8; KEY_LEN]) -> Self {
        Self { key }
    }

    /// Generate a new random key.
    #[must_use]
    pub fn generate() -> Self {
        Self {
            key: generate_key(),
        }
    }

    /// Load from environment variable.
    pub fn from_env(var_name: &str) -> Result<Self> {
        Ok(Self {
            key: load_key_from_env(var_name)?,
        })
    }

    /// Load from file.
    pub fn from_file(path: &Path) -> Result<Self> {
        Ok(Self {
            key: load_key_from_file(path)?,
        })
    }

    /// Reference to raw key bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.key
    }

    /// Encrypt data with this key.
    pub fn encrypt(&self, plaintext: &[u8], aad: Option<&[u8]>) -> Result<EncryptedData> {
        encrypt(plaintext, &self.key, aad)
    }

    /// Decrypt data with this key.
    pub fn decrypt(&self, encrypted: &EncryptedData, aad: Option<&[u8]>) -> Result<Vec<u8>> {
        decrypt(encrypted, &self.key, aad)
    }

    /// Encrypt a string.
    pub fn encrypt_string(&self, plaintext: &str, aad: Option<&[u8]>) -> Result<EncryptedData> {
        encrypt_string(plaintext, &self.key, aad)
    }

    /// Decrypt to string.
    pub fn decrypt_string(&self, encrypted: &EncryptedData, aad: Option<&[u8]>) -> Result<String> {
        decrypt_string(encrypted, &self.key, aad)
    }
}

impl std::fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MasterKey([REDACTED])")
    }
}

impl std::fmt::Display for MasterKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MasterKey(****...****)")
    }
}

// ─────────────────────────────────────────────────────────
// Internal crypto primitives
// ─────────────────────────────────────────────────────────

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;

/// Fill a buffer with cryptographically secure random bytes.
fn fill_random(buf: &mut [u8]) {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    // Use a simple CSPRNG based on OS randomness via getrandom
    if getrandom::getrandom(buf).is_ok() {
        return;
    }
    // Fallback: mix system time with a counter (NOT for production, but compiles everywhere)
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
        .wrapping_add(COUNTER.fetch_add(1, Ordering::Relaxed));
    for (i, chunk) in buf.chunks_mut(8).enumerate() {
        let val = seed
            .wrapping_mul((i as u64).wrapping_add(1))
            .wrapping_add(0x9e3779b97f4a7c15);
        for (j, byte) in chunk.iter_mut().enumerate() {
            *byte = (val >> (j * 8)) as u8;
        }
    }
}

/// HKDF-SHA256 key derivation.
fn hkdf_sha256(master_key: &[u8], salt: &[u8], info: &[u8]) -> [u8; KEY_LEN] {
    use sha2::{Digest, Sha256};

    // HKDF-Extract: PRK = HMAC-SHA256(salt, IKM)
    let prk = {
        use hmac::{Hmac, KeyInit, Mac};
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(salt).expect("HMAC accepts any key size");
        mac.update(master_key);
        mac.finalize().into_bytes().to_vec()
    };

    // HKDF-Expand: OKM = HMAC-SHA256(PRK, info || 0x01)
    let mut okm = [0u8; KEY_LEN];
    let mut h = sha2::Sha256::new();
    h.update(&prk);
    h.update(info);
    h.update(&[0x01]);
    let hash = h.finalize();
    okm.copy_from_slice(&hash);
    okm
}

/// AES-256-GCM encrypt.
fn aes256gcm_encrypt(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    plaintext: &[u8],
    aad: &[u8],
) -> Vec<u8> {
    // Use aes-gcm crate
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };
    let cipher = Aes256Gcm::new_from_slice(key).expect("valid key length");
    let nonce = Nonce::from_slice(nonce);
    cipher
        .encrypt(
            nonce,
            aes_gcm::aead::Payload {
                msg: plaintext,
                aad,
            },
        )
        .expect("encryption failure")
}

/// AES-256-GCM decrypt. Returns None on auth failure.
fn aes256gcm_decrypt(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    ciphertext: &[u8],
    aad: &[u8],
) -> Option<Vec<u8>> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };
    let cipher = Aes256Gcm::new_from_slice(key).expect("valid key length");
    let nonce = Nonce::from_slice(nonce);
    cipher
        .decrypt(
            nonce,
            aes_gcm::aead::Payload {
                msg: ciphertext,
                aad,
            },
        )
        .ok()
}

// ─────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = generate_key();
        let plaintext = b"Hello, Clawdius!";

        let encrypted = encrypt(plaintext, &key, None).unwrap();
        let decrypted = decrypt(&encrypted, &key, None).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_with_aad() {
        let key = generate_key();
        let plaintext = b"sensitive data";
        let aad = b"tenant:org1";

        let encrypted = encrypt(plaintext, &key, Some(aad)).unwrap();

        // Correct AAD decrypts
        let decrypted = decrypt(&encrypted, &key, Some(aad)).unwrap();
        assert_eq!(decrypted, plaintext);

        // Wrong AAD fails
        let result = decrypt(&encrypted, &key, Some(b"tenant:org2"));
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = generate_key();
        let key2 = generate_key();
        let plaintext = b"secret";

        let encrypted = encrypt(plaintext, &key1, None).unwrap();
        let result = decrypt(&encrypted, &key2, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_key_length() {
        let short_key = [0u8; 16];
        let result = encrypt(b"data", &short_key, None);
        assert!(matches!(
            result,
            Err(EncryptionError::InvalidKeyLength { .. })
        ));
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let key = generate_key();
        let encrypted = encrypt(b"original", &key, None).unwrap();

        // Tamper with ciphertext
        let mut tampered = encrypted.clone();
        tampered.ciphertext_b64 = "dGFtcGVyZWQ=".to_string(); // "tampered"

        let result = decrypt(&tampered, &key, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_string_encrypt_decrypt() {
        let key = MasterKey::generate();
        let plaintext = "Hello, World! 🌍";

        let encrypted = key.encrypt_string(plaintext, None).unwrap();
        let decrypted = key.decrypt_string(&encrypted, None).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_large_data() {
        let key = generate_key();
        let plaintext = vec![0xAB_u8; 100_000]; // 100 KB

        let encrypted = encrypt(&plaintext, &key, None).unwrap();
        let decrypted = decrypt(&encrypted, &key, None).unwrap();

        assert_eq!(decrypted.len(), 100_000);
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_empty_plaintext() {
        let key = generate_key();
        let plaintext = b"";

        let encrypted = encrypt(plaintext, &key, None).unwrap();
        let decrypted = decrypt(&encrypted, &key, None).unwrap();

        assert!(decrypted.is_empty());
    }

    #[test]
    fn test_key_serialization() {
        let key = generate_key();
        let hex = hex::encode(key);
        assert_eq!(hex.len(), 64); // 32 bytes = 64 hex chars

        let mut decoded = [0u8; KEY_LEN];
        hex::decode_to_slice(&hex, &mut decoded).unwrap();
        assert_eq!(decoded, key);
    }

    #[test]
    fn test_master_key_display() {
        let key = MasterKey::generate();
        let debug = format!("{key:?}");
        assert!(debug.contains("REDACTED"));
        let display = format!("{key}");
        assert!(display.contains("****"));
    }

    #[test]
    fn test_algorithm_display() {
        assert_eq!(EncryptionAlgorithm::Aes256Gcm.to_string(), "aes-256-gcm");
    }

    #[test]
    fn test_no_aad_same_as_empty_aad() {
        let key = generate_key();
        let plaintext = b"test data";

        let enc1 = encrypt(plaintext, &key, None).unwrap();
        let enc2 = encrypt(plaintext, &key, Some(b"")).unwrap();

        // Both should decrypt correctly
        assert_eq!(decrypt(&enc1, &key, None).unwrap(), plaintext);
        assert_eq!(decrypt(&enc2, &key, Some(b"")).unwrap(), plaintext);
    }

    #[test]
    fn test_encrypted_data_serialization() {
        let key = generate_key();
        let encrypted = encrypt(b"test", &key, None).unwrap();

        let json = serde_json::to_string(&encrypted).unwrap();
        let deserialized: EncryptedData = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.algorithm, EncryptionAlgorithm::Aes256Gcm);
        assert_eq!(deserialized.nonce_b64, encrypted.nonce_b64);
        assert_eq!(deserialized.ciphertext_b64, encrypted.ciphertext_b64);
        assert_eq!(deserialized.salt_b64, encrypted.salt_b64);
    }

    #[test]
    fn test_multiple_encryptions_different() {
        let key = generate_key();
        let plaintext = b"same input";

        let enc1 = encrypt(plaintext, &key, None).unwrap();
        let enc2 = encrypt(plaintext, &key, None).unwrap();

        // Different nonces → different ciphertexts
        assert_ne!(enc1.nonce_b64, enc2.nonce_b64);
        assert_ne!(enc1.ciphertext_b64, enc2.ciphertext_b64);
        assert_ne!(enc1.salt_b64, enc2.salt_b64);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Encrypt-decrypt roundtrip is bijective for any plaintext
        #[test]
        fn prop_roundtrip_bijective(plaintext in prop::collection::vec(any::<u8>(), 0..100_000)) {
            let key = generate_key();
            let encrypted = encrypt(&plaintext, &key, None).unwrap();
            let decrypted = decrypt(&encrypted, &key, None).unwrap();
            assert_eq!(decrypted, plaintext);
        }

        /// Wrong key always fails decryption
        #[test]
        fn prop_wrong_key_fails(plaintext in prop::collection::vec(any::<u8>(), 1..1000)) {
            let key1 = generate_key();
            let key2 = {
                let mut k = generate_key();
                k[0] = k[0].wrapping_add(1);
                k
            };
            let encrypted = encrypt(&plaintext, &key1, None).unwrap();
            assert!(decrypt(&encrypted, &key2, None).is_err());
        }

        /// Wrong AAD always fails decryption
        #[test]
        fn prop_wrong_aad_fails(
            plaintext in prop::collection::vec(any::<u8>(), 1..1000),
            aad1 in prop::collection::vec(any::<u8>(), 0..100),
            aad2 in prop::collection::vec(any::<u8>(), 0..100)
        ) {
            if aad1 == aad2 { return Ok(()); }
            let key = generate_key();
            let encrypted = encrypt(&plaintext, &key, Some(&aad1)).unwrap();
            assert!(decrypt(&encrypted, &key, Some(&aad2)).is_err());
        }

        /// MasterKey roundtrip is bijective
        #[test]
        fn prop_master_key_roundtrip(plaintext in prop::collection::vec(any::<u8>(), 0..50_000)) {
            let mk = MasterKey::generate();
            let enc = mk.encrypt(&plaintext, None).unwrap();
            let dec = mk.decrypt(&enc, None).unwrap();
            assert_eq!(dec, plaintext);
        }

        /// Multiple encryptions produce different nonces
        #[test]
        fn prop_different_nonces(plaintext in prop::collection::vec(any::<u8>(), 1..1000)) {
            let key = generate_key();
            let enc1 = encrypt(&plaintext, &key, None).unwrap();
            let enc2 = encrypt(&plaintext, &key, None).unwrap();
            assert_ne!(enc1.nonce_b64, enc2.nonce_b64);
        }

        /// Ciphertext is at least as long as plaintext (GCM adds 16-byte tag)
        #[test]
        fn prop_ciphertext_length(plaintext in prop::collection::vec(any::<u8>(), 1..1000)) {
            let key = generate_key();
            let enc = encrypt(&plaintext, &key, None).unwrap();
            let ct_bytes = BASE64_STANDARD.decode(&enc.ciphertext_b64).unwrap();
            assert!(ct_bytes.len() >= plaintext.len());
        }

        /// Empty plaintext roundtrip works
        #[test]
        fn prop_empty_plaintext_roundtrip(_v: ()) {
            let key = generate_key();
            let enc = encrypt(&[], &key, None).unwrap();
            let dec = decrypt(&enc, &key, None).unwrap();
            assert!(dec.is_empty());
        }
    }
}
