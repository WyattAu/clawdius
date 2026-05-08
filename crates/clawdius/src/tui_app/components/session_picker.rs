#![allow(dead_code)]

//! Session picker popup component.
//!
//! Displays a list of sessions for the user to select from.
//! Supports filtering and keyboard navigation.

use ratatui::{
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, ListItem, ListState, Paragraph, Widget},
};

use crate::tui_app::theme::Theme;

/// A session entry for the picker.
#[derive(Debug, Clone)]
pub struct SessionEntry {
    /// Session ID.
    pub id: String,
    /// Session title.
    pub title: String,
    /// Number of messages.
    pub message_count: usize,
    /// Last activity timestamp.
    pub last_active: String,
    /// Token usage.
    pub tokens_used: usize,
    /// Whether this is the currently active session.
    pub is_active: bool,
}

/// Session picker state.
pub struct SessionPicker {
    /// List of sessions to display.
    pub entries: Vec<SessionEntry>,
    /// Current selection index.
    pub state: ListState,
    /// Filter query.
    pub filter: String,
    /// Whether the picker is visible.
    pub visible: bool,
}

impl SessionPicker {
    /// Create a new session picker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            state: ListState::default(),
            filter: String::new(),
            visible: false,
        }
    }

    /// Set the session entries.
    pub fn set_entries(&mut self, entries: Vec<SessionEntry>) {
        self.entries = entries;
        if self.entries.is_empty() {
            self.state.select(None);
        } else {
            // Select the active session by default
            let active_idx = self.entries.iter().position(|e| e.is_active);
            self.state.select(active_idx.or(Some(0)));
        }
    }

    /// Open the picker.
    pub fn open(&mut self) {
        self.visible = true;
        self.filter.clear();
        if !self.entries.is_empty() {
            self.state.select(Some(0));
        }
    }

    /// Close the picker.
    pub const fn close(&mut self) {
        self.visible = false;
    }

    /// Move selection up.
    pub fn move_up(&mut self) {
        let current = self.state.selected().unwrap_or(0);
        if current > 0 {
            self.state.select(Some(current - 1));
        }
    }

    /// Move selection down.
    pub fn move_down(&mut self) {
        let count = self.filtered_entries().count();
        let current = self.state.selected().unwrap_or(0);
        if current + 1 < count {
            self.state.select(Some(current + 1));
        }
    }

    /// Get the currently selected session ID.
    #[must_use]
    pub fn selected_id(&self) -> Option<&str> {
        let idx = self.state.selected()?;
        let entry = self.filtered_entries().nth(idx)?;
        Some(&entry.id)
    }

    /// Add a character to the filter.
    pub fn type_char(&mut self, c: char) {
        self.filter.push(c);
        self.state.select(Some(0));
    }

    /// Remove last character from filter.
    pub fn backspace(&mut self) {
        self.filter.pop();
        self.state.select(Some(0));
    }

    /// Get filtered entries.
    fn filtered_entries(&self) -> impl Iterator<Item = &SessionEntry> {
        let filter = self.filter.to_lowercase();
        self.entries.iter().filter(move |e| {
            filter.is_empty()
                || e.title.to_lowercase().contains(&filter)
                || e.id.to_lowercase().contains(&filter)
        })
    }
}

impl Default for SessionPicker {
    fn default() -> Self {
        Self::new()
    }
}

/// Session picker widget.
pub struct SessionPickerWidget<'a> {
    picker: &'a SessionPicker,
    theme: &'a Theme,
}

impl<'a> SessionPickerWidget<'a> {
    pub const fn new(picker: &'a SessionPicker, theme: &'a Theme) -> Self {
        Self { picker, theme }
    }
}

impl Widget for SessionPickerWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        // Clear the background
        Clear.render(area, buf);

        let width = area.width.min(60);
        let height = area.height.min(20);

        // Center the popup
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let popup_area = Rect::new(x, y, width, height);

        let filtered: Vec<_> = self.picker.filtered_entries().collect();

        let title = if self.picker.filter.is_empty() {
            " Sessions ".to_string()
        } else {
            format!(" Sessions (filter: {}) ", self.picker.filter)
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(self.theme.border_focus());

        if filtered.is_empty() {
            let no_results = Paragraph::new("No matching sessions")
                .style(self.theme.muted())
                .block(block);
            no_results.render(popup_area, buf);
            return;
        }

        let items: Vec<ListItem<'_>> = filtered
            .iter()
            .map(|entry| {
                let active_marker = if entry.is_active { "● " } else { "  " };
                let title = if entry.title.is_empty() {
                    "<untitled>".to_string()
                } else if entry.title.len() > 35 {
                    format!("{}…", &entry.title[..33])
                } else {
                    entry.title.clone()
                };

                let line = Line::from(vec![
                    Span::styled(
                        active_marker.to_string(),
                        if entry.is_active {
                            self.theme.user_message()
                        } else {
                            self.theme.muted()
                        },
                    ),
                    Span::styled(title, self.theme.file_item()),
                    Span::styled(format!(" ({}) ", entry.message_count), self.theme.muted()),
                    Span::styled(
                        format!("{}tok", entry.tokens_used),
                        self.theme.muted().add_modifier(Modifier::DIM),
                    ),
                ]);

                ListItem::new(line)
            })
            .collect();

        // Render the list with highlight
        ratatui::widgets::List::new(items)
            .block(block)
            .highlight_style(self.theme.file_selected().add_modifier(Modifier::BOLD))
            .render(popup_area, buf);
    }
}
