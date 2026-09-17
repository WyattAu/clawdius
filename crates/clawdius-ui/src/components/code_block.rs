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

#[derive(Clone, Debug, PartialEq, Eq)]
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

impl From<String> for Language {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

// leptos' #[component] macro re-emits the implementation as a `#[doc(hidden)]`
// pub fn __component_* and drops `#[must_use]` from it, so this targeted allow
// is the only way to satisfy clippy::must_use_candidate for that generated fn.
#[allow(clippy::must_use_candidate)]
#[component]
pub fn CodeBlock(
    #[prop(into)] code: String,
    #[prop(into)] language: Language,
    #[prop(optional)] file_path: Option<String>,
    #[prop(optional)] highlight_lines: Option<Vec<u32>>,
) -> impl IntoView {
    let lang_label = match language {
        Language::Unknown(s) => s,
        other => format!("{other:?}"),
    };
    let copied = RwSignal::new(false);
    let wrapped = RwSignal::new(false);
    let line_count = code.lines().count();
    let line_num_width = format!("{line_count}").len();
    let hl = highlight_lines.unwrap_or_default();
    let lines_view = code_lines(&code, &hl, line_num_width);
    let code_for_copy = code;

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
            {code_block_header(lang_label, file_path, code_for_copy, copied, wrapped)}
            <div
                class="code-block-content"
                style:overflow-x=move || if wrapped.get() { "hidden" } else { "auto" }
                style:padding=format!("{} {}", spacing::SPACE_12, spacing::SPACE_16)
            >
                <pre style:margin="0" style:white-space=move || if wrapped.get() { "pre-wrap" } else { "pre" }>
                    {lines_view}
                </pre>
            </div>
        </div>
    }
}

fn code_block_header(
    lang_label: String,
    file_path: Option<String>,
    code_for_copy: String,
    copied: RwSignal<bool>,
    wrapped: RwSignal<bool>,
) -> impl IntoView {
    view! {
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
                    on:click=move |_| wrapped.update(|w| *w = !*w)
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
                        copied.set(true);
                        copy_to_clipboard(&code_for_copy);
                        set_timeout_copy(move || copied.set(false));
                    }
                >
                    {move || if copied.get() { "Copied!" } else { "Copy" }}
                </button>
            </div>
        </div>
    }
}

fn code_lines(code: &str, hl: &[u32], line_num_width: usize) -> impl IntoView {
    code.lines()
        .enumerate()
        .map(|(i, line)| {
            let line_num = u32::try_from(i + 1).unwrap_or(u32::MAX);
            let is_highlighted = hl.contains(&line_num);
            let bg = if is_highlighted {
                colors::BG_ELEVATED
            } else {
                "transparent"
            };
            let num_str = format!("{line_num:>line_num_width$}");
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
        })
        .collect::<Vec<_>>()
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
