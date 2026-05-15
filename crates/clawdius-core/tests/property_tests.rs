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

// ---------------------------------------------------------------------------
// 4. Token Counting Properties
// ---------------------------------------------------------------------------

mod token_counting {
    use super::*;
    use clawdius_core::tokenize::{count_tokens, TokenizerStrategy};

    proptest! {
        #[test]
        fn empty_string_always_zero(strategy in prop_oneof![
            Just(TokenizerStrategy::Simple),
            Just(TokenizerStrategy::Code),
            Just(TokenizerStrategy::Natural),
            Just(TokenizerStrategy::BpeApproximation),
        ]) {
            prop_assert_eq!(count_tokens("", strategy), 0);
        }

        #[test]
        fn count_is_non_negative(text in ".{0,1000}", strategy in prop_oneof![
            Just(TokenizerStrategy::Simple),
            Just(TokenizerStrategy::Code),
            Just(TokenizerStrategy::Natural),
            Just(TokenizerStrategy::BpeApproximation),
        ]) {
            let _count = count_tokens(&text, strategy);
        }

        #[test]
        fn splitting_text_counting_parts_ge_total(text in "[a-zA-Z ]{1,500}") {
            let total = count_tokens(&text, TokenizerStrategy::Simple);
            let parts: Vec<&str> = text.split_whitespace().collect();
            if parts.len() > 1 {
                let mid = parts.len() / 2;
                let left: String = parts[..mid].join(" ");
                let right: String = parts[mid..].join(" ");
                let sum = count_tokens(&left, TokenizerStrategy::Simple)
                        + count_tokens(&right, TokenizerStrategy::Simple);
                prop_assert!(sum >= total);
            }
        }

        #[test]
        fn idempotency_same_text_same_count(text in ".{0,500}", strategy in prop_oneof![
            Just(TokenizerStrategy::Simple),
            Just(TokenizerStrategy::Code),
            Just(TokenizerStrategy::Natural),
            Just(TokenizerStrategy::BpeApproximation),
        ]) {
            let first = count_tokens(&text, strategy);
            let second = count_tokens(&text, strategy);
            prop_assert_eq!(first, second);
        }

        #[test]
        fn longer_text_has_more_or_equal_tokens(
            short_text in "[a-z ]{1,50}",
            extra in "[a-z ]{1,50}"
        ) {
            let longer = format!("{short_text} {extra}");
            let short_count = count_tokens(&short_text, TokenizerStrategy::BpeApproximation);
            let long_count = count_tokens(&longer, TokenizerStrategy::BpeApproximation);
            prop_assert!(long_count >= short_count);
        }
    }
}

// ---------------------------------------------------------------------------
// 5. HTML Compression Extended Properties
// ---------------------------------------------------------------------------

mod html_compressor_extended {
    use super::*;
    use clawdius_core::tools::html_compressor::HtmlCompressor;

    proptest! {
        #[test]
        fn compress_output_not_longer_than_input(html in "[a-zA-Z0-9 ]{1,2000}") {
            let compressor = HtmlCompressor::new();
            if let Ok(result) = compressor.compress(&html) {
                prop_assert!(result.compressed_bytes <= html.len());
            }
        }

        #[test]
        fn compress_removes_html_tags(html_tag_content in "[a-z]{1,50}") {
            let html = format!("<p>{html_tag_content}</p>");
            let compressor = HtmlCompressor::new();
            if let Ok(result) = compressor.compress(&html) {
                prop_assert!(!result.markdown.contains('<'));
                prop_assert!(!result.markdown.contains('>'));
            }
        }

        #[test]
        fn empty_input_produces_empty_or_minimal_output(html in "") {
            let compressor = HtmlCompressor::new();
            if let Ok(result) = compressor.compress(&html) {
                prop_assert!(result.compressed_bytes <= 1);
            }
        }

        #[test]
        fn compression_ratio_bounded(html in "[a-zA-Z0-9 ]{1,500}") {
            let compressor = HtmlCompressor::new();
            if let Ok(result) = compressor.compress(&html) {
                prop_assert!(result.ratio >= -1.0);
                prop_assert!(result.ratio <= 1.0);
            }
        }

        #[test]
        fn script_tags_are_stripped(script_content in "[a-zA-Z0-9 ]{1,200}") {
            let html = format!("<script>{script_content}</script><p>visible</p>");
            let compressor = HtmlCompressor::new();
            if let Ok(result) = compressor.compress(&html) {
                prop_assert!(!result.markdown.contains("script"));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 6. Diff Computation Properties
// ---------------------------------------------------------------------------

mod diff_computation {
    use super::*;
    use clawdius_core::diff::FileDiff;
    use std::path::PathBuf;

    proptest! {
        #[test]
        fn identical_inputs_produce_no_hunks(text in ".{0,500}") {
            let diff = FileDiff::compute(PathBuf::from("test.txt"), Some(&text), &text);
            prop_assert_eq!(diff.hunks.len(), 0);
        }

        #[test]
        fn stats_files_changed_is_always_one(
            old in ".{0,200}",
            new in ".{0,200}"
        ) {
            let diff = FileDiff::compute(PathBuf::from("test.txt"), Some(&old), &new);
            prop_assert_eq!(diff.stats().files_changed, 1);
        }

        #[test]
        fn unified_diff_starts_with_old_header(
            old in ".{0,200}",
            new in ".{0,200}"
        ) {
            let diff = FileDiff::compute(PathBuf::from("test.txt"), Some(&old), &new);
            let unified = diff.to_unified();
            if !unified.is_empty() {
                prop_assert!(unified.starts_with("--- "));
                prop_assert!(unified.contains("+++ "));
            }
        }

        #[test]
        fn new_file_has_zero_deletions(new_content in ".{1,200}") {
            let diff = FileDiff::compute(PathBuf::from("new.txt"), None, &new_content);
            prop_assert_eq!(diff.stats().deletions, 0);
        }

        #[test]
        fn empty_old_new_file_produces_additions(new_content in ".{1,100}") {
            let diff = FileDiff::compute(PathBuf::from("test.txt"), Some(""), &new_content);
            if !diff.hunks.is_empty() {
                let stats = diff.stats();
                prop_assert!(stats.additions > 0);
                prop_assert_eq!(stats.deletions, 0);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 7. Session Serialization Properties
// ---------------------------------------------------------------------------

mod session_serialization {
    use super::*;
    use clawdius_core::session::{Message, MessageRole, Session, SessionMeta, TokenUsage};

    fn make_session(title: Option<String>, messages: Vec<Message>) -> Session {
        let mut session = Session::new();
        session.title = title;
        session.messages = messages;
        session
    }

    proptest! {
        #[test]
        fn json_roundtrip_preserves_title(title in "[a-zA-Z0-9 ]{0,100}") {
            let session = make_session(Some(title), vec![]);
            let json = serde_json::to_string(&session).unwrap();
            let restored: Session = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(restored.title, session.title);
        }

        #[test]
        fn json_roundtrip_preserves_message_count(
            messages in proptest::collection::vec(
                "[a-zA-Z ]{1,50}",
                0..20
            )
        ) {
            let msgs: Vec<Message> = messages
                .into_iter()
                .map(Message::user)
                .collect();
            let session = make_session(None, msgs);
            let json = serde_json::to_string(&session).unwrap();
            let restored: Session = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(restored.messages.len(), session.messages.len());
        }

        #[test]
        fn serialization_is_deterministic(title in "[a-z]{1,20}") {
            let session = make_session(Some(title), vec![Message::user("hello")]);
            let json1 = serde_json::to_string(&session).unwrap();
            let json2 = serde_json::to_string(&session).unwrap();
            prop_assert_eq!(json1, json2);
        }

        #[test]
        fn json_roundtrip_preserves_message_roles(
            roles in proptest::collection::vec(
                prop_oneof![
                    Just(MessageRole::User),
                    Just(MessageRole::Assistant),
                    Just(MessageRole::System),
                ],
                0..10
            )
        ) {
            let msgs: Vec<Message> = roles
                .into_iter()
                .enumerate()
                .map(|(i, role)| {
                    let mut msg = match role {
                        MessageRole::User | MessageRole::Tool => Message::user(format!("msg {i}")),
                        MessageRole::Assistant => Message::assistant(format!("resp {i}")),
                        MessageRole::System => Message::system(format!("sys {i}")),
                    };
                    msg.role = role;
                    msg
                })
                .collect();
            let session = make_session(None, msgs);
            let json = serde_json::to_string(&session).unwrap();
            let restored: Session = serde_json::from_str(&json).unwrap();
            for (orig, rest) in session.messages.iter().zip(restored.messages.iter()) {
                prop_assert_eq!(orig.role, rest.role);
            }
        }

        #[test]
        fn token_usage_roundtrip(input in 0usize..10000, output in 0usize..10000) {
            let usage = TokenUsage { input, output, cached: 0 };
            let json = serde_json::to_string(&usage).unwrap();
            let restored: TokenUsage = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(restored.input, usage.input);
            prop_assert_eq!(restored.output, usage.output);
        }

        #[test]
        fn session_meta_roundtrip(
            provider in proptest::option::of("[a-z]{1,20}"),
            model in proptest::option::of("[a-z0-9.-]{1,30}"),
            tags in proptest::collection::vec("[a-z]{1,10}", 0..5)
        ) {
            let meta = SessionMeta {
                provider,
                model,
                working_dir: None,
                tags,
                extra: serde_json::Map::new(),
            };
            let json = serde_json::to_string(&meta).unwrap();
            let restored: SessionMeta = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(restored.provider, meta.provider);
            prop_assert_eq!(restored.model, meta.model);
            prop_assert_eq!(restored.tags.len(), meta.tags.len());
        }
    }
}
