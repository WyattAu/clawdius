//! Language detection and support

use std::path::Path;

use serde::{Deserialize, Serialize};
use tree_sitter::Language;

#[cfg(not(target_arch = "wasm32"))]
use tree_sitter_go::LANGUAGE as GO_LANGUAGE;
#[cfg(not(target_arch = "wasm32"))]
use tree_sitter_javascript::LANGUAGE as JAVASCRIPT_LANGUAGE;
#[cfg(not(target_arch = "wasm32"))]
use tree_sitter_python::LANGUAGE as PYTHON_LANGUAGE;
#[cfg(not(target_arch = "wasm32"))]
use tree_sitter_rust::LANGUAGE as RUST_LANGUAGE;
#[cfg(not(target_arch = "wasm32"))]
use tree_sitter_typescript::LANGUAGE_TSX as TSX_LANGUAGE;
#[cfg(not(target_arch = "wasm32"))]
use tree_sitter_typescript::LANGUAGE_TYPESCRIPT as TYPESCRIPT_LANGUAGE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageKind {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    TypeScriptJsx,
    Go,
}

impl LanguageKind {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            LanguageKind::Rust => "rust",
            LanguageKind::Python => "python",
            LanguageKind::JavaScript => "javascript",
            LanguageKind::TypeScript => "typescript",
            LanguageKind::TypeScriptJsx => "typescript-jsx",
            LanguageKind::Go => "go",
        }
    }

    #[must_use]
    pub fn parse_from_name(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "rs" | "rust" => Some(LanguageKind::Rust),
            "py" | "python" | "pyi" | "pyw" => Some(LanguageKind::Python),
            "js" | "javascript" | "mjs" | "cjs" => Some(LanguageKind::JavaScript),
            "ts" => Some(LanguageKind::TypeScript),
            "tsx" => Some(LanguageKind::TypeScriptJsx),
            "go" => Some(LanguageKind::Go),
            _ => None,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn tree_sitter_language(&self) -> Language {
        match self {
            LanguageKind::Rust => RUST_LANGUAGE.into(),
            LanguageKind::Python => PYTHON_LANGUAGE.into(),
            LanguageKind::JavaScript => JAVASCRIPT_LANGUAGE.into(),
            LanguageKind::TypeScript => TYPESCRIPT_LANGUAGE.into(),
            LanguageKind::TypeScriptJsx => TSX_LANGUAGE.into(),
            LanguageKind::Go => GO_LANGUAGE.into(),
        }
    }

    #[must_use]
    pub fn file_extensions(&self) -> &[&'static str] {
        match self {
            LanguageKind::Rust => &["rs"],
            LanguageKind::Python => &["py", "pyi", "pyw"],
            LanguageKind::JavaScript => &["js", "mjs", "cjs"],
            LanguageKind::TypeScript => &["ts"],
            LanguageKind::TypeScriptJsx => &["tsx"],
            LanguageKind::Go => &["go"],
        }
    }
}

impl std::fmt::Display for LanguageKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[must_use]
pub fn detect_language(path: &Path) -> Option<LanguageKind> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    LanguageKind::parse_from_name(&ext)
}

#[must_use]
pub fn is_supported(path: &Path) -> bool {
    detect_language(path).is_some()
}

#[must_use]
pub fn supported_extensions() -> Vec<&'static str> {
    vec![
        "rs", "py", "pyi", "pyw", "js", "mjs", "cjs", "ts", "tsx", "go",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language_rust() {
        assert_eq!(
            detect_language(Path::new("src/main.rs")),
            Some(LanguageKind::Rust)
        );
    }

    #[test]
    fn test_detect_language_python() {
        assert_eq!(
            detect_language(Path::new("script.py")),
            Some(LanguageKind::Python)
        );
        assert_eq!(
            detect_language(Path::new("script.pyi")),
            Some(LanguageKind::Python)
        );
    }

    #[test]
    fn test_detect_language_javascript() {
        assert_eq!(
            detect_language(Path::new("index.js")),
            Some(LanguageKind::JavaScript)
        );
        assert_eq!(
            detect_language(Path::new("module.mjs")),
            Some(LanguageKind::JavaScript)
        );
        assert_eq!(
            detect_language(Path::new("common.cjs")),
            Some(LanguageKind::JavaScript)
        );
    }

    #[test]
    fn test_detect_language_typescript() {
        assert_eq!(
            detect_language(Path::new("app.ts")),
            Some(LanguageKind::TypeScript)
        );
        assert_eq!(
            detect_language(Path::new("component.tsx")),
            Some(LanguageKind::TypeScriptJsx)
        );
    }

    #[test]
    fn test_detect_language_go() {
        assert_eq!(
            detect_language(Path::new("main.go")),
            Some(LanguageKind::Go)
        );
    }

    #[test]
    fn test_detect_language_unknown() {
        assert_eq!(detect_language(Path::new("README.md")), None);
        assert_eq!(detect_language(Path::new("config.json")), None);
    }

    #[test]
    fn test_is_supported() {
        assert!(is_supported(Path::new("main.rs")));
        assert!(is_supported(Path::new("app.py")));
        assert!(is_supported(Path::new("index.js")));
        assert!(!is_supported(Path::new("README.md")));
    }

    #[test]
    fn test_supported_extensions() {
        let extensions = supported_extensions();
        assert!(extensions.contains(&"rs"));
        assert!(extensions.contains(&"py"));
        assert!(extensions.contains(&"js"));
        assert!(extensions.contains(&"ts"));
        assert!(extensions.contains(&"go"));
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn test_tree_sitter_language() {
        let _rust_lang = LanguageKind::Rust.tree_sitter_language();
    }
}
