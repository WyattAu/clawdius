#![no_main]

use clawdius_core::graph_rag::languages::LanguageKind;
use clawdius_core::graph_rag::parser::CodeParser;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(mut parser) = CodeParser::new() {
            for lang in [
                LanguageKind::Rust,
                LanguageKind::Python,
                LanguageKind::JavaScript,
            ] {
                let _ = parser.parse(s, lang);
            }
        }
    }
});
