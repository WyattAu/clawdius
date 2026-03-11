//! @Mentions parsing and resolution
//!
//! Parse @mentions from text and resolve them to context content.
//!
//! # Supported Mention Types
//!
//! - `@file:path` - Include file contents
//! - `@folder:path` - List folder contents
//! - `@url:https://...` - Fetch and include URL content
//! - `@problems[:severity]` - Include workspace diagnostics
//! - `@git:diff` or `@git:staged` - Include git diff
//! - `@git:log:N` - Include last N commits
//! - `@symbol:name` - Include symbol definition
//! - `@search:"query"` or `@search:query` - Search codebase
//!
//! # Examples
//!
//! ```rust
//! use clawdius_core::Mention;
//!
//! let text = "Compare @file:src/a.rs with @file:src/b.rs and check @url:https://example.com";
//! let mentions = Mention::parse(text);
//! assert_eq!(mentions.len(), 3);
//! ```

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::{CommitInfo, ContextItem, SearchResult};
use crate::error::{Error, Result};

/// A parsed mention from text
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Mention {
    /// @file:path - Add file contents
    File {
        /// File path
        path: PathBuf,
    },

    /// @folder:path - Add all files in folder
    Folder {
        /// Folder path
        path: PathBuf,
        /// Include files recursively
        recursive: bool,
    },

    /// @url:https://... - Fetch and convert to markdown
    Url {
        /// URL to fetch
        url: String,
    },

    /// @problems - Add workspace diagnostics
    Problems {
        /// Filter by severity
        severity: Option<String>,
    },

    /// @git:diff - Add current git diff
    GitDiff {
        /// Staged or unstaged
        staged: bool,
    },

    /// @git:log - Add recent commits
    GitLog {
        /// Number of commits
        count: usize,
    },

    /// @symbol:name - Add symbol definition
    Symbol {
        /// Symbol name
        name: String,
    },

    /// @search:query - Semantic search
    Search {
        /// Search query
        query: String,
        /// Max results
        limit: usize,
    },
}

impl Mention {
    /// Parse @mentions from text
    pub fn parse(text: &str) -> Vec<(usize, usize, Mention)> {
        let mut mentions = Vec::new();

        // Pattern: @type:value or @type:"value with spaces"
        let patterns = [
            // @file:path
            (r"@file:([^\s]+)", MentionType::File),
            // @folder:path
            (r"@folder:([^\s]+)", MentionType::Folder),
            // @url:url
            (r"@url:(https?://[^\s]+)", MentionType::Url),
            // @problems
            (r"@problems(?::(\w+))?", MentionType::Problems),
            // @git:diff or @git:staged
            (r"@git:(diff|staged|log)(?::(\d+))?", MentionType::Git),
            // @symbol:name
            (r"@symbol:([^\s]+)", MentionType::Symbol),
            // @search:"query" or @search:query
            (r#"@search:"([^"]+)""#, MentionType::SearchQuoted),
            (r#"@search:(?!")([^\s]+)"#, MentionType::Search),
        ];

        for (pattern, mention_type) in patterns {
            if let Ok(re) = Regex::new(pattern) {
                for cap in re.captures_iter(text) {
                    let full_match = cap.get(0).unwrap();
                    let start = full_match.start();
                    let end = full_match.end();

                    if let Some(mention) = Self::from_capture(&mention_type, &cap) {
                        mentions.push((start, end, mention));
                    }
                }
            }
        }

        // Sort by position
        mentions.sort_by_key(|(start, _, _)| *start);

        mentions
    }

    fn from_capture(mention_type: &MentionType, cap: &regex::Captures<'_>) -> Option<Self> {
        match mention_type {
            MentionType::File => {
                let path = cap.get(1)?.as_str();
                Some(Self::File {
                    path: PathBuf::from(path),
                })
            }
            MentionType::Folder => {
                let path = cap.get(1)?.as_str();
                Some(Self::Folder {
                    path: PathBuf::from(path),
                    recursive: false,
                })
            }
            MentionType::Url => {
                let url = cap.get(1)?.as_str();
                Some(Self::Url {
                    url: url.to_string(),
                })
            }
            MentionType::Problems => {
                let severity = cap.get(1).map(|m| m.as_str().to_string());
                Some(Self::Problems { severity })
            }
            MentionType::Git => {
                let git_type = cap.get(1)?.as_str();
                match git_type {
                    "diff" => Some(Self::GitDiff { staged: false }),
                    "staged" => Some(Self::GitDiff { staged: true }),
                    "log" => {
                        let count = cap
                            .get(2)
                            .and_then(|m| m.as_str().parse().ok())
                            .unwrap_or(10);
                        Some(Self::GitLog { count })
                    }
                    _ => None,
                }
            }
            MentionType::Symbol => {
                let name = cap.get(1)?.as_str();
                Some(Self::Symbol {
                    name: name.to_string(),
                })
            }
            MentionType::Search => {
                let query = cap.get(1)?.as_str();
                Some(Self::Search {
                    query: query.to_string(),
                    limit: 10,
                })
            }
            MentionType::SearchQuoted => {
                let query = cap.get(1)?.as_str();
                Some(Self::Search {
                    query: query.to_string(),
                    limit: 10,
                })
            }
        }
    }
}

#[derive(Debug)]
enum MentionType {
    File,
    Folder,
    Url,
    Problems,
    Git,
    Symbol,
    Search,
    SearchQuoted,
}

/// Mention parser
pub struct MentionParser {
    working_dir: PathBuf,
}

impl MentionParser {
    /// Create a new mention parser
    pub fn new(working_dir: impl Into<PathBuf>) -> Self {
        Self {
            working_dir: working_dir.into(),
        }
    }

    /// Parse mentions from text
    pub fn parse(&self, text: &str) -> Vec<Mention> {
        Mention::parse(text)
            .into_iter()
            .map(|(_, _, mention)| mention)
            .collect()
    }

    /// Get the working directory
    pub fn working_dir(&self) -> &Path {
        &self.working_dir
    }
}

/// Mention resolver - resolves mentions to context items
pub struct MentionResolver {
    parser: MentionParser,
}

impl MentionResolver {
    /// Create a new mention resolver
    pub fn new(working_dir: impl Into<PathBuf>) -> Self {
        Self {
            parser: MentionParser::new(working_dir),
        }
    }

    /// Parse and resolve all mentions in text
    pub async fn resolve_all(&self, text: &str) -> Result<Vec<ContextItem>> {
        let mentions = self.parser.parse(text);
        let mut items = Vec::new();

        for mention in mentions {
            match self.resolve(&mention).await {
                Ok(item) => items.push(item),
                Err(e) => {
                    tracing::warn!("Failed to resolve mention {:?}: {}", mention, e);
                }
            }
        }

        Ok(items)
    }

    /// Resolve a single mention
    pub async fn resolve(&self, mention: &Mention) -> Result<ContextItem> {
        match mention {
            Mention::File { path } => self.resolve_file(path).await,
            Mention::Folder { path, recursive } => self.resolve_folder(path, *recursive).await,
            Mention::Url { url } => self.resolve_url(url).await,
            Mention::Problems { severity } => self.resolve_problems(severity.as_deref()).await,
            Mention::GitDiff { staged } => self.resolve_git_diff(*staged).await,
            Mention::GitLog { count } => self.resolve_git_log(*count).await,
            Mention::Symbol { name } => self.resolve_symbol(name).await,
            Mention::Search { query, limit } => self.resolve_search(query, *limit).await,
        }
    }

    async fn resolve_file(&self, path: &Path) -> Result<ContextItem> {
        let full_path = self.parser.working_dir.join(path);

        let content = tokio::fs::read_to_string(&full_path)
            .await
            .map_err(|e| Error::NotFound(format!("File {:?}: {}", path, e)))?;

        let language = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|s| s.to_string());

        Ok(ContextItem::File {
            path: path.display().to_string(),
            content,
            language,
        })
    }

    async fn resolve_folder(&self, path: &Path, _recursive: bool) -> Result<ContextItem> {
        let full_path = self.parser.working_dir.join(path);

        let mut entries = tokio::fs::read_dir(&full_path)
            .await
            .map_err(|e| Error::NotFound(format!("Folder {:?}: {}", path, e)))?;

        let mut files = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            if let Ok(name) = entry.file_name().into_string() {
                if !name.starts_with('.') {
                    files.push(name);
                }
            }
        }

        files.sort();

        Ok(ContextItem::Folder {
            path: path.display().to_string(),
            files,
        })
    }

    async fn resolve_url(&self, url: &str) -> Result<ContextItem> {
        // Fetch URL and convert to markdown
        let response = reqwest::get(url)
            .await
            .map_err(|e| Error::Context(format!("Failed to fetch URL: {}", e)))?;

        let html = response
            .text()
            .await
            .map_err(|e| Error::Context(format!("Failed to read response: {}", e)))?;

        // Simple HTML to markdown conversion
        // In production, use a proper HTML-to-markdown library
        let content = html
            .replace("<br>", "\n")
            .replace("<br/>", "\n")
            .replace("</p>", "\n\n")
            .replace("</h1>", "\n\n")
            .replace("</h2>", "\n\n")
            .replace("</h3>", "\n\n");

        // Strip HTML tags (very basic)
        let re = regex::Regex::new(r"<[^>]+>").unwrap();
        let content = re.replace_all(&content, "").to_string();

        Ok(ContextItem::Url {
            url: url.to_string(),
            content: content.trim().to_string(),
            title: None,
        })
    }

    async fn resolve_problems(&self, _severity: Option<&str>) -> Result<ContextItem> {
        // In production, integrate with LSP or language servers
        // For now, return empty
        Ok(ContextItem::Problems {
            diagnostics: vec![],
        })
    }

    async fn resolve_git_diff(&self, staged: bool) -> Result<ContextItem> {
        let args: Vec<&str> = if staged {
            vec!["diff", "--cached"]
        } else {
            vec!["diff"]
        };
        let output = tokio::process::Command::new("git")
            .args(&args)
            .current_dir(&self.parser.working_dir)
            .output()
            .await
            .map_err(|e| Error::Context(format!("Git diff failed: {}", e)))?;

        let diff = String::from_utf8_lossy(&output.stdout).to_string();

        Ok(ContextItem::GitDiff { diff, staged })
    }

    async fn resolve_git_log(&self, count: usize) -> Result<ContextItem> {
        let output = tokio::process::Command::new("git")
            .args([
                "log",
                &format!("-{}", count),
                "--pretty=format:%H|%an|%s|%ci",
            ])
            .current_dir(&self.parser.working_dir)
            .output()
            .await
            .map_err(|e| Error::Context(format!("Git log failed: {}", e)))?;

        let log = String::from_utf8_lossy(&output.stdout);
        let commits: Vec<CommitInfo> = log
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.splitn(4, '|').collect();
                if parts.len() == 4 {
                    Some(CommitInfo {
                        hash: parts[0].to_string(),
                        author: parts[1].to_string(),
                        message: parts[2].to_string(),
                        timestamp: parts[3].to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(ContextItem::GitLog { commits })
    }

    async fn resolve_symbol(&self, _name: &str) -> Result<ContextItem> {
        // In production, integrate with tree-sitter or LSP
        // For now, return placeholder
        Ok(ContextItem::Symbol {
            name: _name.to_string(),
            location: "unknown".to_string(),
            content: String::new(),
        })
    }

    async fn resolve_search(&self, query: &str, _limit: usize) -> Result<ContextItem> {
        // In production, use semantic search with LanceDB
        // For now, use ripgrep
        let output = tokio::process::Command::new("rg")
            .args(["-l", query])
            .current_dir(&self.parser.working_dir)
            .output()
            .await;

        let results = match output {
            Ok(out) => {
                let files = String::from_utf8_lossy(&out.stdout);
                files
                    .lines()
                    .take(_limit)
                    .map(|file| SearchResult {
                        file: file.to_string(),
                        line: 0,
                        content: String::new(),
                        score: 1.0,
                    })
                    .collect()
            }
            Err(_) => vec![],
        };

        Ok(ContextItem::Search {
            query: query.to_string(),
            results,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_file_mention() {
        let text = "Fix the bug in @file:src/main.rs";
        let mentions = Mention::parse(text);

        assert_eq!(mentions.len(), 1);
        assert!(matches!(mentions[0].2, Mention::File { .. }));
    }

    mod integration {
        use super::*;
        use std::path::PathBuf;
        use tokio::fs;

        async fn create_test_file(path: &std::path::Path, content: &str) -> Result<()> {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).await?;
            }
            fs::write(path, content).await?;
            Ok(())
        }

        #[tokio::test]
        async fn test_resolve_file() {
            let temp_dir = tempfile::tempdir().unwrap();
            let test_file = temp_dir.path().join("test.rs");
            create_test_file(&test_file, "fn main() {}").await.unwrap();

            let resolver = MentionResolver::new(temp_dir.path());
            let mention = Mention::File {
                path: PathBuf::from("test.rs"),
            };

            let result = resolver.resolve(&mention).await.unwrap();

            if let ContextItem::File { path, language, .. } = result {
                assert_eq!(path, "test.rs");
                assert_eq!(language, Some("rs".to_string()));
            } else {
                panic!("Expected File");
            }
        }

        #[tokio::test]
        async fn test_resolve_folder() {
            let temp_dir = tempfile::tempdir().unwrap();
            let subdir = temp_dir.path().join("src");
            fs::create_dir_all(&subdir).await.unwrap();
            fs::write(subdir.join("a.rs"), "").await.unwrap();
            fs::write(subdir.join("b.rs"), "").await.unwrap();

            let resolver = MentionResolver::new(temp_dir.path());
            let mention = Mention::Folder {
                path: PathBuf::from("src"),
                recursive: false,
            };

            let result = resolver.resolve(&mention).await.unwrap();

            if let ContextItem::Folder { files, .. } = result {
                assert!(files.contains(&"a.rs".to_string()));
                assert!(files.contains(&"b.rs".to_string()));
            } else {
                panic!("Expected Folder");
            }
        }

        #[tokio::test]
        async fn test_resolve_all() {
            let temp_dir = tempfile::tempdir().unwrap();

            fs::write(temp_dir.path().join("a.rs"), "fn a() {}")
                .await
                .unwrap();
            fs::write(temp_dir.path().join("b.rs"), "fn b() {}")
                .await
                .unwrap();

            let resolver = MentionResolver::new(temp_dir.path());
            let text = "Compare @file:a.rs with @file:b.rs";
            let items = resolver.resolve_all(text).await.unwrap();

            assert_eq!(items.len(), 2);
        }
    }

    #[test]
    fn test_parse_multiple_mentions() {
        let text = "Compare @file:src/a.rs with @file:src/b.rs and check @url:https://example.com";
        let mentions = Mention::parse(text);

        assert_eq!(mentions.len(), 3);
    }

    #[test]
    fn test_parse_git_mentions() {
        let text = "Review @git:diff and @git:log:5";
        let mentions = Mention::parse(text);

        assert_eq!(mentions.len(), 2);

        if let Mention::GitDiff { staged } = &mentions[0].2 {
            assert!(!staged);
        } else {
            panic!("Expected GitDiff");
        }

        if let Mention::GitLog { count } = &mentions[1].2 {
            assert_eq!(*count, 5);
        } else {
            panic!("Expected GitLog");
        }
    }

    #[test]
    fn test_parse_search_quoted() {
        let text = r#"Search for @search:"function definition""#;
        let mentions = Mention::parse(text);

        assert_eq!(mentions.len(), 1);
        if let Mention::Search { query, .. } = &mentions[0].2 {
            assert_eq!(query, "function definition");
        }
    }

    #[test]
    fn test_parse_folder_mention() {
        let text = "Check @folder:src/components";
        let mentions = Mention::parse(text);

        assert_eq!(mentions.len(), 1);
        if let Mention::Folder { path, recursive } = &mentions[0].2 {
            assert_eq!(path.to_str(), Some("src/components"));
            assert!(!recursive);
        } else {
            panic!("Expected Folder");
        }
    }

    #[test]
    fn test_parse_problems_mention() {
        let text = "Fix @problems:error";
        let mentions = Mention::parse(text);

        assert_eq!(mentions.len(), 1);
        if let Mention::Problems { severity } = &mentions[0].2 {
            assert_eq!(severity, &Some("error".to_string()));
        } else {
            panic!("Expected Problems");
        }
    }

    #[test]
    fn test_parse_symbol_mention() {
        let text = "Explain @symbol:parse_function";
        let mentions = Mention::parse(text);

        assert_eq!(mentions.len(), 1);
        if let Mention::Symbol { name } = &mentions[0].2 {
            assert_eq!(name, "parse_function");
        } else {
            panic!("Expected Symbol");
        }
    }

    #[test]
    fn test_parse_url_mention() {
        let text = "Summarize @url:https://example.com/doc";
        let mentions = Mention::parse(text);

        assert_eq!(mentions.len(), 1);
        if let Mention::Url { url } = &mentions[0].2 {
            assert_eq!(url, "https://example.com/doc");
        } else {
            panic!("Expected Url");
        }
    }

    #[test]
    fn test_mention_positions() {
        let text = "Start @file:a.rs middle @file:b.rs end";
        let mentions = Mention::parse(text);

        assert_eq!(mentions.len(), 2);
        assert_eq!(mentions[0].0, 6); // Start position of first mention
        assert_eq!(mentions[1].0, 24); // Start position of second mention
    }

    #[test]
    fn test_no_mentions() {
        let text = "This is a regular message without mentions";
        let mentions = Mention::parse(text);

        assert_eq!(mentions.len(), 0);
    }
}
