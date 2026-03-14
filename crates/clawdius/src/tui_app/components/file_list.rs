use ratatui::{
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};
use std::path::PathBuf;

use super::super::theme;

#[derive(Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub is_dir: bool,
    pub selected: bool,
}

#[derive(Clone)]
pub struct FileList {
    entries: Vec<FileEntry>,
    state: ListState,
    current_dir: PathBuf,
}

impl FileList {
    pub fn new() -> Self {
        let current_dir = std::env::current_dir().unwrap_or_default();
        let mut list = Self {
            entries: Vec::new(),
            state: ListState::default(),
            current_dir,
        };
        list.refresh();
        list
    }

    pub fn refresh(&mut self) {
        self.entries.clear();

        if let Some(parent) = self.current_dir.parent() {
            self.entries.push(FileEntry {
                path: parent.to_path_buf(),
                is_dir: true,
                selected: false,
            });
        }

        if let Ok(read_dir) = std::fs::read_dir(&self.current_dir) {
            let mut dirs: Vec<_> = read_dir
                .filter_map(|e| e.ok())
                .map(|e| FileEntry {
                    path: e.path(),
                    is_dir: e.path().is_dir(),
                    selected: false,
                })
                .collect();

            dirs.sort_by(|a, b| match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.path.file_name().cmp(&b.path.file_name()),
            });

            self.entries.extend(dirs);
        }

        if !self.entries.is_empty() {
            self.state.select(Some(0));
        }
    }

    pub fn up(&mut self) {
        if let Some(selected) = self.state.selected() {
            if selected > 0 {
                self.state.select(Some(selected - 1));
            }
        }
    }

    pub fn down(&mut self) {
        if let Some(selected) = self.state.selected() {
            if selected + 1 < self.entries.len() {
                self.state.select(Some(selected + 1));
            }
        }
    }

    pub fn enter(&mut self) -> Option<PathBuf> {
        if let Some(selected) = self.state.selected() {
            if let Some(entry) = self.entries.get(selected) {
                if entry.is_dir {
                    self.current_dir = entry.path.clone();
                    self.refresh();
                    return None;
                } else {
                    return Some(entry.path.clone());
                }
            }
        }
        None
    }

    pub fn toggle_select(&mut self) {
        if let Some(selected) = self.state.selected() {
            if let Some(entry) = self.entries.get_mut(selected) {
                entry.selected = !entry.selected;
            }
        }
    }

    pub fn selected_files(&self) -> Vec<PathBuf> {
        self.entries
            .iter()
            .filter(|e| e.selected && !e.is_dir)
            .map(|e| e.path.clone())
            .collect()
    }

    pub fn current_dir(&self) -> &PathBuf {
        &self.current_dir
    }

    pub fn render(&mut self, f: &mut Frame<'_>, area: Rect) {
        let theme = theme::current();

        let items: Vec<ListItem<'_>> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let name = if i == 0 && entry.path != self.current_dir {
                    "..".to_string()
                } else {
                    entry
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                };

                let prefix = if entry.is_dir { "D " } else { "  " };
                let check = if entry.selected { "* " } else { "  " };

                let style = if entry.is_dir {
                    theme.file_dir()
                } else if entry.selected {
                    theme.file_selected()
                } else {
                    theme.file_item()
                };

                ListItem::new(Line::styled(format!("{}{}{}", check, prefix, name), style))
            })
            .collect();

        let title = format!("FILES {}", self.current_dir.display());
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.border())
                    .title(Line::styled(title, theme.title())),
            )
            .highlight_style(Style::default().fg(theme.text).bg(theme.selection))
            .highlight_symbol("> ");

        f.render_stateful_widget(list, area, &mut self.state);
    }
}

impl Default for FileList {
    fn default() -> Self {
        Self::new()
    }
}
