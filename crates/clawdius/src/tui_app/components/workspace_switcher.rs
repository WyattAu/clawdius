//! Workspace switcher popup component.
//!
//! Displays a list of workspaces/projects for the user to
//! switch between in multi-repo mode.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::tui_app::theme::Theme;

/// A workspace/project entry for the switcher.
#[derive(Debug, Clone)]
pub struct WorkspaceEntry {
    /// Project ID.
    pub id: String,
    /// Project name.
    pub name: String,
    /// Project root path.
    pub root_path: String,
    /// Language detected.
    pub language: String,
    /// Number of files.
    pub file_count: usize,
    /// Whether this is the currently active project.
    pub is_active: bool,
}

/// Workspace switcher state.
pub struct WorkspaceSwitcher {
    /// List of workspaces.
    pub entries: Vec<WorkspaceEntry>,
    /// Current selection index.
    pub selected: usize,
    /// Whether the switcher is visible.
    pub visible: bool,
    /// Filter query.
    pub filter: String,
}

impl WorkspaceSwitcher {
    /// Create a new workspace switcher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            selected: 0,
            visible: false,
            filter: String::new(),
        }
    }

    /// Set workspace entries.
    pub fn set_entries(&mut self, entries: Vec<WorkspaceEntry>) {
        self.entries = entries;
        let active_idx = self.entries.iter().position(|e| e.is_active);
        self.selected = active_idx.unwrap_or(0);
    }

    /// Open the switcher.
    pub fn open(&mut self) {
        self.visible = true;
        self.filter.clear();
        let active_idx = self.entries.iter().position(|e| e.is_active);
        self.selected = active_idx.unwrap_or(0);
    }

    /// Close the switcher.
    pub const fn close(&mut self) {
        self.visible = false;
    }

    /// Move selection up.
    pub fn move_up(&mut self) {
        let count = self.filtered_entries().count();
        if self.selected > 0 {
            self.selected -= 1;
        }
        // Ensure selection is within bounds
        if self.selected >= count {
            self.selected = count.saturating_sub(1);
        }
    }

    /// Move selection down.
    pub fn move_down(&mut self) {
        let count = self.filtered_entries().count();
        if self.selected + 1 < count {
            self.selected += 1;
        }
    }

    /// Get the currently selected workspace ID.
    #[must_use]
    pub fn selected_id(&self) -> Option<&str> {
        let entry = self.filtered_entries().nth(self.selected)?;
        Some(&entry.id)
    }

    /// Add a character to the filter.
    pub fn type_char(&mut self, c: char) {
        self.filter.push(c);
        self.selected = 0;
    }

    /// Remove last character from filter.
    pub fn backspace(&mut self) {
        self.filter.pop();
        self.selected = 0;
    }

    /// Get filtered entries.
    fn filtered_entries(&self) -> impl Iterator<Item = &WorkspaceEntry> {
        let filter = self.filter.to_lowercase();
        self.entries.iter().filter(move |e| {
            filter.is_empty()
                || e.name.to_lowercase().contains(&filter)
                || e.root_path.to_lowercase().contains(&filter)
                || e.language.to_lowercase().contains(&filter)
        })
    }
}

impl Default for WorkspaceSwitcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Workspace switcher widget.
pub struct WorkspaceSwitcherWidget<'a> {
    switcher: &'a WorkspaceSwitcher,
    theme: &'a Theme,
}

impl<'a> WorkspaceSwitcherWidget<'a> {
    pub const fn new(switcher: &'a WorkspaceSwitcher, theme: &'a Theme) -> Self {
        Self { switcher, theme }
    }
}

impl Widget for WorkspaceSwitcherWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        Clear.render(area, buf);

        let width = area.width.min(65);
        let height = area.height.min(20);

        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let popup_area = Rect::new(x, y, width, height);

        let filtered: Vec<_> = self.switcher.filtered_entries().collect();

        let title = if self.switcher.filter.is_empty() {
            " Workspaces ".to_string()
        } else {
            format!(" Workspaces (filter: {}) ", self.switcher.filter)
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(self.theme.border_focus());

        if filtered.is_empty() {
            let no_results = Paragraph::new("No matching workspaces")
                .style(self.theme.muted())
                .block(block);
            no_results.render(popup_area, buf);
            return;
        }

        // Build content lines
        let content_height = (popup_area.height.saturating_sub(2)) as usize;
        let total_lines = filtered.len().min(content_height);

        // Calculate scroll offset
        let scroll_offset = if self.switcher.selected >= content_height {
            self.switcher.selected - content_height + 1
        } else {
            0
        };

        let mut lines = Vec::with_capacity(total_lines);
        for (i, entry) in filtered.iter().enumerate().skip(scroll_offset).take(total_lines) {
            let is_selected = i == self.switcher.selected;
            let active_marker = if entry.is_active { "● " } else { "  " };

            let name = if entry.name.len() > 25 {
                format!("{}…", &entry.name[..23])
            } else {
                entry.name.clone()
            };

            let path = if entry.root_path.len() > 30 {
                format!("…{}", &entry.root_path[entry.root_path.len() - 28..])
            } else {
                entry.root_path.clone()
            };

            let style = if is_selected {
                self.theme.file_selected().add_modifier(Modifier::BOLD)
            } else {
                self.theme.file_item()
            };

            lines.push(Line::from(vec![
                Span::styled(
                    active_marker.to_string(),
                    if entry.is_active {
                        self.theme.user_message()
                    } else {
                        self.theme.muted()
                    },
                ),
                Span::styled(name, style),
                Span::raw(" "),
                Span::styled(
                    format!("[{}]", entry.language),
                    self.theme.title().add_modifier(Modifier::DIM),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{} files", entry.file_count),
                    self.theme.muted().add_modifier(Modifier::DIM),
                ),
            ]));

            lines.push(Line::from(vec![
                Span::styled("    ", style),
                Span::styled(
                    path,
                    self.theme.muted().add_modifier(Modifier::DIM),
                ),
            ]));
        }

        let content = Paragraph::new(lines).block(block);
        content.render(popup_area, buf);
    }
}
