//! Diff module for computing and rendering file changes
//!
//! This module provides functionality to:
//! - Compute unified diffs between file versions
//! - Render diffs for terminal display with colors
//! - Render diffs as HTML for webview integration
//! - Generate diff statistics and previews

pub mod preview;
pub mod renderer;

use similar::{ChangeTag, TextDiff};
use std::path::PathBuf;

pub use preview::{DiffPreview, DiffStats, FileChange};
pub use renderer::{DiffRenderer, DiffTheme};

/// Represents a single file's diff
#[derive(Debug, Clone)]
pub struct FileDiff {
    /// Path to the file
    pub path: PathBuf,
    /// Old content (None for new files)
    pub old_content: Option<String>,
    /// New content
    pub new_content: String,
    /// Diff hunks
    pub hunks: Vec<Hunk>,
}

/// A single hunk in a diff
#[derive(Debug, Clone)]
pub struct Hunk {
    /// Starting line in old file
    pub old_start: usize,
    /// Number of lines in old file
    pub old_lines: usize,
    /// Starting line in new file
    pub new_start: usize,
    /// Number of lines in new file
    pub new_lines: usize,
    /// Lines in this hunk
    pub lines: Vec<DiffLine>,
}

/// A single line in a diff
#[derive(Debug, Clone, PartialEq)]
pub enum DiffLine {
    /// Context line (unchanged)
    Context(String),
    /// Added line
    Added(String),
    /// Removed line
    Removed(String),
}

impl FileDiff {
    /// Compute a diff between old and new content
    pub fn compute(path: PathBuf, old: Option<&str>, new: &str) -> Self {
        let old_content = old.map(|s| s.to_string());
        let old_str = old.unwrap_or("");

        let text_diff = TextDiff::from_lines(old_str, new);
        let mut hunks = Vec::new();

        for op in text_diff.ops() {
            let mut hunk_lines = Vec::new();
            let mut old_start = None;
            let mut new_start = None;
            let mut old_count = 0usize;
            let mut new_count = 0usize;

            for change in text_diff.iter_changes(op) {
                let line = change.value().to_string();

                match change.tag() {
                    ChangeTag::Equal => {
                        hunk_lines.push(DiffLine::Context(line));
                    }
                    ChangeTag::Delete => {
                        if old_start.is_none() {
                            old_start = Some(change.old_index().unwrap_or(0) + 1);
                        }
                        old_count += 1;
                        hunk_lines.push(DiffLine::Removed(line));
                    }
                    ChangeTag::Insert => {
                        if new_start.is_none() {
                            new_start = Some(change.new_index().unwrap_or(0) + 1);
                        }
                        new_count += 1;
                        hunk_lines.push(DiffLine::Added(line));
                    }
                }
            }

            if old_count > 0 || new_count > 0 {
                hunks.push(Hunk {
                    old_start: old_start.unwrap_or(1),
                    old_lines: old_count,
                    new_start: new_start.unwrap_or(1),
                    new_lines: new_count,
                    lines: hunk_lines,
                });
            }
        }

        FileDiff {
            path,
            old_content,
            new_content: new.to_string(),
            hunks,
        }
    }

    /// Convert to unified diff format
    pub fn to_unified(&self) -> String {
        let mut output = String::new();

        let path_str = self.path.to_string_lossy();
        output.push_str(&format!("--- {}\n", path_str));
        output.push_str(&format!("+++ {}\n", path_str));

        for hunk in &self.hunks {
            output.push_str(&format!(
                "@@ -{},{} +{},{} @@\n",
                hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
            ));

            for line in &hunk.lines {
                let (prefix, content) = match line {
                    DiffLine::Context(l) => (" ", l.as_str()),
                    DiffLine::Added(l) => ("+", l.as_str()),
                    DiffLine::Removed(l) => ("-", l.as_str()),
                };
                output.push_str(prefix);
                output.push_str(content);
                if !content.ends_with('\n') {
                    output.push('\n');
                }
            }
        }

        output
    }

    /// Get statistics for this diff
    pub fn stats(&self) -> DiffStats {
        let mut additions = 0;
        let mut deletions = 0;

        for hunk in &self.hunks {
            for line in &hunk.lines {
                match line {
                    DiffLine::Added(_) => additions += 1,
                    DiffLine::Removed(_) => deletions += 1,
                    DiffLine::Context(_) => {}
                }
            }
        }

        DiffStats {
            additions,
            deletions,
            files_changed: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_diff() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nmodified\nline3\n";

        let diff = FileDiff::compute(PathBuf::from("test.txt"), Some(old), new);

        assert_eq!(diff.hunks.len(), 1);
        assert!(diff.stats().additions > 0);
        assert!(diff.stats().deletions > 0);
    }

    #[test]
    fn test_new_file() {
        let new = "new content\n";
        let diff = FileDiff::compute(PathBuf::from("new.txt"), None, new);

        assert!(diff.old_content.is_none());
        assert_eq!(diff.stats().deletions, 0);
    }
}
