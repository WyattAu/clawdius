//! Tool execution result display component.
//!
//! Shows the result of a tool call with expandable output,
//! execution time, success/failure indicator, and arguments.

use crate::theme::colors;
use crate::theme::radius;
use crate::theme::spacing;
use crate::theme::typography;
use leptos::prelude::*;
use leptos::{component, view, IntoView};

#[derive(Clone, Debug)]
pub enum ToolKind {
    FileRead,
    FileWrite,
    FileEdit,
    Shell,
    GitStatus,
    GitLog,
    GitDiff,
    Search,
    Lsp,
    Mcp,
    Custom(String),
}

#[derive(Clone, Debug)]
pub struct ToolResultData {
    pub tool: ToolKind,
    pub success: bool,
    pub output: String,
    pub duration_ms: u32,
    pub arguments: Option<String>,
}

fn tool_label(tool: &ToolKind) -> String {
    match tool {
        ToolKind::FileRead => "File Read".to_string(),
        ToolKind::FileWrite => "File Write".to_string(),
        ToolKind::FileEdit => "File Edit".to_string(),
        ToolKind::Shell => "Shell".to_string(),
        ToolKind::GitStatus => "Git Status".to_string(),
        ToolKind::GitLog => "Git Log".to_string(),
        ToolKind::GitDiff => "Git Diff".to_string(),
        ToolKind::Search => "Search".to_string(),
        ToolKind::Lsp => "LSP".to_string(),
        ToolKind::Mcp => "MCP".to_string(),
        ToolKind::Custom(name) => name.clone(),
    }
}

const fn tool_icon(tool: &ToolKind) -> &'static str {
    match tool {
        ToolKind::FileRead | ToolKind::FileWrite | ToolKind::FileEdit => "F",
        ToolKind::Shell => "$",
        ToolKind::GitStatus | ToolKind::GitLog | ToolKind::GitDiff => "G",
        ToolKind::Search => "S",
        ToolKind::Lsp => "L",
        ToolKind::Mcp => "M",
        ToolKind::Custom(_) => "?",
    }
}

fn format_duration(ms: u32) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", f64::from(ms) / 1000.0)
    }
}

fn tool_result_header(
    icon: &'static str,
    label: String,
    dur: String,
    success: bool,
    expanded: RwSignal<bool>,
    has_output: bool,
) -> impl IntoView {
    let (status_color, status_bg, status_label) = if success {
        (colors::SUCCESS, colors::SUCCESS_BG, "OK")
    } else {
        (colors::ERROR, colors::ERROR_BG, "FAIL")
    };
    view! {
        <div
            class="tool-result-header"
            style:display="flex"
            style:align-items="center"
            style:justify-content="space-between"
            style:padding=format!("{} {}", spacing::SPACE_8, spacing::SPACE_12)
            style:cursor=if has_output { "pointer" } else { "default" }
            on:click=move |_| {
                if has_output { expanded.update(|e| *e = !*e); }
            }
        >
            <div style:display="flex" style:align-items="center" style:gap=spacing::SPACE_8>
                <span
                    class="tool-icon"
                    style:color=colors::TEXT_SECONDARY
                    style:font-family=typography::FONT_MONO
                    style:font-size=typography::SIZE_SM
                    style:width="1.2em"
                    style:text-align="center"
                >
                    {icon}
                </span>
                <span
                    class="tool-name"
                    style:color=colors::TEXT_PRIMARY
                    style:font-weight=typography::WEIGHT_MEDIUM
                >
                    {label}
                </span>
                <span
                    class="tool-status-badge"
                    style:color=status_color
                    style:background-color=status_bg
                    style:font-size=typography::SIZE_XS
                    style:padding=format!("{} {}", spacing::SPACE_4, spacing::SPACE_8)
                    style:border-radius=radius::SM
                    style:font-weight=typography::WEIGHT_SEMIBOLD
                    style:text-transform="uppercase"
                    style:letter-spacing="0.03em"
                >
                    {status_label}
                </span>
            </div>
            <div style:display="flex" style:align-items="center" style:gap=spacing::SPACE_8>
                {has_output.then(|| view! {
                    <span
                        class="tool-expand-icon"
                        style:color=colors::TEXT_MUTED
                        style:font-size=typography::SIZE_XS
                        aria-label=move || if expanded.get() { "Collapse" } else { "Expand" }
                    >
                        {move || if expanded.get() { "v" } else { ">" }}
                    </span>
                })}
                <span
                    class="tool-duration"
                    style:color=colors::TEXT_MUTED
                    style:font-family=typography::FONT_MONO
                    style:font-size=typography::SIZE_XS
                >
                    {dur}
                </span>
            </div>
        </div>
    }
}

fn tool_arguments_section(args: String, args_expanded: RwSignal<bool>) -> impl IntoView {
    view! {
        <div class="tool-arguments" style:border-top=format!("1px solid {}", colors::BORDER)>
            <div
                style:padding=format!("{} {}", spacing::SPACE_4, spacing::SPACE_12)
                style:color=colors::TEXT_MUTED
                style:font-size=typography::SIZE_XS
                style:cursor="pointer"
                on:click=move |ev: web_sys::MouseEvent| {
                    ev.stop_propagation();
                    args_expanded.update(|e| *e = !*e);
                }
            >
                <span style:margin-right=spacing::SPACE_8>
                    {move || if args_expanded.get() { "v" } else { ">" }}
                </span>
                "Input"
            </div>
            {move || args_expanded.get().then(|| view! {
                <pre
                    class="tool-args-content"
                    style:margin="0"
                    style:padding=format!("{} {}", spacing::SPACE_8, spacing::SPACE_12)
                    style:background-color=colors::CODE_BG
                    style:color=colors::TEXT_SECONDARY
                    style:font-family=typography::FONT_MONO
                    style:font-size=typography::SIZE_XS
                    style:overflow-x="auto"
                    style:max-height="150px"
                    style:white-space="pre-wrap"
                    style:word-break="break-all"
                >
                    {args.clone()}
                </pre>
            })}
        </div>
    }
}

fn tool_output_section(
    expanded: RwSignal<bool>,
    is_truncated: bool,
    output_content: String,
    output_preview: String,
    output_lines: usize,
) -> impl IntoView {
    move || {
        if expanded.get() {
            view! {
                <div
                    class="tool-result-output"
                    style:border-top=format!("1px solid {}", colors::BORDER)
                >
                    <pre
                        style:margin="0"
                        style:padding=format!("{} {}", spacing::SPACE_8, spacing::SPACE_12)
                        style:background-color=colors::CODE_BG
                        style:color=colors::TEXT_PRIMARY
                        style:font-family=typography::FONT_MONO
                        style:font-size=typography::SIZE_XS
                        style:overflow-x="auto"
                        style:max-height="300px"
                        style:white-space="pre-wrap"
                        style:word-break="break-all"
                    >
                        {output_content.clone()}
                    </pre>
                </div>
            }
            .into_any()
        } else if is_truncated {
            view! {
                <div
                    class="tool-result-preview"
                    style:border-top=format!("1px solid {}", colors::BORDER)
                    style:padding=format!("{} {}", spacing::SPACE_8, spacing::SPACE_12)
                >
                    <pre
                        style:margin="0"
                        style:color=colors::TEXT_MUTED
                        style:font-family=typography::FONT_MONO
                        style:font-size=typography::SIZE_XS
                        style:overflow="hidden"
                        style:white-space="pre-wrap"
                        style:max-height="3.6em"
                    >
                        {output_preview.clone()}
                    </pre>
                    <span style:color=colors::TEXT_MUTED style:font-size=typography::SIZE_XS>
                        {format!("... {} more lines", output_lines.saturating_sub(3))}
                    </span>
                </div>
            }
            .into_any()
        } else {
            ().into_any()
        }
    }
}

// leptos' #[component] macro re-emits the implementation as a `#[doc(hidden)]`
// pub fn __component_* and drops `#[must_use]` from it, so this targeted allow
// is the only way to satisfy clippy::must_use_candidate for that generated fn.
#[allow(clippy::must_use_candidate)]
#[component]
pub fn ToolResult(#[prop(into)] result: ToolResultData) -> impl IntoView {
    let expanded = RwSignal::new(false);
    let args_expanded = RwSignal::new(false);
    let ToolResultData {
        tool,
        success,
        output,
        duration_ms,
        arguments,
    } = result;
    let label = tool_label(&tool);
    let icon = tool_icon(&tool);
    let status_class = if success {
        "tool-success"
    } else {
        "tool-error"
    };
    let status_color = if success {
        colors::SUCCESS
    } else {
        colors::ERROR
    };
    let dur = format_duration(duration_ms);
    let has_output = !output.is_empty();
    let output_lines = output.lines().count();
    let output_preview: String = output.lines().take(3).collect::<Vec<_>>().join("\n");
    let is_truncated = output_lines > 3;
    let output_content = output;

    view! {
        <div
            class=format!("tool-result {status_class}")
            style:background-color=colors::BG_SURFACE
            style:border=format!("1px solid {}", colors::BORDER)
            style:border-left=format!("3px solid {status_color}")
            style:border-radius=radius::MD
            style:overflow="hidden"
            style:font-family=typography::FONT_SANS
            style:font-size=typography::SIZE_SM
        >
            {tool_result_header(icon, label, dur, success, expanded, has_output)}
            {arguments.map(|args| tool_arguments_section(args, args_expanded))}
            {has_output.then(|| {
                tool_output_section(expanded, is_truncated, output_content, output_preview, output_lines)
            })}
        </div>
    }
}
