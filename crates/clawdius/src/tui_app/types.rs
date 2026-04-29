use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub streaming: bool,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            timestamp: Utc::now(),
            streaming: false,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            timestamp: Utc::now(),
            streaming: false,
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            timestamp: Utc::now(),
            streaming: false,
        }
    }

    pub fn tool(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            timestamp: Utc::now(),
            streaming: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    #[default]
    Chat,
    FileBrowser,
    Diff,
    Help,
}

/// TUI layout mode — single pane or split panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutMode {
    /// Single pane — one view at a time (default).
    #[default]
    Single,
    /// Horizontal split — chat left, code/diff right.
    SplitHorizontal,
    /// Vertical split — chat top, code/diff bottom.
    SplitVertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    Normal,
    Insert,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum VimMode {
    #[default]
    Normal,
    Insert,
    Visual,
    Command,
}

#[allow(dead_code)]
pub enum InputFocus {
    Chat,
    FileBrowser,
    Diff,
}
