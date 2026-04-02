//! Refactoring actions.

use super::{ActionContext, ActionEdit, ActionKind, Applicability, CodeAction, Range, TextEdit};
use crate::Result;

pub struct ExtractFunction;

impl CodeAction for ExtractFunction {
    fn id(&self) -> &'static str {
        "refactor.extract.function"
    }

    fn title(&self) -> &'static str {
        "Extract to function"
    }

    fn applicability(&self, context: &ActionContext) -> Applicability {
        if context.selection.is_some() {
            Applicability::WhenSelected
        } else {
            Applicability::Never
        }
    }

    fn execute(&self, context: &ActionContext) -> Result<ActionEdit> {
        let selection = context.selection.as_ref().ok_or_else(|| {
            crate::Error::InvalidInput("Selection required for extract function".to_string())
        })?;

        let function_name = generate_function_name(selection, &context.language);
        let function_code = format!(
            "fn {}() {{\n    {}\n}}\n\n",
            function_name,
            indent_selection(selection)
        );

        let selection_range = infer_selection_range(context);

        Ok(ActionEdit {
            edits: vec![
                TextEdit {
                    range: Range {
                        start: super::Position { line: 0, column: 0 },
                        end: super::Position { line: 0, column: 0 },
                    },
                    new_text: function_code,
                },
                TextEdit {
                    range: selection_range,
                    new_text: format!("{function_name}()"),
                },
            ],
            title: self.title().to_string(),
            kind: ActionKind::Refactor,
        })
    }
}

pub struct ExtractVariable;

impl CodeAction for ExtractVariable {
    fn id(&self) -> &'static str {
        "refactor.extract.variable"
    }

    fn title(&self) -> &'static str {
        "Extract to variable"
    }

    fn applicability(&self, context: &ActionContext) -> Applicability {
        if context.selection.is_some() {
            Applicability::WhenSelected
        } else {
            Applicability::Never
        }
    }

    fn execute(&self, context: &ActionContext) -> Result<ActionEdit> {
        let selection = context.selection.as_ref().ok_or_else(|| {
            crate::Error::InvalidInput("Selection required for extract variable".to_string())
        })?;

        let var_name = infer_variable_name(selection);
        let var_declaration = format!("let {} = {};\n", var_name, selection.trim());

        let selection_range = infer_selection_range(context);

        Ok(ActionEdit {
            edits: vec![
                TextEdit {
                    range: Range {
                        start: super::Position {
                            line: context.position.line,
                            column: 0,
                        },
                        end: super::Position {
                            line: context.position.line,
                            column: 0,
                        },
                    },
                    new_text: var_declaration,
                },
                TextEdit {
                    range: selection_range,
                    new_text: var_name.clone(),
                },
            ],
            title: self.title().to_string(),
            kind: ActionKind::Refactor,
        })
    }
}

pub struct InlineVariable;

impl CodeAction for InlineVariable {
    fn id(&self) -> &'static str {
        "refactor.inline.variable"
    }

    fn title(&self) -> &'static str {
        "Inline variable"
    }

    fn applicability(&self, context: &ActionContext) -> Applicability {
        if let Some(symbol) = &context.symbol_at_position {
            if matches!(symbol.kind, super::SymbolKind::Variable) {
                Applicability::WhenSelected
            } else {
                Applicability::Never
            }
        } else {
            Applicability::Never
        }
    }

    fn execute(&self, context: &ActionContext) -> Result<ActionEdit> {
        let symbol = context
            .symbol_at_position
            .as_ref()
            .ok_or_else(|| crate::Error::InvalidInput("Symbol at position required".to_string()))?;

        let value = extract_variable_value(&context.document, &symbol.name)?;

        let var_pattern = format!(r"\b{}\s*=", symbol.name);
        let var_line_start = find_pattern_line(&context.document, &var_pattern)?;

        Ok(ActionEdit {
            edits: vec![
                TextEdit {
                    range: Range {
                        start: super::Position {
                            line: var_line_start,
                            column: 0,
                        },
                        end: super::Position {
                            line: var_line_start + 1,
                            column: 0,
                        },
                    },
                    new_text: String::new(),
                },
                TextEdit {
                    range: Range {
                        start: super::Position {
                            line: context.position.line,
                            column: context.position.column,
                        },
                        end: super::Position {
                            line: context.position.line,
                            column: context.position.column + symbol.name.len(),
                        },
                    },
                    new_text: value,
                },
            ],
            title: self.title().to_string(),
            kind: ActionKind::Refactor,
        })
    }
}

pub struct RenameSymbol;

impl CodeAction for RenameSymbol {
    fn id(&self) -> &'static str {
        "refactor.rename"
    }

    fn title(&self) -> &'static str {
        "Rename symbol"
    }

    fn applicability(&self, context: &ActionContext) -> Applicability {
        if context.symbol_at_position.is_some() {
            Applicability::Always
        } else {
            Applicability::Never
        }
    }

    fn execute(&self, context: &ActionContext) -> Result<ActionEdit> {
        let symbol = context
            .symbol_at_position
            .as_ref()
            .ok_or_else(|| crate::Error::InvalidInput("Symbol at position required".to_string()))?;

        let new_name = format!("new_{}", symbol.name.to_lowercase());
        let occurrences = find_all_occurrences(&context.document, &symbol.name);

        let edits: Vec<TextEdit> = occurrences
            .into_iter()
            .map(|(line, col)| TextEdit {
                range: Range {
                    start: super::Position { line, column: col },
                    end: super::Position {
                        line,
                        column: col + symbol.name.len(),
                    },
                },
                new_text: new_name.clone(),
            })
            .collect();

        Ok(ActionEdit {
            edits,
            title: format!("Rename '{}' to '{}'", symbol.name, new_name),
            kind: ActionKind::Refactor,
        })
    }
}

pub struct MoveToModule;

impl CodeAction for MoveToModule {
    fn id(&self) -> &'static str {
        "refactor.move.module"
    }

    fn title(&self) -> &'static str {
        "Move to module"
    }

    fn applicability(&self, context: &ActionContext) -> Applicability {
        if let Some(symbol) = &context.symbol_at_position {
            if matches!(
                symbol.kind,
                super::SymbolKind::Function | super::SymbolKind::Struct | super::SymbolKind::Enum
            ) {
                Applicability::WhenSelected
            } else {
                Applicability::Never
            }
        } else {
            Applicability::Never
        }
    }

    fn execute(&self, context: &ActionContext) -> Result<ActionEdit> {
        let symbol = context
            .symbol_at_position
            .as_ref()
            .ok_or_else(|| crate::Error::InvalidInput("Symbol at position required".to_string()))?;

        let module_name = format!("{}_module", symbol.name.to_lowercase());
        let code = extract_symbol_code(&context.document, symbol)?;

        let module_code = format!("mod {} {{\n    {}\n}}\n", module_name, indent_code(&code));

        let symbol_range = symbol.range.clone();

        Ok(ActionEdit {
            edits: vec![
                TextEdit {
                    range: Range {
                        start: super::Position { line: 0, column: 0 },
                        end: super::Position { line: 0, column: 0 },
                    },
                    new_text: module_code,
                },
                TextEdit {
                    range: symbol_range,
                    new_text: format!("{}::{}", module_name, symbol.name),
                },
            ],
            title: format!("Move '{}' to module", symbol.name),
            kind: ActionKind::Refactor,
        })
    }
}

fn generate_function_name(selection: &str, _language: &str) -> String {
    let trimmed = selection.trim();
    let words: Vec<&str> = trimmed.split_whitespace().take(3).collect();

    if words.is_empty() {
        return "extracted_function".to_string();
    }

    let base_name: String = words
        .iter()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect();

    format!("extracted_{}", base_name.to_lowercase())
}

fn indent_selection(selection: &str) -> String {
    selection
        .lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                format!("    {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn infer_variable_name(selection: &str) -> String {
    let trimmed = selection.trim();

    if trimmed.contains('+') {
        return "sum".to_string();
    }
    if trimmed.contains('*') || trimmed.contains('×') {
        return "product".to_string();
    }
    if trimmed.starts_with('"') || trimmed.starts_with('\'') {
        return "text".to_string();
    }
    if trimmed.chars().all(|c| c.is_ascii_digit()) {
        return "number".to_string();
    }

    "value".to_string()
}

fn infer_selection_range(context: &ActionContext) -> Range {
    Range {
        start: super::Position {
            line: context.position.line,
            column: 0,
        },
        end: super::Position {
            line: context.position.line,
            column: context
                .document
                .lines()
                .nth(context.position.line)
                .map_or(0, str::len),
        },
    }
}

fn extract_variable_value(document: &str, var_name: &str) -> Result<String> {
    let pattern = format!(r"let\s+{var_name}\s*=\s*([^;]+);");
    let re = regex::Regex::new(&pattern)
        .map_err(|e| crate::Error::ParseError(format!("Failed to compile regex: {e}")))?;

    if let Some(caps) = re.captures(document) {
        Ok(caps[1].trim().to_string())
    } else {
        Err(crate::Error::NotFound(format!(
            "Variable '{var_name}' not found"
        )))
    }
}

fn find_pattern_line(document: &str, pattern: &str) -> Result<usize> {
    let re = regex::Regex::new(pattern)
        .map_err(|e| crate::Error::ParseError(format!("Failed to compile regex: {e}")))?;

    for (i, line) in document.lines().enumerate() {
        if re.is_match(line) {
            return Ok(i);
        }
    }

    Err(crate::Error::NotFound(format!(
        "Pattern '{pattern}' not found"
    )))
}

fn find_all_occurrences(document: &str, name: &str) -> Vec<(usize, usize)> {
    let mut occurrences = Vec::new();
    let pattern = format!(r"\b{}\b", regex::escape(name));

    if let Ok(re) = regex::Regex::new(&pattern) {
        for (line_num, line) in document.lines().enumerate() {
            for cap in re.find_iter(line) {
                occurrences.push((line_num, cap.start()));
            }
        }
    }

    occurrences
}

fn extract_symbol_code(document: &str, symbol: &super::Symbol) -> Result<String> {
    let lines: Vec<&str> = document.lines().collect();
    let start = symbol.range.start.line;
    let end = symbol.range.end.line;

    if start >= lines.len() || end >= lines.len() {
        return Err(crate::Error::OutOfRange);
    }

    Ok(lines[start..=end].join("\n"))
}

fn indent_code(code: &str) -> String {
    code.lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                format!("    {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
