use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};
use std::path::Path;

#[allow(dead_code)]
pub struct MentionAutocomplete {
    suggestions: Vec<MentionSuggestion>,
    state: ListState,
    visible: bool,
    input: String,
    cursor_position: usize,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct MentionSuggestion {
    pub mention_type: String,
    pub value: String,
    pub description: String,
}

#[allow(dead_code)]
impl MentionAutocomplete {
    pub fn new() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));

        Self {
            suggestions: Vec::new(),
            state,
            visible: false,
            input: String::new(),
            cursor_position: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, partial: &str, working_dir: &Path) {
        self.visible = true;
        self.input = partial.to_string();
        self.suggestions = Self::get_suggestions(partial, working_dir);
        if !self.suggestions.is_empty() {
            self.state.select(Some(0));
        }
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.suggestions.clear();
    }

    pub fn next(&mut self) {
        if !self.suggestions.is_empty() {
            let i = match self.state.selected() {
                Some(i) => {
                    if i >= self.suggestions.len() - 1 {
                        0
                    } else {
                        i + 1
                    }
                },
                None => 0,
            };
            self.state.select(Some(i));
        }
    }

    pub fn previous(&mut self) {
        if !self.suggestions.is_empty() {
            let i = match self.state.selected() {
                Some(i) => {
                    if i == 0 {
                        self.suggestions.len() - 1
                    } else {
                        i - 1
                    }
                },
                None => 0,
            };
            self.state.select(Some(i));
        }
    }

    pub fn selected(&self) -> Option<&MentionSuggestion> {
        self.state.selected().and_then(|i| self.suggestions.get(i))
    }

    fn get_suggestions(partial: &str, working_dir: &Path) -> Vec<MentionSuggestion> {
        let mut suggestions = Vec::new();

        if !partial.starts_with('@') {
            return suggestions;
        }

        let partial_lower = partial.to_lowercase();

        if partial_lower == "@" || partial_lower.starts_with("@f") {
            suggestions.push(MentionSuggestion {
                mention_type: "@file".to_string(),
                value: "@file:".to_string(),
                description: "Include file contents".to_string(),
            });
        }

        if partial_lower == "@" || partial_lower.starts_with("@fo") {
            suggestions.push(MentionSuggestion {
                mention_type: "@folder".to_string(),
                value: "@folder:".to_string(),
                description: "List folder contents".to_string(),
            });
        }

        if partial_lower == "@" || partial_lower.starts_with("@u") {
            suggestions.push(MentionSuggestion {
                mention_type: "@url".to_string(),
                value: "@url:".to_string(),
                description: "Fetch URL content".to_string(),
            });
        }

        if partial_lower == "@" || partial_lower.starts_with("@p") {
            suggestions.push(MentionSuggestion {
                mention_type: "@problems".to_string(),
                value: "@problems".to_string(),
                description: "Workspace diagnostics".to_string(),
            });
            suggestions.push(MentionSuggestion {
                mention_type: "@problems:error".to_string(),
                value: "@problems:error".to_string(),
                description: "Errors only".to_string(),
            });
            suggestions.push(MentionSuggestion {
                mention_type: "@problems:warning".to_string(),
                value: "@problems:warning".to_string(),
                description: "Warnings only".to_string(),
            });
        }

        if partial_lower == "@" || partial_lower.starts_with("@g") {
            suggestions.push(MentionSuggestion {
                mention_type: "@git:diff".to_string(),
                value: "@git:diff".to_string(),
                description: "Unstaged changes".to_string(),
            });
            suggestions.push(MentionSuggestion {
                mention_type: "@git:staged".to_string(),
                value: "@git:staged".to_string(),
                description: "Staged changes".to_string(),
            });
            suggestions.push(MentionSuggestion {
                mention_type: "@git:log:5".to_string(),
                value: "@git:log:5".to_string(),
                description: "Recent commits".to_string(),
            });
        }

        if partial_lower == "@" || partial_lower.starts_with("@s") {
            suggestions.push(MentionSuggestion {
                mention_type: "@symbol".to_string(),
                value: "@symbol:".to_string(),
                description: "Symbol definition".to_string(),
            });
            suggestions.push(MentionSuggestion {
                mention_type: "@search".to_string(),
                value: r#"@search:"""#.to_string(),
                description: "Search codebase".to_string(),
            });
        }

        if partial.starts_with("@file:") || partial.starts_with("@folder:") {
            let path_partial = partial
                .trim_start_matches("@file:")
                .trim_start_matches("@folder:");
            if let Ok(entries) = std::fs::read_dir(working_dir) {
                let mut matching_paths: Vec<String> = entries
                    .filter_map(std::result::Result::ok)
                    .filter_map(|e| e.file_name().to_str().map(std::string::ToString::to_string))
                    .filter(|s| s.starts_with(path_partial) && !s.starts_with('.'))
                    .take(10)
                    .collect();

                matching_paths.sort();

                for path in matching_paths {
                    let mention_type = if partial.starts_with("@file:") {
                        "@file"
                    } else {
                        "@folder"
                    };
                    suggestions.push(MentionSuggestion {
                        mention_type: mention_type.to_string(),
                        value: format!("{mention_type}:{path}"),
                        description: if std::path::Path::new(&working_dir.join(&path)).is_dir() {
                            "Directory".to_string()
                        } else {
                            "File".to_string()
                        },
                    });
                }
            }
        }

        suggestions
    }

    pub fn render(&mut self, f: &mut Frame<'_>, area: Rect) {
        if !self.visible || self.suggestions.is_empty() {
            return;
        }

        let popup_area = Rect {
            x: area.x,
            y: area
                .y
                .saturating_sub(self.suggestions.len().min(10) as u16 + 2),
            width: area.width.min(60),
            height: self.suggestions.len().min(10) as u16 + 2,
        };

        f.render_widget(Clear, popup_area);

        let items: Vec<ListItem<'_>> = self
            .suggestions
            .iter()
            .map(|s| {
                let style = Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD);

                let content = Line::from(vec![
                    Span::styled(&s.mention_type, style),
                    Span::raw(" "),
                    Span::styled(&s.description, Style::default().fg(Color::Gray)),
                ]);

                ListItem::new(content)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Mentions"))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        f.render_stateful_widget(list, popup_area, &mut self.state);
    }
}

impl Default for MentionAutocomplete {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
pub fn highlight_mentions(text: &str) -> Vec<Span<'_>> {
    let mut spans = Vec::new();
    let mut last_end = 0;

    let patterns = [
        r"@file:[^\s]+",
        r"@folder:[^\s]+",
        r"@url:https?://[^\s]+",
        r"@problems(?::\w+)?",
        r"@git:(?:diff|staged|log(?::\d+)?)",
        r"@symbol:[^\s]+",
        r#"@search:"[^"]+""#,
        r#"@search:(?!")[^\s]+"#,
    ];

    let mut mentions: Vec<(usize, usize)> = Vec::new();

    for pattern in &patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            for cap in re.find_iter(text) {
                mentions.push((cap.start(), cap.end()));
            }
        }
    }

    mentions.sort_by_key(|(start, _)| *start);
    mentions.dedup();

    for (start, end) in mentions {
        if start > last_end {
            spans.push(Span::raw(&text[last_end..start]));
        }

        let mention_style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::UNDERLINED);

        spans.push(Span::styled(&text[start..end], mention_style));
        last_end = end;
    }

    if last_end < text.len() {
        spans.push(Span::raw(&text[last_end..]));
    }

    spans
}
