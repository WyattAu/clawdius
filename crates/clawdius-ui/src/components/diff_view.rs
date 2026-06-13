//! File diff view component.
//!
//! Displays file diffs in unified format with line numbers,
//! color-coded additions/removals, and collapsible context.

use crate::theme::colors;
use crate::theme::radius;
use crate::theme::spacing;
use crate::theme::typography;
use leptos::prelude::*;
use leptos::{component, view, IntoView};

#[derive(Clone, Debug)]
pub struct DiffLine {
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub content: String,
    pub kind: DiffLineKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
    Header,
}

#[derive(Clone, Debug)]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

fn parse_unified_diff(raw: &str) -> (String, Vec<DiffHunk>) {
    let mut file_name = String::from("unknown");
    let mut hunks: Vec<DiffHunk> = Vec::new();
    let mut current_header = String::new();
    let mut current_lines: Vec<DiffLine> = Vec::new();
    let mut old_line: u32 = 0;
    let mut new_line: u32 = 0;
    let mut in_hunk = false;

    for line in raw.lines() {
        if line.starts_with("diff --git") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                file_name = parts[3].trim_start_matches("b/").to_string();
            }
        } else if line.starts_with("@@") {
            if in_hunk {
                hunks.push(DiffHunk {
                    header: current_header.clone(),
                    lines: std::mem::take(&mut current_lines),
                });
            }
            current_header = line.to_string();
            if let Some(nums) = line.split("@@").nth(1) {
                let parts: Vec<&str> = nums.trim().split_whitespace().collect();
                if parts.len() >= 2 {
                    old_line = parts[0]
                        .trim_start_matches('-')
                        .split(',')
                        .next()
                        .and_then(|n| n.parse().ok())
                        .unwrap_or(1);
                    new_line = parts[1]
                        .trim_start_matches('+')
                        .split(',')
                        .next()
                        .and_then(|n| n.parse().ok())
                        .unwrap_or(1);
                }
            }
            in_hunk = true;
        } else if in_hunk {
            if let Some(content) = line.strip_prefix('+') {
                current_lines.push(DiffLine {
                    old_line: None,
                    new_line: Some(new_line),
                    content: content.to_string(),
                    kind: DiffLineKind::Added,
                });
                new_line += 1;
            } else if let Some(content) = line.strip_prefix('-') {
                current_lines.push(DiffLine {
                    old_line: Some(old_line),
                    new_line: None,
                    content: content.to_string(),
                    kind: DiffLineKind::Removed,
                });
                old_line += 1;
            } else if let Some(content) = line.strip_prefix(' ') {
                current_lines.push(DiffLine {
                    old_line: Some(old_line),
                    new_line: Some(new_line),
                    content: content.to_string(),
                    kind: DiffLineKind::Context,
                });
                old_line += 1;
                new_line += 1;
            }
        }
    }

    if in_hunk {
        hunks.push(DiffHunk {
            header: current_header,
            lines: current_lines,
        });
    }

    (file_name, hunks)
}

fn line_bg(kind: &DiffLineKind) -> &'static str {
    match kind {
        DiffLineKind::Added => colors::DIFF_ADDED,
        DiffLineKind::Removed => colors::DIFF_REMOVED,
        DiffLineKind::Context => "transparent",
        DiffLineKind::Header => colors::BG_SURFACE,
    }
}

fn line_color(kind: &DiffLineKind) -> &'static str {
    match kind {
        DiffLineKind::Added => colors::DIFF_ADDED_TEXT,
        DiffLineKind::Removed => colors::DIFF_REMOVED_TEXT,
        DiffLineKind::Context => colors::TEXT_PRIMARY,
        DiffLineKind::Header => colors::TEXT_SECONDARY,
    }
}

fn line_prefix(kind: &DiffLineKind) -> &'static str {
    match kind {
        DiffLineKind::Added => "+",
        DiffLineKind::Removed => "-",
        DiffLineKind::Context => " ",
        DiffLineKind::Header => "",
    }
}

#[component]
pub fn DiffView(
    #[prop(into)] file_path: String,
    #[prop(into)] lines: Vec<DiffLine>,
    #[prop(optional)] raw_diff: Option<String>,
) -> impl IntoView {
    let (collapsed, set_collapsed) = signal(false);

    let (display_path, hunks) = if let Some(ref raw) = raw_diff {
        let (p, h) = parse_unified_diff(raw);
        (p, h)
    } else {
        let hunk = DiffHunk {
            header: String::new(),
            lines,
        };
        (file_path, vec![hunk])
    };

    let added_count: usize = hunks
        .iter()
        .flat_map(|h| h.lines.iter())
        .filter(|l| l.kind == DiffLineKind::Added)
        .count();
    let removed_count: usize = hunks
        .iter()
        .flat_map(|h| h.lines.iter())
        .filter(|l| l.kind == DiffLineKind::Removed)
        .count();
    let total_lines: usize = hunks.iter().map(|h| h.lines.len()).sum();

    view! {
        <div
            class="diff-view"
            style:background-color=colors::BG_PRIMARY
            style:border=format!("1px solid {}", colors::BORDER)
            style:border-radius=radius::MD
            style:overflow="hidden"
            style:font-family=typography::FONT_MONO
            style:font-size=typography::SIZE_SM
        >
            <div
                class="diff-file-header"
                style:display="flex"
                style:align-items="center"
                style:justify-content="space-between"
                style:padding=format!("{} {}", spacing::SPACE_8, spacing::SPACE_12)
                style:background-color=colors::BG_SURFACE
                style:border-bottom=format!("1px solid {}", colors::BORDER)
                style:cursor="pointer"
                on:click=move |_| set_collapsed.update(|c| *c = !*c)
            >
                <div style:display="flex" style:align-items="center" style:gap=spacing::SPACE_8>
                    <span
                        class="diff-collapse-icon"
                        style:color=colors::TEXT_MUTED
                        style:font-size=typography::SIZE_XS
                        aria-label=move || if collapsed.get() { "Expand" } else { "Collapse" }
                    >
                        {move || if collapsed.get() { ">" } else { "v" }}
                    </span>
                    <span
                        class="diff-filename"
                        style:color=colors::TEXT_PRIMARY
                        style:font-weight=typography::WEIGHT_MEDIUM
                    >
                        {display_path}
                    </span>
                </div>
                <div style:display="flex" style:gap=spacing::SPACE_12>
                    <span
                        style:color=colors::DIFF_ADDED_TEXT
                        style:font-size=typography::SIZE_XS
                    >
                        {format!("+{added_count}")}
                    </span>
                    <span
                        style:color=colors::DIFF_REMOVED_TEXT
                        style:font-size=typography::SIZE_XS
                    >
                        {format!("-{removed_count}")}
                    </span>
                </div>
            </div>
            {move || {
                if collapsed.get() {
                    return ().into_any();
                }
                view! {
                    <div
                        class="diff-content"
                        style:max-height="600px"
                        style:overflow-y="auto"
                    >
                        {hunks.iter().flat_map(|hunk| {
                            let mut items: Vec<_> = Vec::new();
                            if !hunk.header.is_empty() {
                                items.push(view! {
                                    <div
                                        class="diff-hunk-header"
                                        style:padding=format!("{} {}", spacing::SPACE_4, spacing::SPACE_12)
                                        style:background-color=colors::BG_ELEVATED
                                        style:color=colors::TEXT_SECONDARY
                                        style:font-size=typography::SIZE_XS
                                    >
                                        {hunk.header.clone()}
                                    </div>
                                }.into_any());
                            }
                            for line in &hunk.lines {
                                let bg = line_bg(&line.kind);
                                let fg = line_color(&line.kind);
                                let prefix = line_prefix(&line.kind);
                                let old_num = line.old_line.map(|n| n.to_string()).unwrap_or_default();
                                let new_num = line.new_line.map(|n| n.to_string()).unwrap_or_default();
                                items.push(view! {
                                    <div
                                        class=format!("diff-line diff-{:?}", line.kind).to_lowercase()
                                        style:display="flex"
                                        style:background-color=bg
                                        style:padding=format!("0 {}", spacing::SPACE_12)
                                        style:min-height="1.4em"
                                    >
                                        <span
                                            class="diff-old-num"
                                            style:color=colors::TEXT_MUTED
                                            style:min-width="4ch"
                                            style:text-align="right"
                                            style:padding-right=spacing::SPACE_12
                                            style:user-select="none"
                                            aria-hidden="true"
                                        >
                                            {old_num}
                                        </span>
                                        <span
                                            class="diff-new-num"
                                            style:color=colors::TEXT_MUTED
                                            style:min-width="4ch"
                                            style:text-align="right"
                                            style:padding-right=spacing::SPACE_12
                                            style:user-select="none"
                                            aria-hidden="true"
                                        >
                                            {new_num}
                                        </span>
                                        <span class="diff-prefix" style:color=fg style:width="1ch" style:flex-shrink="0">
                                            {prefix}
                                        </span>
                                        <span class="diff-text" style:color=fg style:white-space="pre-wrap" style:word-break="break-all">
                                            {line.content.clone()}
                                        </span>
                                    </div>
                                }.into_any());
                            }
                            items
                        }).collect::<Vec<_>>()}
                        <div
                            class="diff-summary"
                            style:padding=format!("{} {}", spacing::SPACE_8, spacing::SPACE_12)
                            style:color=colors::TEXT_MUTED
                            style:font-size=typography::SIZE_XS
                            style:border-top=format!("1px solid {}", colors::BORDER)
                        >
                            {format!("{total_lines} lines changed")}
                        </div>
                    </div>
                }.into_any()
            }}
        </div>
    }
}
