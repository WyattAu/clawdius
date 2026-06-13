//! Workspace file tree component.
//!
//! Displays a collapsible file tree with icons,
//! supporting multi-root workspaces.

use leptos::prelude::*;
use leptos::{component, view, IntoView};

/// A file or directory entry.
#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub children: Vec<FileEntry>,
    pub is_expanded: bool,
}

/// Renders a file tree for workspace navigation.
#[component]
pub fn FileTree(
    /// Root entries to display.
    #[prop(into)]
    entries: Vec<FileEntry>,
    /// Called when a file is selected.
    #[prop(optional)]
    _on_select: Option<impl Fn(String) + 'static>,
) -> impl IntoView {
    view! {
        <div class="file-tree">
            {entries.into_iter().map(|entry| {
                view! { <FileEntryView entry /> }
            }).collect::<Vec<_>>()}
        </div>
    }
}

/// Recursive file entry renderer.
#[component]
fn FileEntryView(entry: FileEntry) -> AnyView {
    let icon = if entry.is_dir { "D" } else { "F" };
    let indent = entry.path.matches('/').count();

    view! {
        <div class="file-entry" style:padding-left=format!("{}rem", indent as f64 * 0.75)>
            <span class="file-icon">{icon}</span>
            <span class="file-name">{entry.name}</span>
        </div>
        {(entry.is_dir && entry.is_expanded).then(|| {
            entry.children.into_iter().map(|child| {
                view! { <FileEntryView entry=child /> }
            }).collect::<Vec<_>>()
        })}
    }
    .into_any()
}
