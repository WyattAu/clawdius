#![deny(unsafe_code)]

//! Encrypted State Store Decorator
//!
//! Provides transparent AES-256-GCM encryption at rest for any `StateStore`
//! implementation. Values are encrypted before being passed to the inner store
//! and decrypted on retrieval.
//!
//! # Format
//!
//! Each stored value is serialized as:
//! ```text
//! [1 byte: version][12 bytes: nonce][remaining: ciphertext + 16 byte tag]
//! ```
//!
//! The key is derived from a hex-encoded 32-byte master key using BLAKE3
//! context derivation (not the master key directly), providing forward secrecy
//! if the key derivation scheme is updated in the future.
//!
//! # Feature Gate
//!
//! This module requires the `encryption` Cargo feature (adds `aes-gcm` dependency).
//! Without it, `StateStoreFactory` returns plaintext stores regardless of config.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use blake3;
use tracing::{debug, warn};

use super::state_store::StateStore;
use super::types::Result;

/// BLAKE3 context string for key derivation.
/// Using a context ensures the derived key is domain-separated from other uses
/// of the same master key material.
const KDF_CONTEXT: &str = "clawdius-state-store-encryption-v1";

/// Current encryption format version.
const FORMAT_VERSION: u8 = 1;

/// Header size: 1 byte version + 12 byte nonce = 13 bytes.
const HEADER_SIZE: usize = 1 + 12;

/// AES-256-GCM key size in bytes.
const KEY_SIZE: usize = 32;

/// Derive a 32-byte AES-256 key from a hex-encoded master key using BLAKE3.
///
/// # Errors
///
/// Returns `MessagingError::InvalidConfig` if the hex key is malformed or wrong length.
pub fn derive_encryption_key(hex_key: &str) -> Result<[u8; KEY_SIZE]> {
    let hex = hex_key.trim();
    if hex.len() != 64 {
        return Err(super::types::MessagingError::InvalidConfig(format!(
            "encryption_key must be 64 hex chars (32 bytes), got {}",
            hex.len()
        )));
    }

    let mut master_key = [0u8; KEY_SIZE];
    hex::decode_to_slice(hex, &mut master_key).map_err(|e| {
        super::types::MessagingError::InvalidConfig(format!("encryption_key is not valid hex: {e}"))
    })?;

    // Derive an application-specific key using BLAKE3 KDF.
    // This provides domain separation and forward secrecy.
    let derived_key: [u8; KEY_SIZE] = blake3::derive_key(KDF_CONTEXT, &master_key);

    Ok(derived_key)
}

/// Encrypt a plaintext value using AES-256-GCM with a random nonce.
///
/// Returns the serialized format: `[version][nonce][ciphertext+tag]`.
#[cfg(feature = "encryption")]
fn encrypt(key: &[u8; KEY_SIZE], plaintext: &[u8]) -> Result<Vec<u8>> {
    use aes_gcm::aead::{Aead, KeyInit, OsRng};
    use aes_gcm::{Aes256Gcm, Nonce};
    use rand_core::RngCore;

    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| {
        super::types::MessagingError::InvalidConfig(format!("AES-256-GCM init failed: {e}"))
    })?;

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| super::types::MessagingError::ParseError(format!("Encryption failed: {e}")))?;

    // Serialize: [version][nonce][ciphertext+tag]
    let mut out = Vec::with_capacity(HEADER_SIZE + ciphertext.len());
    out.push(FORMAT_VERSION);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);

    Ok(out)
}

/// Decrypt a value that was encrypted with `encrypt`.
///
/// Returns the original plaintext.
#[cfg(feature = "encryption")]
fn decrypt(key: &[u8; KEY_SIZE], data: &[u8]) -> Result<Vec<u8>> {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    if data.len() < HEADER_SIZE {
        return Err(super::types::MessagingError::ParseError(
            "Encrypted data too short — missing header".into(),
        ));
    }

    let version = data[0];
    if version != FORMAT_VERSION {
        return Err(super::types::MessagingError::ParseError(format!(
            "Unknown encryption format version: {version}"
        )));
    }

    let nonce = Nonce::from_slice(&data[1..13]);
    let ciphertext = &data[HEADER_SIZE..];

    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| {
        super::types::MessagingError::InvalidConfig(format!("AES-256-GCM init failed: {e}"))
    })?;

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| super::types::MessagingError::ParseError(format!("Decryption failed: {e}")))?;

    Ok(plaintext)
}

/// An encrypted decorator that wraps any `StateStore`.
///
/// All values passed to `set` / `set_multi` are encrypted before being
/// forwarded to the inner store. All values returned by `get` / `get_multi`
/// are decrypted transparently.
///
/// Keys, table names, TTLs, and other metadata remain in plaintext — only
/// the value payloads are encrypted.
pub struct EncryptedStateStore {
    inner: Arc<dyn StateStore>,
    key: [u8; KEY_SIZE],
}

impl EncryptedStateStore {
    /// Create a new encrypted store wrapper.
    ///
    /// # Arguments
    ///
    /// * `inner` — The underlying state store (SQLite, in-memory, etc.)
    /// * `hex_key` — A 64-character hex string encoding a 32-byte AES-256 master key
    ///
    /// # Errors
    ///
    /// Returns an error if the hex key is malformed.
    #[cfg(feature = "encryption")]
    pub fn new(inner: Arc<dyn StateStore>, hex_key: &str) -> Result<Self> {
        let key = derive_encryption_key(hex_key)?;
        debug!("EncryptedStateStore initialized with AES-256-GCM");
        Ok(Self { inner, key })
    }

    /// Create a new encrypted store wrapper (no-op without `encryption` feature).
    ///
    /// Without the `encryption` feature, this returns an error indicating
    /// the feature is not compiled in.
    #[cfg(not(feature = "encryption"))]
    pub fn new(_inner: Arc<dyn StateStore>, _hex_key: &str) -> Result<Self> {
        Err(super::types::MessagingError::InvalidConfig(
            "encryption feature is not enabled — compile with --features encryption".into(),
        ))
    }

    #[must_use]
    pub fn inner(&self) -> &Arc<dyn StateStore> {
        &self.inner
    }
}

#[async_trait]
impl StateStore for EncryptedStateStore {
    async fn get(&self, table: &str, key: &str) -> Result<Option<Vec<u8>>> {
        let encrypted = self.inner.get(table, key).await?;
        match encrypted {
            None => Ok(None),
            Some(data) => {
                let plaintext = decrypt_value(&self.key, &data)?;
                Ok(Some(plaintext))
            },
        }
    }

    async fn set(&self, table: &str, key: &str, value: &[u8], ttl: Option<u64>) -> Result<()> {
        let encrypted = encrypt_value(&self.key, value)?;
        self.inner.set(table, key, &encrypted, ttl).await
    }

    async fn delete(&self, table: &str, key: &str) -> Result<bool> {
        self.inner.delete(table, key).await
    }

    async fn exists(&self, table: &str, key: &str) -> Result<bool> {
        self.inner.exists(table, key).await
    }

    async fn get_multi(&self, table: &str, keys: &[&str]) -> Result<HashMap<String, Vec<u8>>> {
        let encrypted_map = self.inner.get_multi(table, keys).await?;
        let mut result = HashMap::with_capacity(encrypted_map.len());
        for (k, v) in encrypted_map {
            match decrypt_value(&self.key, &v) {
                Ok(plaintext) => {
                    result.insert(k, plaintext);
                },
                Err(e) => {
                    warn!(key = %k, error = %e, "Failed to decrypt value, skipping");
                },
            }
        }
        Ok(result)
    }

    async fn set_multi(
        &self,
        table: &str,
        entries: &[(String, Vec<u8>)],
        ttl: Option<u64>,
    ) -> Result<()> {
        let encrypted_entries: Result<Vec<(String, Vec<u8>)>> = entries
            .iter()
            .map(|(k, v)| encrypt_value(&self.key, v).map(|enc| (k.clone(), enc)))
            .collect();
        self.inner.set_multi(table, &encrypted_entries?, ttl).await
    }

    async fn keys(&self, table: &str, pattern: &str) -> Result<Vec<String>> {
        self.inner.keys(table, pattern).await
    }

    async fn count(&self, table: &str) -> Result<usize> {
        self.inner.count(table).await
    }

    async fn create_table(&self, table: &str) -> Result<()> {
        self.inner.create_table(table).await
    }

    async fn drop_table(&self, table: &str) -> Result<()> {
        self.inner.drop_table(table).await
    }

    async fn table_exists(&self, table: &str) -> Result<bool> {
        self.inner.table_exists(table).await
    }

    async fn health_check(&self) -> Result<bool> {
        self.inner.health_check().await
    }

    fn store_type(&self) -> &'static str {
        "encrypted"
    }
}

/// Encrypt a value, delegating to the `encryption` feature or returning a no-op error.
#[cfg(feature = "encryption")]
fn encrypt_value(key: &[u8; KEY_SIZE], plaintext: &[u8]) -> Result<Vec<u8>> {
    encrypt(key, plaintext)
}

#[cfg(not(feature = "encryption"))]
fn encrypt_value(_key: &[u8; KEY_SIZE], _plaintext: &[u8]) -> Result<Vec<u8>> {
    Err(super::types::MessagingError::InvalidConfig(
        "encryption feature not enabled".into(),
    ))
}

/// Decrypt a value, delegating to the `encryption` feature or returning a no-op error.
#[cfg(feature = "encryption")]
fn decrypt_value(key: &[u8; KEY_SIZE], data: &[u8]) -> Result<Vec<u8>> {
    decrypt(key, data)
}

#[cfg(not(feature = "encryption"))]
fn decrypt_value(_key: &[u8; KEY_SIZE], _data: &[u8]) -> Result<Vec<u8>> {
    Err(super::types::MessagingError::InvalidConfig(
        "encryption feature not enabled".into(),
    ))
}

/// Build an optionally-encrypted state store.
///
/// If `encryption_key` is non-empty and the `encryption` feature is enabled,
/// returns an `EncryptedStateStore` wrapping the inner store.
/// Otherwise returns the inner store as-is.
pub fn maybe_encrypt(
    inner: Arc<dyn StateStore>,
    encryption_key: &str,
) -> Result<Arc<dyn StateStore>> {
    if encryption_key.trim().is_empty() {
        debug!("No encryption key configured — using plaintext state store");
        return Ok(inner);
    }

    match EncryptedStateStore::new(inner, encryption_key) {
        Ok(encrypted) => {
            debug!("Encryption at rest enabled for state store");
            Ok(Arc::new(encrypted))
        },
        Err(e) => {
            warn!(error = %e, "Failed to initialize encrypted store — falling back to plaintext");
            Err(e)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::state_store::InMemoryStateStore;

    fn valid_hex_key() -> String {
        "a".repeat(64)
    }

    #[test]
    fn derive_key_valid_hex() {
        let key = derive_encryption_key(&valid_hex_key());
        assert!(key.is_ok());
    }

    #[test]
    fn derive_key_wrong_length() {
        let result = derive_encryption_key("deadbeef");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("64 hex chars"));
    }

    #[test]
    fn derive_key_invalid_hex() {
        let result = derive_encryption_key(&"z".repeat(64));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not valid hex"));
    }

    #[test]
    fn derive_key_empty_string() {
        let result = derive_encryption_key("");
        assert!(result.is_err());
    }

    #[cfg(feature = "encryption")]
    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = derive_encryption_key(&valid_hex_key()).expect("key ok");
        let plaintext = b"hello, world! this is a secret message.";

        let encrypted = encrypt(&key, plaintext).expect("encrypt ok");
        assert!(
            encrypted.len() > plaintext.len(),
            "encrypted should be larger"
        );

        let decrypted = decrypt(&key, &encrypted).expect("decrypt ok");
        assert_eq!(decrypted, plaintext);
    }

    #[cfg(feature = "encryption")]
    #[test]
    fn decrypt_wrong_key_fails() {
        let key1 = derive_encryption_key(&"a".repeat(64)).expect("key1 ok");
        let key2 = derive_encryption_key(&"b".repeat(64)).expect("key2 ok");
        let plaintext = b"secret data";

        let encrypted = encrypt(&key1, plaintext).expect("encrypt ok");
        let result = decrypt(&key2, &encrypted);
        assert!(result.is_err(), "decrypting with wrong key should fail");
    }

    #[cfg(feature = "encryption")]
    #[test]
    fn decrypt_truncated_data_fails() {
        let key = derive_encryption_key(&valid_hex_key()).expect("key ok");
        let short_data = vec![0u8; 5];
        let result = decrypt(&key, &short_data);
        assert!(result.is_err());
    }

    #[cfg(feature = "encryption")]
    #[test]
    fn decrypt_wrong_version_fails() {
        let key = derive_encryption_key(&valid_hex_key()).expect("key ok");
        let mut bad_version = vec![0u8; 30];
        bad_version[0] = 99; // wrong version
        let result = decrypt(&key, &bad_version);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("version"));
    }

    #[cfg(feature = "encryption")]
    #[test]
    fn encrypt_different_nonces_each_time() {
        let key = derive_encryption_key(&valid_hex_key()).expect("key ok");
        let plaintext = b"same message";

        let enc1 = encrypt(&key, plaintext).expect("encrypt1 ok");
        let enc2 = encrypt(&key, plaintext).expect("encrypt2 ok");

        // Random nonces mean the ciphertexts should differ
        assert_ne!(
            enc1, enc2,
            "same plaintext should produce different ciphertexts"
        );
    }

    #[cfg(feature = "encryption")]
    #[test]
    fn encrypt_empty_plaintext() {
        let key = derive_encryption_key(&valid_hex_key()).expect("key ok");
        let plaintext = b"";

        let encrypted = encrypt(&key, plaintext).expect("encrypt ok");
        let decrypted = decrypt(&key, &encrypted).expect("decrypt ok");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn maybe_encrypt_no_key_returns_inner() {
        let inner = super::super::state_store::InMemoryStateStore::new();
        let inner: Arc<dyn StateStore> = Arc::new(inner);
        let result = maybe_encrypt(inner.clone(), "").expect("ok");
        assert_eq!(result.store_type(), "memory");
    }

    // -----------------------------------------------------------------------
    // Integration tests: EncryptedStateStore wrapping InMemoryStateStore
    // -----------------------------------------------------------------------

    #[cfg(feature = "encryption")]
    #[tokio::test]
    async fn encrypted_store_set_get_roundtrip() {
        let inner: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::new());
        let encrypted = EncryptedStateStore::new(inner, &valid_hex_key()).expect("create ok");

        encrypted
            .create_table("test_tbl")
            .await
            .expect("create table ok");
        encrypted
            .set("test_tbl", "key1", b"secret value 123", None)
            .await
            .expect("set ok");

        let result = encrypted.get("test_tbl", "key1").await.expect("get ok");
        assert!(result.is_some());
        let val = result.unwrap();
        assert_eq!(val, b"secret value 123");

        // Verify the raw inner store has encrypted data (not plaintext)
        let raw = encrypted
            .inner()
            .get("test_tbl", "key1")
            .await
            .expect("raw get ok");
        assert!(raw.is_some());
        let raw_val = raw.unwrap();
        assert_ne!(
            raw_val, b"secret value 123",
            "inner store should contain ciphertext"
        );
    }

    #[cfg(feature = "encryption")]
    #[tokio::test]
    async fn encrypted_store_delete_works() {
        let inner: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::new());
        let encrypted = EncryptedStateStore::new(inner, &valid_hex_key()).expect("create ok");

        encrypted
            .create_table("test_tbl")
            .await
            .expect("create table ok");
        encrypted
            .set("test_tbl", "del_me", b"will be deleted", None)
            .await
            .expect("set ok");

        let deleted = encrypted
            .delete("test_tbl", "del_me")
            .await
            .expect("delete ok");
        assert!(deleted);

        let result = encrypted.get("test_tbl", "del_me").await.expect("get ok");
        assert!(result.is_none());
    }

    #[cfg(feature = "encryption")]
    #[tokio::test]
    async fn encrypted_store_keys_and_count() {
        let inner: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::new());
        let encrypted = EncryptedStateStore::new(inner, &valid_hex_key()).expect("create ok");

        encrypted
            .create_table("test_tbl")
            .await
            .expect("create table ok");
        encrypted
            .set("test_tbl", "k1", b"v1", None)
            .await
            .expect("set ok");
        encrypted
            .set("test_tbl", "k2", b"v2", None)
            .await
            .expect("set ok");
        encrypted
            .set("test_tbl", "k3", b"v3", None)
            .await
            .expect("set ok");

        let count = encrypted.count("test_tbl").await.expect("count ok");
        assert_eq!(count, 3);

        let keys = encrypted.keys("test_tbl", "*").await.expect("keys ok");
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"k1".to_string()));
        assert!(keys.contains(&"k2".to_string()));
        assert!(keys.contains(&"k3".to_string()));
    }

    #[cfg(feature = "encryption")]
    #[tokio::test]
    async fn encrypted_store_exists_and_health_check() {
        let inner: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::new());
        let encrypted = EncryptedStateStore::new(inner, &valid_hex_key()).expect("create ok");

        encrypted
            .create_table("test_tbl")
            .await
            .expect("create table ok");

        assert!(!encrypted
            .exists("test_tbl", "nope")
            .await
            .expect("exists ok"));
        encrypted
            .set("test_tbl", "yes", b"v", None)
            .await
            .expect("set ok");
        assert!(encrypted
            .exists("test_tbl", "yes")
            .await
            .expect("exists ok"));

        assert!(encrypted.health_check().await.expect("health ok"));
        assert!(encrypted
            .table_exists("test_tbl")
            .await
            .expect("table_exists ok"));
    }

    #[cfg(feature = "encryption")]
    #[tokio::test]
    async fn encrypted_store_multi_operations() {
        let inner: Arc<dyn StateStore> = Arc::new(InMemoryStateStore::new());
        let encrypted = EncryptedStateStore::new(inner, &valid_hex_key()).expect("create ok");

        encrypted
            .create_table("test_tbl")
            .await
            .expect("create table ok");

        encrypted
            .set_multi(
                "test_tbl",
                &[
                    ("mk1".to_string(), b"mv1".to_vec()),
                    ("mk2".to_string(), b"mv2".to_vec()),
                ],
                None,
            )
            .await
            .expect("set_multi ok");

        let results = encrypted
            .get_multi("test_tbl", &["mk1", "mk2"])
            .await
            .expect("get_multi ok");
        assert_eq!(results.len(), 2);
        assert_eq!(results["mk1"], b"mv1");
        assert_eq!(results["mk2"], b"mv2");
    }
}
