//! Modern TUI theme system
//!
//! Design philosophy:
//! - Assertive: Bold, confident colors that command attention
//! - Clean: Minimal decoration, maximum information density
//! - Professional: No emojis, no whimsy, pure function
//! - High contrast: Readable in any lighting condition

use ratatui::style::{Color, Modifier, Style};

/// Modern dark theme inspired by GitHub's dark mode
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct Theme {
    /// Background color for the main area
    pub bg: Color,
    /// Surface color for elevated elements (headers, footers)
    pub surface: Color,
    /// Primary accent color (interactive elements)
    pub accent: Color,
    /// Secondary accent (less prominent interactive elements)
    pub accent_muted: Color,
    /// Success color (positive indicators)
    pub success: Color,
    /// Warning color (caution indicators)
    pub warning: Color,
    /// Error color (failure indicators)
    pub error: Color,
    /// Primary text color
    pub text: Color,
    /// Muted text color (secondary information)
    pub text_muted: Color,
    /// Border color
    pub border: Color,
    /// Border color for focused elements
    pub border_focus: Color,
    /// Selection background
    pub selection: Color,
}

#[allow(dead_code)]
impl Theme {
    #[allow(dead_code)]
    /// Create the default dark theme
    pub const fn dark() -> Self {
        Self {
            bg: Color::Rgb(13, 17, 23),             // #0d1117
            surface: Color::Rgb(22, 27, 34),        // #161b22
            accent: Color::Rgb(88, 166, 255),       // #58a6ff
            accent_muted: Color::Rgb(56, 139, 253), // #388bfd
            success: Color::Rgb(63, 185, 80),       // #3fb950
            warning: Color::Rgb(210, 153, 34),      // #d29922
            error: Color::Rgb(248, 81, 73),         // #f85149
            text: Color::Rgb(201, 209, 217),        // #c9d1d9
            text_muted: Color::Rgb(139, 148, 158),  // #8b949e
            border: Color::Rgb(48, 54, 61),         // #30363d
            border_focus: Color::Rgb(88, 166, 255), // #58a6ff
            selection: Color::Rgb(56, 139, 253),    // #388bfd with alpha
        }
    }

    /// Header style
    #[inline]
    pub const fn header(&self) -> Style {
        Style::new().fg(self.text).bg(self.surface)
    }

    /// Title text style (bold accent)
    #[inline]
    pub const fn title(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    /// Normal border style
    #[inline]
    pub const fn border(&self) -> Style {
        Style::new().fg(self.border)
    }

    /// Focused border style
    #[inline]
    pub const fn border_focus(&self) -> Style {
        Style::new().fg(self.border_focus)
    }

    /// User message style
    #[inline]
    #[allow(dead_code)]
    pub const fn user_message(&self) -> Style {
        Style::new().fg(self.success).add_modifier(Modifier::BOLD)
    }

    /// Assistant message style
    #[inline]
    pub const fn assistant_message(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    /// System message style
    #[inline]
    pub const fn system_message(&self) -> Style {
        Style::new().fg(self.text_muted)
    }

    /// Tool message style
    #[inline]
    pub const fn tool_message(&self) -> Style {
        Style::new().fg(self.warning)
    }

    /// Normal mode indicator
    #[inline]
    pub const fn mode_normal(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    /// Insert mode indicator
    #[inline]
    pub const fn mode_insert(&self) -> Style {
        Style::new().fg(self.success).add_modifier(Modifier::BOLD)
    }

    /// Command mode indicator
    #[inline]
    pub const fn mode_command(&self) -> Style {
        Style::new().fg(self.warning).add_modifier(Modifier::BOLD)
    }

    /// Status bar style
    #[inline]
    pub const fn status(&self) -> Style {
        Style::new().fg(self.text_muted).bg(self.surface)
    }

    /// Status highlight style
    #[inline]
    pub const fn status_highlight(&self) -> Style {
        Style::new()
            .fg(self.accent)
            .bg(self.surface)
            .add_modifier(Modifier::BOLD)
    }

    /// Muted text style
    #[inline]
    pub const fn muted(&self) -> Style {
        Style::new().fg(self.text_muted)
    }

    /// Error style
    #[inline]
    pub const fn error(&self) -> Style {
        Style::new().fg(self.error).add_modifier(Modifier::BOLD)
    }

    /// Help key style
    #[inline]
    pub const fn help_key(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    /// Help description style
    #[inline]
    pub const fn help_desc(&self) -> Style {
        Style::new().fg(self.text_muted)
    }

    /// Scrollbar track style
    #[inline]
    pub const fn scrollbar_track(&self) -> Style {
        Style::new().fg(self.border)
    }

    /// Scrollbar thumb style
    #[inline]
    pub const fn scrollbar_thumb(&self) -> Style {
        Style::new().fg(self.text_muted)
    }

    /// File list item style
    #[inline]
    pub const fn file_item(&self) -> Style {
        Style::new().fg(self.text)
    }

    /// File list directory style
    #[inline]
    pub const fn file_dir(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    /// File list selected style
    #[inline]
    pub const fn file_selected(&self) -> Style {
        Style::new().fg(self.text).bg(self.selection)
    }

    /// Diff addition style
    #[inline]
    pub const fn diff_add(&self) -> Style {
        Style::new().fg(self.success).bg(Color::Rgb(30, 50, 30))
    }

    /// Diff deletion style
    #[inline]
    pub const fn diff_delete(&self) -> Style {
        Style::new().fg(self.error).bg(Color::Rgb(50, 30, 30))
    }

    /// Diff header style
    #[inline]
    pub const fn diff_header(&self) -> Style {
        Style::new().fg(self.warning).add_modifier(Modifier::BOLD)
    }

    /// Loading spinner style
    #[inline]
    pub const fn spinner(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    /// Token count style
    #[inline]
    pub const fn token_count(&self) -> Style {
        Style::new().fg(self.success)
    }

    /// Model/provider info style
    #[inline]
    pub const fn model_info(&self) -> Style {
        Style::new().fg(self.warning)
    }

    // ===== Markdown Styles =====

    /// Markdown code block style
    #[inline]
    pub const fn md_code_block(&self) -> Style {
        Style::new().fg(self.accent_muted)
    }

    /// Markdown inline code style
    #[inline]
    pub const fn md_inline_code(&self) -> Style {
        Style::new().fg(self.accent)
    }

    /// Markdown bold text style
    #[inline]
    pub const fn md_bold(&self) -> Style {
        Style::new().fg(self.text).add_modifier(Modifier::BOLD)
    }

    /// Markdown header style
    #[inline]
    pub const fn md_header(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    /// Markdown link style
    #[inline]
    pub const fn md_link(&self) -> Style {
        Style::new()
            .fg(self.accent)
            .add_modifier(Modifier::UNDERLINED)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

/// Global theme instance
pub static THEME: std::sync::OnceLock<Theme> = std::sync::OnceLock::new();

/// Get the current theme
pub fn current() -> &'static Theme {
    THEME.get_or_init(Theme::dark)
}
