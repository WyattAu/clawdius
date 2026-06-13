//! Syntax-highlighted code block component.
//!
//! Renders code with line numbers, copy button, and wrap toggle.

use crate::theme::colors;
use crate::theme::radius;
use crate::theme::spacing;
use crate::theme::typography;
use leptos::prelude::*;
use leptos::{component, view, IntoView};
use wasm_bindgen::JsCast;

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
    Html,
    Css,
    Shell,
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
            "html" | "htm" => Self::Html,
            "css" | "scss" | "sass" => Self::Css,
            "shell" | "sh" | "bash" | "zsh" => Self::Shell,
            other => Self::Unknown(other.to_string()),
        }
    }
}

#[component]
pub fn CodeBlock(
    #[prop(into)] code: String,
    #[prop(into)] language: String,
    #[prop(optional)] file_path: Option<String>,
    #[prop(optional)] highlight_lines: Option<Vec<u32>>,
) -> impl IntoView {
    let lang: Language = language.as_str().into();
    let lang_label = match &lang {
        Language::Unknown(s) => s.clone(),
        other => format!("{other:?}"),
    };
    let (copied, set_copied) = signal(false);
    let (wrapped, set_wrapped) = signal(false);
    let line_count = code.lines().count();
    let line_num_width = format!("{}", line_count).len();
    let hl = highlight_lines.unwrap_or_default();
    let code_for_copy = code.clone();

    view! {
        <div
            class="code-block"
            style:background-color=colors::CODE_BG
            style:border=format!("1px solid {}", colors::BORDER)
            style:border-radius=radius::MD
            style:overflow="hidden"
            style:font-family=typography::FONT_MONO
            style:font-size=typography::SIZE_SM
        >
            <div
                class="code-block-header"
                style:display="flex"
                style:align-items="center"
                style:justify-content="space-between"
                style:padding=format!("{} {}", spacing::SPACE_8, spacing::SPACE_12)
                style:background-color=colors::BG_SURFACE
                style:border-bottom=format!("1px solid {}", colors::BORDER)
            >
                <div style:display="flex" style:align-items="center" style:gap=spacing::SPACE_8>
                    <span
                        class="code-block-lang"
                        style:color=colors::ACCENT
                        style:font-size=typography::SIZE_XS
                        style:font-weight=typography::WEIGHT_SEMIBOLD
                        style:text-transform="uppercase"
                        style:letter-spacing="0.05em"
                    >
                        {lang_label}
                    </span>
                    {file_path.map(|p| view! {
                        <span
                            class="code-block-path"
                            style:color=colors::TEXT_MUTED
                            style:font-size=typography::SIZE_XS
                        >
                            {p}
                        </span>
                    })}
                </div>
                <div style:display="flex" style:gap=spacing::SPACE_8>
                    <button
                        class="code-block-wrap"
                        title=move || if wrapped.get() { "Unwrap lines" } else { "Wrap lines" }
                        aria-label=move || if wrapped.get() { "Unwrap lines" } else { "Wrap lines" }
                        style:background="transparent"
                        style:border="none"
                        style:color=move || if wrapped.get() { colors::ACCENT } else { colors::TEXT_MUTED }
                        style:cursor="pointer"
                        style:font-size=typography::SIZE_XS
                        style:padding=format!("{} {}", spacing::SPACE_4, spacing::SPACE_8)
                        style:border-radius=radius::SM
                        on:click=move |_| set_wrapped.update(|w| *w = !*w)
                    >
                        {move || if wrapped.get() { "Unwrap" } else { "Wrap" }}
                    </button>
                    <button
                        class="code-block-copy"
                        title="Copy code"
                        aria-label="Copy code to clipboard"
                        style:background="transparent"
                        style:border="none"
                        style:color=colors::TEXT_MUTED
                        style:cursor="pointer"
                        style:font-size=typography::SIZE_XS
                        style:padding=format!("{} {}", spacing::SPACE_4, spacing::SPACE_8)
                        style:border-radius=radius::SM
                        on:click=move |_| {
                            set_copied.set(true);
                            copy_to_clipboard(&code_for_copy);
                            set_timeout_copy(move || set_copied.set(false));
                        }
                    >
                        {move || if copied.get() { "Copied!" } else { "Copy" }}
                    </button>
                </div>
            </div>
            <div
                class="code-block-content"
                style:overflow-x=move || if wrapped.get() { "hidden" } else { "auto" }
                style:padding=format!("{} {}", spacing::SPACE_12, spacing::SPACE_16)
            >
                <pre style:margin="0" style:white-space=move || if wrapped.get() { "pre-wrap" } else { "pre" }>
                    {code.lines().enumerate().map(|(i, line)| {
                        let line_num = (i + 1) as u32;
                        let is_highlighted = hl.contains(&line_num);
                        let bg = if is_highlighted { colors::BG_ELEVATED } else { "transparent" };
                        let num_str = format!("{:>width$}", line_num, width = line_num_width);
                        view! {
                            <div
                                class="code-line"
                                style:display="flex"
                                style:background-color=bg
                                style:border-radius=radius::SM
                            >
                                <span
                                    class="line-number"
                                    style:color=colors::TEXT_MUTED
                                    style:user-select="none"
                                    style:min-width=format!("{}ch", line_num_width + 1)
                                    style:text-align="right"
                                    style:padding-right=spacing::SPACE_16
                                    style:flex-shrink="0"
                                    aria-hidden="true"
                                >
                                    {num_str}
                                </span>
                                <code style:color=colors::TEXT_PRIMARY style:flex="1">
                                    {line.to_string()}
                                </code>
                            </div>
                        }
                    }).collect::<Vec<_>>()}
                </pre>
            </div>
        </div>
    }
}

fn copy_to_clipboard(text: &str) {
    if let Some(w) = web_sys::window() {
        let nav = w.navigator();
        let clipboard = nav.clipboard();
        let _ = clipboard.write_text(text);
    }
}

fn set_timeout_copy(f: impl FnOnce() + 'static) {
    if let Some(w) = web_sys::window() {
        let closure = wasm_bindgen::closure::Closure::once_into_js(f);
        let func: &js_sys::Function = closure.unchecked_ref();
        let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(func, 1500);
    }
}
