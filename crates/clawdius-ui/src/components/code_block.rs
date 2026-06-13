//! Syntax-highlighted code block component.
//!
//! Renders code with syntax highlighting. Currently uses a
//! simple approach; will integrate tree-sitter WASM for
//! full highlighting in production.

use leptos::prelude::*;
use leptos::{component, view, IntoView};

/// Programming language for syntax highlighting.
#[derive(Clone, Debug, PartialEq)]
pub enum Language {
    Rust,
    Python,
    TypeScript,
    JavaScript,
    Go,
    Java,
    Cpp,
    Ruby,
    Php,
    Toml,
    Json,
    Markdown,
    Unknown(String),
}

impl From<&str> for Language {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "rust" | "rs" => Self::Rust,
            "python" | "py" => Self::Python,
            "typescript" | "ts" => Self::TypeScript,
            "javascript" | "js" => Self::JavaScript,
            "go" => Self::Go,
            "java" => Self::Java,
            "cpp" | "c++" | "cxx" => Self::Cpp,
            "ruby" | "rb" => Self::Ruby,
            "php" => Self::Php,
            "toml" => Self::Toml,
            "json" => Self::Json,
            "markdown" | "md" => Self::Markdown,
            other => Self::Unknown(other.to_string()),
        }
    }
}

/// Renders a syntax-highlighted code block with copy button.
#[component]
pub fn CodeBlock(
    /// Source code to display.
    #[prop(into)]
    code: String,
    /// Programming language.
    #[prop(into)]
    language: String,
    /// Optional file path for header display.
    #[prop(optional)]
    file_path: Option<String>,
) -> impl IntoView {
    let lang: Language = language.as_str().into();
    let lang_label = match &lang {
        Language::Unknown(s) => s.clone(),
        other => format!("{other:?}"),
    };

    view! {
        <div class="code-block">
            <div class="code-block-header">
                <span class="code-block-lang">{lang_label}</span>
                {file_path.map(|p| view! {
                    <span class="code-block-path">{p}</span>
                })}
                <button class="code-block-copy" title="Copy code">
                    "Copy"
                </button>
            </div>
            <pre class="code-block-content">
                <code>{code}</code>
            </pre>
        </div>
    }
}
