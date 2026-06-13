//! Workspace file tree component.
//!
//! Displays a collapsible file tree with icons,
//! keyboard navigation, and selection support.

use crate::theme::colors;
use crate::theme::radius;
use crate::theme::spacing;
use crate::theme::typography;
use leptos::prelude::*;
use leptos::{component, view, IntoView};

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub children: Vec<FileEntry>,
    pub is_expanded: bool,
    pub size: Option<u64>,
}

fn file_icon(name: &str, is_dir: bool) -> &'static str {
    if is_dir {
        return "D";
    }
    match name.rsplit('.').next().unwrap_or("") {
        "rs" => "R",
        "toml" => "T",
        "json" => "J",
        "js" | "ts" => "S",
        "md" => "M",
        "css" | "scss" => "C",
        "html" => "H",
        "py" => "P",
        "go" => "G",
        "yaml" | "yml" => "Y",
        "lock" => "L",
        _ => "F",
    }
}

fn format_size(size: u64) -> String {
    if size < 1024 {
        format!("{size}B")
    } else if size < 1024 * 1024 {
        format!("{:.1}K", size as f64 / 1024.0)
    } else {
        format!("{:.1}M", size as f64 / (1024.0 * 1024.0))
    }
}

#[component]
pub fn FileTree(
    #[prop(into)] entries: Vec<FileEntry>,
    #[prop(optional)] selected_path: Option<String>,
    #[prop(optional)] on_select: Option<impl Fn(String) + 'static>,
) -> impl IntoView {
    let expanded = RwSignal::new(
        entries
            .iter()
            .filter(|e| e.is_dir && e.is_expanded)
            .map(|e| e.path.clone())
            .collect::<Vec<_>>(),
    );
    let focused_idx = RwSignal::new(0usize);

    let flat_entries: Vec<(String, bool, String, Option<u64>, usize)> = {
        let mut flat = Vec::new();
        fn flatten(
            entries: &[FileEntry],
            depth: usize,
            expanded: &[String],
            flat: &mut Vec<(String, bool, String, Option<u64>, usize)>,
        ) {
            for e in entries {
                flat.push((e.path.clone(), e.is_dir, e.name.clone(), e.size, depth));
                if e.is_dir && expanded.contains(&e.path) {
                    flatten(&e.children, depth + 1, expanded, flat);
                }
            }
        }
        flatten(&entries, 0, &[], &mut flat);
        flat
    };

    let total = flat_entries.len();
    let _flat_for_render = flat_entries.clone();
    let flat_for_keys = flat_entries.clone();

    view! {
        <div
            class="file-tree"
            role="tree"
            aria-label="File browser"
            tabindex="0"
            style:background-color=colors::BG_PRIMARY
            style:border=format!("1px solid {}", colors::BORDER)
            style:border-radius=radius::MD
            style:overflow="hidden"
            style:font-family=typography::FONT_MONO
            style:font-size=typography::SIZE_SM
            on:keydown=move |ev: web_sys::KeyboardEvent| {
                match ev.key().as_str() {
                    "ArrowDown" => {
                        ev.prevent_default();
                        focused_idx.update(|i| {
                            if *i < total.saturating_sub(1) { *i += 1; }
                        });
                    }
                    "ArrowUp" => {
                        ev.prevent_default();
                        focused_idx.update(|i| {
                            *i = i.saturating_sub(1);
                        });
                    }
                    "Enter" => {
                        if let Some((path, is_dir, _, _, _)) = flat_for_keys.get(focused_idx.get()) {
                            if *is_dir {
                                expanded.update(|ex| {
                                    if ex.contains(path) {
                                        ex.retain(|p| p != path);
                                    } else {
                                        ex.push(path.clone());
                                    }
                                });
                            } else if let Some(ref cb) = on_select {
                                cb(path.clone());
                            }
                        }
                    }
                    " " => {
                        ev.prevent_default();
                        if let Some((path, is_dir, _, _, _)) = flat_for_keys.get(focused_idx.get()) {
                            if *is_dir {
                                expanded.update(|ex| {
                                    if ex.contains(path) {
                                        ex.retain(|p| p != path);
                                    } else {
                                        ex.push(path.clone());
                                    }
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        >
            <div class="file-tree-header" style:padding=format!("{} {}", spacing::SPACE_8, spacing::SPACE_12) style:border-bottom=format!("1px solid {}", colors::BORDER) style:color=colors::TEXT_SECONDARY style:font-size=typography::SIZE_XS>
                "EXPLORER"
            </div>
            <div class="file-tree-items" style:padding=format!("{} 0", spacing::SPACE_4)>
                {entries.into_iter().map(|entry| {
                    view! { <FileEntryView entry selected_path=selected_path.clone() /> }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}

#[component]
fn FileEntryView(entry: FileEntry, selected_path: Option<String>) -> AnyView {
    let icon = file_icon(&entry.name, entry.is_dir);
    let indent = entry.path.matches('/').count();
    let is_selected = selected_path.as_ref() == Some(&entry.path);
    let icon_color = if entry.is_dir {
        colors::ACCENT_DIM
    } else {
        colors::TEXT_SECONDARY
    };
    let entry_expanded = RwSignal::new(entry.is_expanded);
    let children = entry.children.clone();
    let _entry_path = entry.path.clone();
    let entry_is_dir = entry.is_dir;
    let entry_name = entry.name.clone();
    let entry_size = entry.size;
    let sp = selected_path.clone();

    view! {
        <div>
            <div
                class=format!("file-entry{}", if is_selected { " file-selected" } else { "" })
                role="treeitem"
                aria-expanded=move || if entry_is_dir { Some(entry_expanded.get()) } else { None }
                aria-selected=is_selected
                style:display="flex"
                style:align-items="center"
                style:gap=spacing::SPACE_8
                style:padding=format!("{} {}", spacing::SPACE_4, spacing::SPACE_12)
                style:padding-left=format!("{}rem", indent as f64 * 0.75 + 0.75)
                style:cursor="pointer"
                style:background-color=move || if is_selected { colors::BG_ELEVATED } else { "transparent" }
                style:border-radius=radius::SM
                style:border-left=move || if is_selected { format!("2px solid {}", colors::ACCENT) } else { "2px solid transparent".to_string() }
                on:click=move |_| {
                    if entry_is_dir {
                        entry_expanded.update(|e| *e = !*e);
                    }
                }
                on:keydown=move |ev: web_sys::KeyboardEvent| {
                    if ev.key() == "Enter" || ev.key() == " " {
                        ev.prevent_default();
                        if entry_is_dir {
                            entry_expanded.update(|e| *e = !*e);
                        }
                    }
                }
            >
                {entry_is_dir.then(|| view! {
                    <span
                        class="expand-icon"
                        style:color=colors::TEXT_MUTED
                        style:font-size=typography::SIZE_XS
                        style:width="1em"
                        style:text-align="center"
                    >
                        {move || if entry_expanded.get() { "v" } else { ">" }}
                    </span>
                })}
                <span
                    class="file-icon"
                    style:color=icon_color
                    style:font-size=typography::SIZE_SM
                    style:width="1.2em"
                    style:text-align="center"
                >
                    {icon}
                </span>
                <span
                    class="file-name"
                    style:color=if is_selected { colors::TEXT_PRIMARY } else { colors::TEXT_SECONDARY }
                    style:flex="1"
                    style:overflow="hidden"
                    style:text-overflow="ellipsis"
                    style:white-space="nowrap"
                >
                    {entry_name}
                </span>
                {entry_size.map(|s| view! {
                    <span
                        class="file-size"
                        style:color=colors::TEXT_MUTED
                        style:font-size=typography::SIZE_XS
                        style:flex-shrink="0"
                    >
                        {format_size(s)}
                    </span>
                })}
            </div>
            {move || {
                if entry_is_dir && entry_expanded.get() {
                    children.iter().map(|child| {
                        view! { <FileEntryView entry=child.clone() selected_path=sp.clone() /> }
                    }).collect::<Vec<_>>()
                } else {
                    Vec::new()
                }
            }}
        </div>
    }
    .into_any()
}
