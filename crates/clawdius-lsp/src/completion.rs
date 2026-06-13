//! Completion item generation for `textDocument/completion`.
//!
//! Generates completion candidates from workspace symbols and
//! Rust language keywords, filtered by the prefix at the cursor.

use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionResponse, Documentation, InsertTextFormat,
    MarkupContent, MarkupKind, Position, SymbolKind,
};

use crate::symbol_index::IndexedSymbol;

/// Generate completion items from symbols in the workspace.
#[must_use]
pub fn generate_completions(
    symbols: &[&IndexedSymbol],
    prefix: &str,
    max_items: usize,
) -> Vec<CompletionItem> {
    symbols
        .iter()
        .filter(|s| s.name.starts_with(prefix))
        .take(max_items)
        .map(|s| CompletionItem {
            label: s.name.clone(),
            kind: Some(map_symbol_kind_to_completion_kind(s.kind)),
            detail: s.doc_comment.clone(),
            documentation: s.doc_comment.as_ref().map(|d| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: d.clone(),
                })
            }),
            insert_text: Some(s.name.clone()),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..Default::default()
        })
        .collect()
}

/// Map an LSP [`SymbolKind`] to the closest [`CompletionItemKind`].
const fn map_symbol_kind_to_completion_kind(kind: SymbolKind) -> CompletionItemKind {
    match kind {
        SymbolKind::FUNCTION => CompletionItemKind::FUNCTION,
        SymbolKind::STRUCT => CompletionItemKind::STRUCT,
        SymbolKind::ENUM => CompletionItemKind::ENUM,
        SymbolKind::INTERFACE => CompletionItemKind::INTERFACE,
        SymbolKind::CONSTANT => CompletionItemKind::CONSTANT,
        SymbolKind::MODULE | SymbolKind::NAMESPACE => CompletionItemKind::MODULE,
        SymbolKind::CLASS => CompletionItemKind::CLASS,
        _ => CompletionItemKind::TEXT,
    }
}

/// Keyword completions for Rust.
#[must_use]
pub fn rust_keyword_completions(prefix: &str) -> Vec<CompletionItem> {
    let keywords = [
        "fn", "let", "mut", "pub", "struct", "enum", "trait", "impl", "match", "if", "else", "for",
        "while", "loop", "return", "break", "continue", "use", "mod", "crate", "self", "super",
        "async", "await", "move", "ref", "static", "const", "unsafe", "extern", "where", "type",
        "dyn", "as", "in",
    ];

    keywords
        .iter()
        .filter(|k| k.starts_with(prefix))
        .map(|k| CompletionItem {
            label: (*k).to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            insert_text: Some((*k).to_string()),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..Default::default()
        })
        .collect()
}

/// Extract the word prefix at the given cursor position.
///
/// Returns the contiguous sequence of identifier characters
/// (alphanumeric + underscore) ending at `position`.
#[must_use]
pub fn get_word_at_position(text: &str, position: Position) -> String {
    let line = text.lines().nth(position.line as usize);
    let Some(line) = line else {
        return String::new();
    };

    let char_idx = (position.character as usize).min(line.len());
    let bytes = line.as_bytes();

    let mut start = char_idx;
    while start > 0 && is_ident_char(bytes[start - 1]) {
        start -= 1;
    }

    line[start..char_idx].to_string()
}

/// Build a [`CompletionResponse`] from a list of completion items.
#[must_use]
pub fn completion_response(items: Vec<CompletionItem>) -> Option<CompletionResponse> {
    if items.is_empty() {
        None
    } else {
        Some(CompletionResponse::Array(items))
    }
}

const fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn make_symbol(name: &str, kind: SymbolKind) -> IndexedSymbol {
        IndexedSymbol {
            name: name.to_string(),
            kind,
            line: 0,
            character: 0,
            end_character: name.len() as u32,
            uri: "file:///test.rs".to_string(),
            definition_line: format!("{name}() {{}}"),
            doc_comment: None,
        }
    }

    #[test]
    fn test_generate_completions_filters_by_prefix() {
        let s1 = make_symbol("hello", SymbolKind::FUNCTION);
        let s2 = make_symbol("help", SymbolKind::FUNCTION);
        let s3 = make_symbol("world", SymbolKind::FUNCTION);
        let symbols = vec![&s1, &s2, &s3];

        let completions = generate_completions(&symbols, "hel", 50);
        assert_eq!(completions.len(), 2);
        assert!(completions.iter().all(|c| c.label.starts_with("hel")));
    }

    #[test]
    fn test_generate_completions_respects_max_items() {
        let symbols: Vec<IndexedSymbol> = (0..10)
            .map(|i| make_symbol(&format!("f{i}"), SymbolKind::FUNCTION))
            .collect();
        let refs: Vec<&IndexedSymbol> = symbols.iter().collect();
        let completions = generate_completions(&refs, "f", 3);
        assert_eq!(completions.len(), 3);
    }

    #[test]
    fn test_keyword_completions() {
        let completions = rust_keyword_completions("fn");
        assert!(completions.iter().any(|c| c.label == "fn"));
        assert!(completions.iter().all(|c| c.label.starts_with("fn")));
    }

    #[test]
    fn test_keyword_completions_empty_prefix() {
        let completions = rust_keyword_completions("");
        assert!(completions.len() > 10);
    }

    #[test]
    fn test_get_word_at_position() {
        let text = "fn hello_world() {}";
        let prefix = get_word_at_position(text, Position::new(0, 7));
        assert_eq!(prefix, "hell");
    }

    #[test]
    fn test_get_word_at_position_start_of_line() {
        let text = "let x = 1;";
        let prefix = get_word_at_position(text, Position::new(0, 0));
        assert_eq!(prefix, "");
    }

    #[test]
    fn test_map_symbol_kind() {
        assert_eq!(
            map_symbol_kind_to_completion_kind(SymbolKind::FUNCTION),
            CompletionItemKind::FUNCTION
        );
        assert_eq!(
            map_symbol_kind_to_completion_kind(SymbolKind::STRUCT),
            CompletionItemKind::STRUCT
        );
        assert_eq!(
            map_symbol_kind_to_completion_kind(SymbolKind::ENUM),
            CompletionItemKind::ENUM
        );
        assert_eq!(
            map_symbol_kind_to_completion_kind(SymbolKind::INTERFACE),
            CompletionItemKind::INTERFACE
        );
    }

    #[test]
    fn test_completion_response_empty() {
        assert!(completion_response(Vec::new()).is_none());
    }

    #[test]
    fn test_completion_response_nonempty() {
        let item = CompletionItem {
            label: "test".to_string(),
            ..Default::default()
        };
        assert!(completion_response(vec![item]).is_some());
    }

    #[test]
    fn test_doc_comment_in_completion() {
        let mut sym = make_symbol("foo", SymbolKind::FUNCTION);
        sym.doc_comment = Some("A useful function".to_string());
        let completions = generate_completions(&[&sym], "foo", 50);
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].detail.as_deref(), Some("A useful function"));
    }
}
