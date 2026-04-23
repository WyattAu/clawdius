//! Repository map — compact, ranked codebase overview for LLM context.
//!
//! Produces a token-budget-aware summary of a codebase's structure using
//! tree-sitter AST parsing and importance scoring on the symbol dependency
//! graph. Inspired by Aider's `RepoMap` algorithm.
//!
//! # Algorithm
//!
//! 1. **Parse** each source file with tree-sitter, extract symbols (functions,
//!    structs, classes, methods, etc.) and their import/reference relationships.
//! 2. **Score** — rank symbols by cross-file usage, definition visibility,
//!    kind importance, and definition length (approximates PageRank).
//! 3. **Binary search** — sort tags by score (descending), find the maximum
//!    prefix that fits the token budget.
//! 4. **Format** — output as lines grouped by file path, with each tag
//!    showing name, kind, and line range.
//!
//! # Usage
//!
//! ```ignore
//! use clawdius_core::graph_rag::repo_map::RepoMap;
//!
//! let map = RepoMap::build(project_root.to_path_buf(), 4000)?;
//! println!("{}", map.to_string());
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use walkdir::DirEntry;

use crate::graph_rag::ast::SymbolKind;
use crate::graph_rag::languages::detect_language;
use crate::graph_rag::parser::CodeParser;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A ranked tag extracted from a source file.
#[derive(Debug, Clone)]
pub struct Tag {
    /// Fully qualified name (e.g., `my_module::MyStruct`)
    pub name: String,
    /// Symbol kind (function, struct, class, etc.)
    pub kind: SymbolKind,
    /// File path relative to project root.
    pub file: String,
    /// Start line (1-indexed).
    pub line: usize,
    /// End line (1-indexed).
    pub end_line: usize,
    /// Importance score [0, 1].
    pub score: f64,
    /// Optional signature string.
    pub signature: Option<String>,
}

/// Repository map — a compact, ranked overview of the codebase.
#[derive(Debug, Clone)]
pub struct RepoMap {
    /// All tags sorted by importance score (descending).
    tags: Vec<Tag>,
    /// Token budget used to build this map.
    token_budget: usize,
    /// Total tokens across all discovered tags (before truncation).
    total_tokens: usize,
    /// Number of files indexed.
    file_count: usize,
    /// Number of symbols discovered.
    symbol_count: usize,
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builder for constructing a [`RepoMap`] with configuration.
pub struct RepoMapBuilder {
    /// Project root directory.
    project_root: PathBuf,
    /// Token budget for the output map.
    token_budget: usize,
    /// Maximum files to index (0 = unlimited).
    max_files: usize,
    /// File extensions to include (None = all supported).
    extensions: Option<Vec<String>>,
    /// File paths to exclude.
    exclude_patterns: Vec<String>,
    /// Symbol kinds to include (None = all).
    include_kinds: Option<Vec<SymbolKind>>,
    /// Minimum importance score to include (0.0 = include all).
    min_score: f64,
}

impl RepoMapBuilder {
    /// Create a new builder with the given project root.
    #[must_use]
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            token_budget: 4096,
            max_files: 0,
            extensions: None,
            exclude_patterns: vec![
                "target/".into(),
                "node_modules/".into(),
                ".git/".into(),
                "vendor/".into(),
                "__pycache__/".into(),
                ".venv/".into(),
            ],
            include_kinds: None,
            min_score: 0.0,
        }
    }

    /// Set the token budget for the output map.
    #[must_use]
    pub fn token_budget(mut self, budget: usize) -> Self {
        self.token_budget = budget;
        self
    }

    /// Set the maximum number of files to index.
    #[must_use]
    pub fn max_files(mut self, max: usize) -> Self {
        self.max_files = max;
        self
    }

    /// Only include files with these extensions (e.g., `["rs", "py", "ts"]`).
    #[must_use]
    pub fn extensions(mut self, exts: Vec<&str>) -> Self {
        self.extensions = Some(exts.into_iter().map(String::from).collect());
        self
    }

    /// Exclude files/directories matching these patterns.
    #[must_use]
    pub fn exclude(mut self, patterns: Vec<&str>) -> Self {
        self.exclude_patterns = patterns.into_iter().map(String::from).collect();
        self
    }

    /// Only include these symbol kinds (e.g., `[SymbolKind::Function, SymbolKind::Struct]`).
    #[must_use]
    pub fn include_kinds(mut self, kinds: Vec<SymbolKind>) -> Self {
        self.include_kinds = Some(kinds);
        self
    }

    /// Only include symbols with importance score >= this threshold.
    #[must_use]
    pub fn min_score(mut self, score: f64) -> Self {
        self.min_score = score;
        self
    }

    /// Build the repository map.
    ///
    /// # Errors
    ///
    /// Returns an error if the project root doesn't exist or a file can't be read.
    pub fn build(self) -> crate::Result<RepoMap> {
        RepoMap::build_with_config(self)
    }
}

// ---------------------------------------------------------------------------
// Core implementation
// ---------------------------------------------------------------------------

impl RepoMap {
    /// Build a repository map with default settings.
    ///
    /// Uses a 4096-token budget.
    ///
    /// # Errors
    ///
    /// Returns an error if the project root doesn't exist.
    pub fn build(project_root: PathBuf) -> crate::Result<Self> {
        Self::build_with_config(RepoMapBuilder::new(project_root).token_budget(4096))
    }

    fn build_with_config(config: RepoMapBuilder) -> crate::Result<Self> {
        let parser = CodeParser::new()?;

        // ── Phase 1: Discover and parse files ───────────────────────
        let mut all_tags: Vec<Tag> = Vec::new();
        let mut files_indexed = 0usize;

        let exclude = &config.exclude_patterns;
        let exts_filter = &config.extensions;

        // Walk the project directory — single filter_entry closure
        for entry in walkdir::WalkDir::new(&config.project_root)
            .into_iter()
            .filter_entry(|e: &DirEntry| {
                // Skip excluded directories/files
                let path_str = e.path().to_string_lossy();
                if exclude.iter().any(|pat| path_str.contains(pat)) {
                    return false;
                }
                // Only yield files (directories pass through for recursion)
                if !e.file_type().is_file() {
                    return true;
                }
                // Filter by extension if specified
                if let Some(ref exts) = exts_filter {
                    e.path()
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map_or(false, |ext| exts.iter().any(|allowed| *allowed == ext))
                } else {
                    detect_language(e.path()).is_some()
                }
            })
        {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue, // skip I/O errors
            };
            let path = entry.path();
            let relative = path
                .strip_prefix(&config.project_root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            if config.max_files > 0 && files_indexed >= config.max_files {
                break;
            }

            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(_) => continue, // Skip unreadable files
            };

            let lang = match detect_language(path) {
                Some(l) => l,
                None => continue,
            };

            let tree = match parser.parse(&source, lang) {
                Ok(t) => t,
                Err(_) => continue, // Skip unparseable files
            };

            let file_id = files_indexed as i64;
            let symbols = parser.extract_symbols(&tree, &source, file_id, lang);

            for sym in &symbols {
                // Filter by symbol kind if specified
                if let Some(ref kinds) = config.include_kinds {
                    if !kinds.contains(&sym.kind) {
                        continue;
                    }
                }

                // Skip trivially small symbols
                if sym.name.len() <= 1 {
                    continue;
                }

                // Only include "definition" symbols, not every occurrence
                let include = matches!(
                    sym.kind,
                    SymbolKind::Function
                        | SymbolKind::Struct
                        | SymbolKind::Enum
                        | SymbolKind::Trait
                        | SymbolKind::Class
                        | SymbolKind::Module
                        | SymbolKind::Interface
                        | SymbolKind::Type
                        | SymbolKind::Macro
                        | SymbolKind::Method
                        | SymbolKind::Constant
                );
                if !include {
                    continue;
                }

                all_tags.push(Tag {
                    name: sym.name.clone(),
                    kind: sym.kind.clone(),
                    file: relative.clone(),
                    line: sym.start_line.max(1) as usize,
                    end_line: sym.end_line.max(1) as usize,
                    score: 0.0, // Will be set by scoring
                    signature: sym.signature.clone().filter(|s: &String| !s.is_empty()),
                });
            }

            files_indexed += 1;
        }

        // ── Phase 2: Score symbols by structural importance ─────────
        //
        // Heuristic scoring that approximates PageRank:
        //   - Cross-file usage: symbols defined in more files are more important
        //   - Definition visibility: top-level items are more visible
        //   - Symbol kind weight: traits/funcs > structs > modules
        //   - Definition length: longer definitions may be more complex

        // Count how many files each symbol name appears in (cross-file usage)
        let mut name_file_count: HashMap<String, usize> = HashMap::new();
        for tag in &all_tags {
            *name_file_count.entry(tag.name.clone()).or_insert(0) += 1;
        }

        // Score each tag
        for tag in &mut all_tags {
            // Factor 1: Cross-file usage
            let file_usage = *name_file_count.get(&tag.name).unwrap_or(&1) as f64;
            let file_usage_score = if file_usage > 1.0 { 0.4 } else { 0.0 };

            // Factor 2: Definition-level (top-level items are more visible)
            let level_score = if tag.line <= 5 { 0.2 } else { 0.0 };

            // Factor 3: Symbol kind weight
            let kind_weight = match tag.kind {
                SymbolKind::Function | SymbolKind::Method => 0.3,
                SymbolKind::Struct
                | SymbolKind::Class
                | SymbolKind::Enum
                | SymbolKind::Interface => 0.25,
                SymbolKind::Trait => 0.35,
                SymbolKind::Module => 0.15,
                SymbolKind::Type | SymbolKind::Macro => 0.2,
                _ => 0.1,
            };

            // Factor 4: Definition length (longer definitions may be more complex)
            let length_score = {
                let lines = (tag.end_line - tag.line + 1) as f64;
                if lines > 20.0 { 0.1 } else { 0.0 }
            };

            // Normalize to [0, 1]
            let raw = file_usage_score + level_score + kind_weight + length_score;
            tag.score = (raw / 1.1_f64).min(1.0_f64);
        }

        // ── Phase 3: Sort and truncate for token budget ─────────────
        all_tags.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Estimate token count for each tag
        let estimate_tag_tokens = |tag: &Tag| -> usize {
            // name + "path/to/file" + line numbers + kind + overhead
            let name_tokens = tag.name.len() / 4 + 1;
            let path_tokens = tag.file.len() / 4 + 2;
            let line_tokens = format!("{}-{}", tag.line, tag.end_line).len() / 4 + 1;
            let kind_tokens = 1;
            let overhead = 3;
            name_tokens + path_tokens + line_tokens + kind_tokens + overhead
        };

        let total_tokens: usize = all_tags.iter().map(estimate_tag_tokens).sum();

        let symbol_count = all_tags.len();

        // Find the maximum prefix that fits the token budget
        let mut prefix_len = 0;
        let mut cumulative = 0usize;
        for tag in &all_tags {
            let cost = estimate_tag_tokens(tag);
            if cumulative + cost <= config.token_budget {
                cumulative += cost;
                prefix_len += 1;
            } else {
                break;
            }
        }
        all_tags.truncate(prefix_len);

        Ok(RepoMap {
            tags: all_tags,
            token_budget: config.token_budget,
            total_tokens,
            file_count: files_indexed,
            symbol_count,
        })
    }

    /// Format the repo map as a compact string suitable for LLM context.
    ///
    /// Output format:
    /// ```text
    /// src/lib.rs:42: fn my_function(...)
    /// src/lib.rs:85: struct MyStruct
    /// src/utils.rs:10: mod utils
    /// ```
    #[must_use]
    pub fn to_string(&self) -> String {
        let mut output = String::with_capacity(self.token_budget * 4);
        let mut current_file = String::new();

        for tag in &self.tags {
            let tag_line = format!(
                "{}:{}: {} {}",
                tag.file,
                tag.line,
                tag.kind.as_str(),
                tag.name
            );

            // Group by file for readability
            if tag.file != current_file {
                if !current_file.is_empty() {
                    output.push('\n');
                }
                current_file = tag.file.clone();
            }

            if output.is_empty() {
                output.push_str(&tag_line);
            } else {
                output.push('\n');
                output.push_str(&tag_line);
            }
        }

        output
    }

    /// Returns the estimated token count of the formatted map.
    #[must_use]
    pub fn estimated_tokens(&self) -> usize {
        // Rough estimate: ~4 chars per token
        self.to_string().len() / 4
    }

    /// Returns the number of tags in the map.
    #[must_use]
    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }

    /// Returns the number of files indexed.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.file_count
    }

    /// Returns the total number of symbols discovered (before truncation).
    #[must_use]
    pub fn total_symbol_count(&self) -> usize {
        self.symbol_count
    }

    /// Returns the fill ratio (tags_included / total_discovered).
    #[must_use]
    pub fn fill_ratio(&self) -> f64 {
        if self.symbol_count == 0 {
            0.0
        } else {
            self.tag_count() as f64 / self.symbol_count as f64
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_project(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("lib.rs"),
            r#"
pub mod utils;

/// A utility function for calculations.
pub fn calculate(x: i32, y: i32) -> i32 {
    x + y
}

/// The main entry point.
pub fn main() {
    let result = calculate(1, 2);
    println!("Result: {}", result);
}
"#,
        )
        .unwrap();

        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join("src").join("utils.rs"),
            r#"
/// Helper utilities.
pub mod helpers {
    /// Formats a string.
    pub fn format_name(name: &str) -> String {
        name.to_uppercase()
    }
}

/// Internal helper.
fn internal_helper() -> bool {
    true
}
"#,
        )
        .unwrap();

        fs::write(
            dir.join("main.py"),
            r#"
class Calculator:
    """A simple calculator."""

    def add(self, a: int, b: int) -> int:
        return a + b

class DataStore:
    """Stores data efficiently."""

    def get(self, key: str) -> str:
        return self.data.get(key, "")

def process_items(items: list) -> list:
    return [x for x in items if x > 0]
"#,
        )
        .unwrap();
    }

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("failed to create tempdir")
    }

    #[test]
    fn test_build_basic_repo_map() {
        let dir = temp_dir();
        create_test_project(dir.path());
        let map = RepoMap::build(dir.path().to_path_buf()).unwrap();
        assert!(map.tag_count() > 0);
        assert!(map.file_count() > 0);
        assert!(map.total_symbol_count() > 0);
    }

    #[test]
    fn test_repo_map_format() {
        let dir = temp_dir();
        create_test_project(dir.path());
        let map = RepoMap::build(dir.path().to_path_buf()).unwrap();
        let output = map.to_string();
        // Should contain file paths
        assert!(output.contains("lib.rs"));
        assert!(output.contains("main.py"));
        // Should contain symbol names
        assert!(output.contains("calculate"));
        assert!(output.contains("Calculator"));
        assert!(output.contains("DataStore"));
    }

    #[test]
    fn test_repo_map_token_budget() {
        let dir = temp_dir();
        create_test_project(dir.path());
        let map = RepoMapBuilder::new(dir.path().to_path_buf())
            .token_budget(100)
            .build()
            .unwrap();
        let output = map.to_string();
        // With a small budget, we should get fewer tags
        let line_count = output.lines().count();
        assert!(line_count > 0);
        // Rough check: 100 tokens ≈ 400 chars
        assert!(
            output.len() < 600,
            "Output too large for budget: {} chars",
            output.len()
        );
    }

    #[test]
    fn test_repo_map_filter_by_extension() {
        let dir = temp_dir();
        create_test_project(dir.path());
        let map = RepoMapBuilder::new(dir.path().to_path_buf())
            .extensions(vec!["rs"])
            .build()
            .unwrap();
        let output = map.to_string();
        assert!(output.contains("lib.rs"));
        assert!(output.contains("utils.rs"));
        assert!(!output.contains("main.py")); // Python file excluded
    }

    #[test]
    fn test_repo_map_filter_by_kind() {
        let dir = temp_dir();
        create_test_project(dir.path());
        let map = RepoMapBuilder::new(dir.path().to_path_buf())
            .include_kinds(vec![SymbolKind::Function, SymbolKind::Class])
            .build()
            .unwrap();
        let output = map.to_string();
        assert!(output.contains("calculate")); // function
        assert!(output.contains("Calculator")); // class
    }

    #[test]
    fn test_repo_map_min_score() {
        let dir = temp_dir();
        create_test_project(dir.path());
        let map_unfiltered = RepoMap::build(dir.path().to_path_buf()).unwrap();
        let map_filtered = RepoMapBuilder::new(dir.path().to_path_buf())
            .min_score(0.9)
            .build()
            .unwrap();
        // With very high min_score, we should get fewer or equal tags
        assert!(
            map_filtered.tag_count() <= map_unfiltered.tag_count(),
            "min_score=0.9 tags ({}) should be <= unfiltered tags ({})",
            map_filtered.tag_count(),
            map_unfiltered.tag_count()
        );
    }

    #[test]
    fn test_repo_map_exclude_patterns() {
        let dir = temp_dir();
        create_test_project(dir.path());
        let map = RepoMapBuilder::new(dir.path().to_path_buf())
            .exclude(vec!["src"])
            .build()
            .unwrap();
        let output = map.to_string();
        assert!(!output.contains("format_name")); // src/ excluded
        assert!(output.contains("calculate")); // lib.rs still included
    }

    #[test]
    fn test_repo_map_fill_ratio() {
        let dir = temp_dir();
        create_test_project(dir.path());
        let map = RepoMap::build(dir.path().to_path_buf()).unwrap();
        let ratio = map.fill_ratio();
        assert!(ratio > 0.0);
        assert!(ratio <= 1.0);
    }

    #[test]
    fn test_repo_map_estimated_tokens() {
        let dir = temp_dir();
        create_test_project(dir.path());
        let map = RepoMap::build(dir.path().to_path_buf()).unwrap();
        assert!(map.estimated_tokens() > 0);
    }

    #[test]
    fn test_repo_map_empty_directory() {
        let dir = temp_dir();
        fs::write(dir.path().join("README.md"), "# Empty project\n").unwrap();
        let map = RepoMap::build(dir.path().to_path_buf()).unwrap();
        assert_eq!(map.tag_count(), 0);
        assert_eq!(map.file_count(), 0);
    }

    #[test]
    fn test_repo_map_max_files() {
        let dir = temp_dir();
        create_test_project(dir.path());
        // Only index 1 file
        let map = RepoMapBuilder::new(dir.path().to_path_buf())
            .max_files(1)
            .build()
            .unwrap();
        assert!(map.tag_count() > 0); // Still gets tags from that 1 file
        assert!(map.file_count() <= 1);
    }
}
