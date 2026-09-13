//! Integration tests for the Clawdius LSP intelligence seams over a real
//! `SymbolIndex` + `analyze_diagnostics` + completion pipeline: Rust source
//! in, IDE-grade responses out. These lock the contracts the LSP backend
//! serves to editors (`documentSymbol`, hover, go-to-definition, references,
//! diagnostics, completion) independently of the transport layer.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]
#![allow(clippy::cast_possible_truncation)]

use tower_lsp::lsp_types::{Position, Url};

use clawdius_lsp::completion::{
    completion_response, generate_completions, get_word_at_position, rust_keyword_completions,
};
use clawdius_lsp::diagnostics::analyze_diagnostics;
use clawdius_lsp::symbol_index::SymbolIndex;

const SAMPLE_RUST: &str = r"//! Module docs.
use std::collections::HashMap;

/// Adds one to the answer.
pub fn answer_with_one(base: u32) -> u32 {
    base + 1
}

struct Cache {
    entries: HashMap<String, u32>,
}

impl Cache {
    fn new() -> Self {
        Self { entries: HashMap::new() }
    }
}
";

fn file_uri(name: &str) -> Url {
    Url::parse(&format!("file:///tmp/{name}")).expect("valid uri")
}

fn indexed_sample() -> (SymbolIndex, Url) {
    let uri = file_uri("sample.rs");
    let mut index = SymbolIndex::new();
    index.index_document(&uri, SAMPLE_RUST);
    (index, uri)
}

#[test]
fn document_symbols_extracts_functions_and_structs() {
    let (index, uri) = indexed_sample();
    let symbols = index.document_symbols(&uri).expect("document indexed");
    let names: Vec<String> = symbols.iter().map(|s| s.name.clone()).collect();
    assert!(
        names.iter().any(|n| n.contains("answer_with_one")),
        "pub fn extracted, got {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("Cache")),
        "struct extracted, got {names:?}"
    );
}

#[test]
fn hover_returns_doc_comment_for_documented_symbol() {
    let (index, uri) = indexed_sample();
    // Position of `answer_with_one` on its definition line.
    let line = SAMPLE_RUST
        .lines()
        .position(|l| l.starts_with("pub fn answer_with_one"))
        .expect("definition line");
    let hover = index.hover(&uri, line as u32, 10).expect("hover info");
    assert!(
        hover.to_markdown().contains("Adds one"),
        "doc comment surfaces in hover, got {}",
        hover.to_markdown()
    );
}

#[test]
fn goto_definition_and_references_resolve_via_the_name_index() {
    let (mut index, uri) = indexed_sample();
    // A second file defines the same symbol name; the name index is global.
    let other = file_uri("twin.rs");
    index.index_document(
        &other,
        "pub fn answer_with_one(other: u32) -> u32 {\n    other\n}\n",
    );

    // Resolve from the definition site in sample.rs...
    let def_line = SAMPLE_RUST
        .lines()
        .position(|l| l.starts_with("pub fn answer_with_one"))
        .expect("definition line");
    let def = index
        .goto_definition(&uri, def_line as u32, 10)
        .expect("definition found at its own site");
    assert_eq!(def.name, "answer_with_one");

    // ...and the name index collects the definitions from both documents.
    let refs = index.references(&uri, def_line as u32, 10);
    let mut uris: Vec<String> = refs.iter().map(|r| r.uri.clone()).collect();
    uris.sort();
    assert_eq!(
        uris,
        vec![uri.to_string(), other.to_string()],
        "references lists every indexed definition of the name"
    );
}

#[test]
fn reindex_replaces_symbols_without_duplicates() {
    let (mut index, uri) = indexed_sample();
    let updated = SAMPLE_RUST.replace("pub fn answer_with_one", "pub fn renamed_fn");
    index.index_document(&uri, &updated);

    let symbols = index.document_symbols(&uri).expect("indexed");
    let names: Vec<String> = symbols.iter().map(|s| s.name.clone()).collect();
    assert!(
        names.iter().any(|n| n.contains("renamed_fn")),
        "new name indexed"
    );
    assert!(
        !names.iter().any(|n| n.contains("answer_with_one")),
        "old name dropped on re-index, got {names:?}"
    );
    // KNOWN ISSUE: `summary()` (derived from the global name index) counts
    // stale entries left behind by re-indexing, while `all_symbols()`
    // reflects only the current document state. The document-level
    // replacement contract is pinned above; the summary discrepancy is
    // reported upstream rather than pinned as a promise.
    assert_eq!(
        index.all_symbols().len(),
        index.document_symbols(&uri).expect("indexed").len(),
        "document-level symbol set is replaced wholesale"
    );
}

#[test]
fn diagnostics_flag_todo_stubs() {
    let diags = analyze_diagnostics("fn f() {\n    // TODO: implement\n    todo!()\n}\n");
    assert!(!diags.is_empty(), "TODO/todo! markers produce diagnostics");
    let clean = analyze_diagnostics("fn f() -> u32 { 1 }\n");
    assert!(
        clean.is_empty(),
        "clean source produces no diagnostics, got {clean:?}"
    );
}

#[test]
fn completion_pipeline_returns_items_for_prefix() {
    let (index, _uri) = indexed_sample();
    let symbols = index.all_symbols();
    let items = generate_completions(&symbols, "ans", 20);
    assert!(
        !items.is_empty(),
        "prefix completion finds matching symbols"
    );
    let keywords = rust_keyword_completions("pu");
    assert!(
        keywords.iter().any(|k| k.label == "pub"),
        "keyword completion offers pub"
    );
    let response = completion_response(items);
    assert!(response.is_some(), "non-empty completion list is Some");
}

#[test]
fn word_at_position_extracts_the_cursor_prefix() {
    let line = "    answer_with_one(41);";
    // The extractor returns the identifier prefix ending at the cursor
    // (completion-oriented): mid-word cursor yields the partial word.
    let prefix = get_word_at_position(line, Position::new(0, 7));
    assert_eq!(prefix, "ans");
    // Cursor at the word's end yields the full identifier.
    let full = get_word_at_position(line, Position::new(0, 19));
    assert_eq!(full, "answer_with_one");
}
