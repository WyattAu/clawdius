//! Diff preview system for displaying file changes

use super::{DiffLine, FileDiff};
use std::path::PathBuf;

/// Statistics about a diff
#[derive(Debug, Clone, Default)]
pub struct DiffStats {
    /// Number of lines added
    pub additions: usize,
    /// Number of lines deleted
    pub deletions: usize,
    /// Number of files changed
    pub files_changed: usize,
}

impl DiffStats {
    /// Merge stats from another instance
    pub fn merge(&mut self, other: &DiffStats) {
        self.additions += other.additions;
        self.deletions += other.deletions;
        self.files_changed += other.files_changed;
    }
}

/// Represents a file change
#[derive(Debug, Clone)]
pub struct FileChange {
    /// Path to the file
    pub path: PathBuf,
    /// Old content (None for new files)
    pub old_content: Option<String>,
    /// New content
    pub new_content: String,
}

/// Preview of multiple file diffs
#[derive(Debug, Clone)]
pub struct DiffPreview {
    /// Individual file diffs
    pub diffs: Vec<FileDiff>,
    /// Summary text
    pub summary: String,
    /// Aggregate statistics
    pub stats: DiffStats,
}

impl DiffPreview {
    /// Create a preview from file changes
    #[must_use]
    pub fn from_changes(changes: &[FileChange]) -> Self {
        let diffs: Vec<FileDiff> = changes
            .iter()
            .map(|change| {
                FileDiff::compute(
                    change.path.clone(),
                    change.old_content.as_deref(),
                    &change.new_content,
                )
            })
            .collect();

        let mut stats = DiffStats::default();
        for diff in &diffs {
            let diff_stats = diff.stats();
            stats.merge(&diff_stats);
        }

        let summary = format!(
            "{} file{} changed, {} insertion{}(+), {} deletion{}(-)",
            stats.files_changed,
            if stats.files_changed == 1 { "" } else { "s" },
            stats.additions,
            if stats.additions == 1 { "" } else { "s" },
            stats.deletions,
            if stats.deletions == 1 { "" } else { "s" }
        );

        DiffPreview {
            diffs,
            summary,
            stats,
        }
    }

    /// Convert to markdown format
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str("# Diff Preview\n\n");
        md.push_str(&format!("**{}**\n\n", self.summary));

        for diff in &self.diffs {
            let path_str = diff.path.to_string_lossy();
            let stats = diff.stats();

            md.push_str(&format!("## `{path_str}`\n\n"));
            md.push_str(&format!(
                "+{} additions, -{} deletions\n\n",
                stats.additions, stats.deletions
            ));

            if !diff.hunks.is_empty() {
                md.push_str("```diff\n");

                for hunk in &diff.hunks {
                    md.push_str(&format!(
                        "@@ -{},{} +{},{} @@\n",
                        hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
                    ));

                    for line in &hunk.lines {
                        let (prefix, content) = match line {
                            DiffLine::Context(l) => (" ", l.as_str()),
                            DiffLine::Added(l) => ("+", l.as_str()),
                            DiffLine::Removed(l) => ("-", l.as_str()),
                        };
                        md.push_str(prefix);
                        md.push_str(content);
                        if !content.ends_with('\n') {
                            md.push('\n');
                        }
                    }
                }

                md.push_str("```\n\n");
            }
        }

        md
    }

    /// Check if there are any changes
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.diffs.is_empty() || self.stats.files_changed == 0
    }

    /// Get list of changed file paths
    #[must_use]
    pub fn changed_files(&self) -> Vec<&PathBuf> {
        self.diffs.iter().map(|d| &d.path).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_changes() {
        let changes = vec![
            FileChange {
                path: PathBuf::from("file1.txt"),
                old_content: Some("old\n".to_string()),
                new_content: "new\n".to_string(),
            },
            FileChange {
                path: PathBuf::from("file2.txt"),
                old_content: None,
                new_content: "new file\n".to_string(),
            },
        ];

        let preview = DiffPreview::from_changes(&changes);

        assert_eq!(preview.diffs.len(), 2);
        assert_eq!(preview.stats.files_changed, 2);
        assert!(preview.summary.contains("2 files"));
    }

    #[test]
    fn test_to_markdown() {
        let changes = vec![FileChange {
            path: PathBuf::from("test.txt"),
            old_content: Some("old\n".to_string()),
            new_content: "new\n".to_string(),
        }];

        let preview = DiffPreview::from_changes(&changes);
        let markdown = preview.to_markdown();

        assert!(markdown.contains("# Diff Preview"));
        assert!(markdown.contains("test.txt"));
        assert!(markdown.contains("```diff"));
    }
}
