//! Code View widget with syntax highlighting

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Style as SyntaxStyle, ThemeSet},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

pub struct CodeView {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl CodeView {
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    pub fn render(&self, code: &str, language: &str, focused: bool) -> Paragraph<'static> {
        let border_style = if focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        let lines = if code.is_empty() {
            vec![Line::styled(
                "// No code loaded",
                Style::default().fg(Color::DarkGray),
            )]
        } else {
            self.highlight_code(code, language)
        };

        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Code [{}]", language))
                .title_style(Style::default().fg(Color::White))
                .border_style(border_style),
        )
    }

    fn highlight_code(&self, code: &str, language: &str) -> Vec<Line<'static>> {
        let syntax = self
            .syntax_set
            .find_syntax_by_name(language)
            .or_else(|| self.syntax_set.find_syntax_by_extension(language))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes["base16-eighties.dark"];

        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut lines = Vec::new();

        for line in LinesWithEndings::from(code) {
            let spans: Vec<Span<'_>> = highlighter
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default()
                .into_iter()
                .map(|(style, text)| convert_style(style, text))
                .collect();

            lines.push(Line::from(spans));
        }

        lines
    }
}

fn convert_style(style: SyntaxStyle, text: &str) -> Span<'static> {
    let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);

    let mut span_style = Style::default().fg(fg);

    if style
        .font_style
        .contains(syntect::highlighting::FontStyle::BOLD)
    {
        span_style = span_style.add_modifier(ratatui::style::Modifier::BOLD);
    }
    if style
        .font_style
        .contains(syntect::highlighting::FontStyle::ITALIC)
    {
        span_style = span_style.add_modifier(ratatui::style::Modifier::ITALIC);
    }
    if style
        .font_style
        .contains(syntect::highlighting::FontStyle::UNDERLINE)
    {
        span_style = span_style.add_modifier(ratatui::style::Modifier::UNDERLINED);
    }

    Span::styled(text.to_string(), span_style)
}

impl Default for CodeView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_view_new() {
        let view = CodeView::new();
        assert!(view.syntax_set.syntaxes().len() > 0);
    }

    #[test]
    fn test_render_empty_code() {
        let view = CodeView::new();
        let result = view.render("", "rust", false);
        let _ = result;
    }
}
