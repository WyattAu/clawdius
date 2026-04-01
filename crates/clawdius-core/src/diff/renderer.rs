//! Diff rendering for terminal and HTML output

use super::{DiffLine, FileDiff};

/// Color representation (RGB)
#[derive(Debug, Clone, Copy)]
pub struct Color {
    /// Red component (0-255)
    pub r: u8,
    /// Green component (0-255)
    pub g: u8,
    /// Blue component (0-255)
    pub b: u8,
}

impl Color {
    /// Create a new color
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Convert to ANSI escape sequence
    #[must_use]
    pub fn to_ansi(&self) -> String {
        format!("\x1b[38;2;{};{};{}m", self.r, self.g, self.b)
    }

    /// Convert to hex color string
    #[must_use]
    pub fn to_hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::new(255, 255, 255)
    }
}

/// Theme for diff rendering
#[derive(Debug, Clone)]
pub struct DiffTheme {
    /// Color for added lines
    pub addition_color: Color,
    /// Color for deleted lines
    pub deletion_color: Color,
    /// Color for context lines
    pub context_color: Color,
    /// Color for headers
    pub header_color: Color,
}

impl Default for DiffTheme {
    fn default() -> Self {
        Self {
            addition_color: Color::new(46, 160, 67),  // Green
            deletion_color: Color::new(248, 81, 73),  // Red
            context_color: Color::new(200, 200, 200), // Gray
            header_color: Color::new(100, 149, 237),  // Cornflower blue
        }
    }
}

impl DiffTheme {
    /// Create a dark theme
    #[must_use]
    pub fn dark() -> Self {
        Self {
            addition_color: Color::new(78, 201, 176),
            deletion_color: Color::new(255, 85, 85),
            context_color: Color::new(150, 150, 150),
            header_color: Color::new(130, 170, 255),
        }
    }

    /// Create a light theme
    #[must_use]
    pub fn light() -> Self {
        Self {
            addition_color: Color::new(34, 139, 34),
            deletion_color: Color::new(220, 20, 60),
            context_color: Color::new(105, 105, 105),
            header_color: Color::new(65, 105, 225),
        }
    }
}

/// Renderer for diffs
pub struct DiffRenderer {
    /// Theme to use for rendering
    pub theme: DiffTheme,
}

impl Default for DiffRenderer {
    fn default() -> Self {
        Self::new(DiffTheme::default())
    }
}

impl DiffRenderer {
    /// Create a new renderer with the given theme
    #[must_use]
    pub fn new(theme: DiffTheme) -> Self {
        Self { theme }
    }

    /// Render diff for terminal with ANSI colors
    #[must_use]
    pub fn render_terminal(&self, diff: &FileDiff) -> String {
        let mut output = String::new();
        let reset = "\x1b[0m";
        let bold = "\x1b[1m";

        let path_str = diff.path.to_string_lossy();
        let header_color = self.theme.header_color.to_ansi();

        output.push_str(&format!("{bold}{header_color}--- {path_str}{reset}\n"));
        output.push_str(&format!("{bold}{header_color}+++ {path_str}{reset}\n"));

        for hunk in &diff.hunks {
            output.push_str(&format!(
                "{}{}@@ -{},{} +{},{} @@{}\n",
                header_color,
                bold,
                hunk.old_start,
                hunk.old_lines,
                hunk.new_start,
                hunk.new_lines,
                reset
            ));

            for line in &hunk.lines {
                let (prefix, content, color) = match line {
                    DiffLine::Context(l) => (" ", l.as_str(), &self.theme.context_color),
                    DiffLine::Added(l) => ("+", l.as_str(), &self.theme.addition_color),
                    DiffLine::Removed(l) => ("-", l.as_str(), &self.theme.deletion_color),
                };
                output.push_str(&color.to_ansi());
                output.push_str(prefix);
                output.push_str(content);
                output.push_str(reset);
                if !content.ends_with('\n') {
                    output.push('\n');
                }
            }
        }

        output
    }

    /// Render diff as HTML for webview
    #[must_use]
    pub fn render_html(&self, diff: &FileDiff) -> String {
        let mut html = String::new();

        html.push_str("<div class=\"diff-container\">\n");

        let path_str = diff.path.to_string_lossy();
        html.push_str(&format!(
            "  <div class=\"diff-header\">{}</div>\n",
            html_escape(&path_str)
        ));

        for hunk in &diff.hunks {
            html.push_str("  <div class=\"diff-hunk\">\n");
            html.push_str(&format!(
                "    <div class=\"diff-hunk-header\">@@ -{},{} +{},{} @@</div>\n",
                hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
            ));

            html.push_str("    <div class=\"diff-lines\">\n");

            for line in &hunk.lines {
                match line {
                    DiffLine::Context(l) => {
                        html.push_str(&format!(
                            "      <div class=\"diff-line context\"><span class=\"prefix\"> </span>{}</div>\n",
                            html_escape(l)
                        ));
                    },
                    DiffLine::Added(l) => {
                        html.push_str(&format!(
                            "      <div class=\"diff-line added\"><span class=\"prefix\">+</span>{}</div>\n",
                            html_escape(l)
                        ));
                    },
                    DiffLine::Removed(l) => {
                        html.push_str(&format!(
                            "      <div class=\"diff-line removed\"><span class=\"prefix\">-</span>{}</div>\n",
                            html_escape(l)
                        ));
                    },
                }
            }

            html.push_str("    </div>\n");
            html.push_str("  </div>\n");
        }

        html.push_str("</div>\n");

        html
    }

    /// Get CSS styles for HTML rendering
    #[must_use]
    pub fn get_css(&self) -> String {
        format!(
            r".diff-container {{
  font-family: monospace;
  font-size: 13px;
  line-height: 1.5;
  background: #1e1e1e;
  color: #d4d4d4;
  border-radius: 4px;
  overflow: hidden;
}}

.diff-header {{
  background: #2d2d2d;
  padding: 8px 12px;
  font-weight: bold;
  border-bottom: 1px solid #3d3d3d;
}}

.diff-hunk {{
  border-top: 1px solid #3d3d3d;
}}

.diff-hunk-header {{
  background: #2d2d2d;
  padding: 4px 12px;
  color: #{:02x}{:02x}{:02x};
  font-size: 12px;
}}

.diff-lines {{
  padding: 0;
}}

.diff-line {{
  padding: 1px 12px;
  white-space: pre;
  display: flex;
}}

.diff-line .prefix {{
  width: 15px;
  display: inline-block;
}}

.diff-line.context {{
  color: #{:02x}{:02x}{:02x};
}}

.diff-line.added {{
  background: rgba({}, {}, {}, 0.2);
  color: #{:02x}{:02x}{:02x};
}}

.diff-line.removed {{
  background: rgba({}, {}, {}, 0.2);
  color: #{:02x}{:02x}{:02x};
}}",
            self.theme.header_color.r,
            self.theme.header_color.g,
            self.theme.header_color.b,
            self.theme.context_color.r,
            self.theme.context_color.g,
            self.theme.context_color.b,
            self.theme.addition_color.r,
            self.theme.addition_color.g,
            self.theme.addition_color.b,
            self.theme.addition_color.r,
            self.theme.addition_color.g,
            self.theme.addition_color.b,
            self.theme.deletion_color.r,
            self.theme.deletion_color.g,
            self.theme.deletion_color.b,
            self.theme.deletion_color.r,
            self.theme.deletion_color.g,
            self.theme.deletion_color.b,
        )
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\n', "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_render_terminal() {
        let old = "line1\nline2\n";
        let new = "line1\nmodified\n";

        let diff = FileDiff::compute(PathBuf::from("test.txt"), Some(old), new);
        let renderer = DiffRenderer::default();
        let output = renderer.render_terminal(&diff);

        assert!(output.contains("test.txt"));
        assert!(output.contains('\x1b'));
    }

    #[test]
    fn test_render_html() {
        let old = "line1\nline2\n";
        let new = "line1\nmodified\n";

        let diff = FileDiff::compute(PathBuf::from("test.txt"), Some(old), new);
        let renderer = DiffRenderer::default();
        let html = renderer.render_html(&diff);

        assert!(html.contains("<div class=\"diff-container\">"));
        assert!(html.contains("diff-line added"));
    }
}
