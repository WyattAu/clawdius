#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    clippy::useless_format
)]
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// 1. HTML Compressor Properties
// ---------------------------------------------------------------------------

mod html_compressor {
    use super::*;
    use clawdius_core::tools::html_compressor::{BatchCompressor, HtmlCompressor};

    proptest! {
        #[test]
        fn compress_produces_shorter_or_equal_output(html in "[a-z ]{1,1000}") {
            let compressor = HtmlCompressor::new();
            if let Ok(result) = compressor.compress(&html) {
                prop_assert!(result.compressed_bytes <= result.original_bytes);
            }
        }

        #[test]
        fn compress_ratio_is_between_zero_and_one(html in "[a-z ]{1,500}") {
            let compressor = HtmlCompressor::new();
            if let Ok(result) = compressor.compress(&html) {
                prop_assert!(result.ratio >= 0.0);
                prop_assert!(result.ratio <= 1.0);
            }
        }

        #[test]
        fn batch_compression_preserves_individual_results(pages in proptest::collection::vec("[a-z ]{1,200}", 0..10)) {
            let batch = BatchCompressor::new();
            let pairs: Vec<(String, &str)> = pages.iter().map(|s| (format!("http://example.com"), s.as_str())).collect();
            let results = batch.compress_batch(&pairs);
            prop_assert_eq!(results.len(), pages.len());
        }
    }
}

// ---------------------------------------------------------------------------
// 2. Config Roundtrip Properties
// ---------------------------------------------------------------------------

mod config_roundtrip {
    use super::*;

    proptest! {
        #[test]
        fn config_save_load_roundtrip(name in "[a-z]{1,20}") {
            let mut config = clawdius_core::config::Config::default();
            config.project.name = name;
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("config.toml");
            config.save(&path).unwrap();
            let loaded = clawdius_core::config::Config::load(&path).unwrap();
            assert_eq!(loaded.project.name, config.project.name);
        }
    }
}

// ---------------------------------------------------------------------------
// 3. Encryption Roundtrip Properties
// ---------------------------------------------------------------------------

mod encryption_roundtrip {
    use super::*;

    proptest! {
        #[test]
        fn encrypt_decrypt_roundtrip(plaintext in ".{0,1000}") {
            let key = clawdius_core::encryption::MasterKey::generate();
            let encrypted = key.encrypt(plaintext.as_bytes(), None).unwrap();
            let decrypted = key.decrypt(&encrypted, None).unwrap();
            assert_eq!(decrypted, plaintext.as_bytes());
        }

        #[test]
        fn wrong_key_fails(plaintext in ".{1,100}") {
            let key1 = clawdius_core::encryption::MasterKey::generate();
            let key2 = clawdius_core::encryption::MasterKey::generate();
            let encrypted = key1.encrypt(plaintext.as_bytes(), None).unwrap();
            assert!(key2.decrypt(&encrypted, None).is_err());
        }
    }
}
