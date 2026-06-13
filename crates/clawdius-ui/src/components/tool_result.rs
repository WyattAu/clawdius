//! Tool execution result display component.
//!
//! Shows the result of a tool call (file read, shell command,
//! git operation, etc.) with appropriate formatting.

use leptos::prelude::*;
use leptos::{component, view, IntoView};

/// Type of tool that was executed.
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
    Custom(String),
}

/// Result of a tool execution.
#[derive(Clone, Debug)]
pub struct ToolResultData {
    pub tool: ToolKind,
    pub success: bool,
    pub output: String,
    pub duration_ms: u32,
}

/// Renders a tool execution result.
#[component]
pub fn ToolResult(
    /// The tool result to display.
    #[prop(into)]
    result: ToolResultData,
) -> impl IntoView {
    let tool_label = match &result.tool {
        ToolKind::FileRead => "File Read".to_string(),
        ToolKind::FileWrite => "File Write".to_string(),
        ToolKind::FileEdit => "File Edit".to_string(),
        ToolKind::Shell => "Shell".to_string(),
        ToolKind::GitStatus => "Git Status".to_string(),
        ToolKind::GitLog => "Git Log".to_string(),
        ToolKind::GitDiff => "Git Diff".to_string(),
        ToolKind::Search => "Search".to_string(),
        ToolKind::Custom(name) => name.clone(),
    };

    let status_class = if result.success {
        "tool-success"
    } else {
        "tool-error"
    };

    view! {
        <div class=format!("tool-result {status_class}")>
            <div class="tool-result-header">
                <span class="tool-name">{tool_label}</span>
                <span class="tool-duration">{format!("{}ms", result.duration_ms)}</span>
            </div>
            <div class="tool-result-output">
                <pre>{result.output}</pre>
            </div>
        </div>
    }
}
