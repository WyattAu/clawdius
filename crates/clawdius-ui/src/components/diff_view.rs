//! File diff view component.
//!
//! Displays file diffs in unified or side-by-side format
//! with syntax highlighting and line numbers.

use leptos::prelude::*;
use leptos::{component, view, IntoView};

/// A single diff line.
#[derive(Clone, Debug)]
pub struct DiffLine {
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub content: String,
    pub kind: DiffLineKind,
}

/// Type of diff line.
#[derive(Clone, Debug, PartialEq)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
    Header,
}

/// Renders a diff view for a single file.
#[component]
pub fn DiffView(
    /// File path being diffed.
    #[prop(into)]
    file_path: String,
    /// Diff lines to display.
    #[prop(into)]
    lines: Vec<DiffLine>,
) -> impl IntoView {
    view! {
        <div class="diff-view">
            <div class="diff-header">{file_path}</div>
            <div class="diff-content">
                {lines.into_iter().map(|line| {
                    let class = match line.kind {
                        DiffLineKind::Context => "diff-context",
                        DiffLineKind::Added => "diff-added",
                        DiffLineKind::Removed => "diff-removed",
                        DiffLineKind::Header => "diff-header-line",
                    };
                    let old_num = line.old_line.map(|n| n.to_string()).unwrap_or_default();
                    let new_num = line.new_line.map(|n| n.to_string()).unwrap_or_default();
                    view! {
                        <div class=format!("diff-line {class}")>
                            <span class="diff-old-num">{old_num}</span>
                            <span class="diff-new-num">{new_num}</span>
                            <span class="diff-text">{line.content}</span>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}
