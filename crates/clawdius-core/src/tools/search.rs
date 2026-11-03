//! Repo search tools: content grep and filename glob.
//!
//! The agent's window into a repository was read/write/edit/list only;
//! without content search the model cannot find definitions, callers, or
//! TODOs — the operations it performs most. `SearchTool::grep` walks with
//! the `ignore` crate (respects .gitignore, skips hidden/binary) and
//! matches with `regex`; `SearchTool::glob` lists paths by pattern.

use std::path::{Path, PathBuf};

use regex::Regex;

/// Maximum matches returned by one grep call.
pub const GREP_MAX_RESULTS: usize = 200;
/// Maximum bytes of a file body considered (skip binaries/huge files).
const MAX_FILE_BYTES: u64 = 512 * 1024;

/// One grep match, rendered as `path:line:text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrepMatch {
    /// File the match lives in.
    pub path: String,
    /// 1-based line number.
    pub line: usize,
    /// The matched line, trimmed.
    pub text: String,
}

/// Repo content search.
#[derive(Debug, Clone, Default)]
pub struct SearchTool;

impl SearchTool {
    /// Create a search tool.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Regex-search file contents under `root` (respects .gitignore).
    ///
    /// `include` is an optional glob applied to the file name (e.g.
    /// `*.rs`). Returns at most [`GREP_MAX_RESULTS`] matches; a bad regex
    /// is a caller error and returns [`crate::error::Error::Config`].
    ///
    /// # Errors
    ///
    /// Returns an error when `pattern` is not a valid regex.
    pub fn grep(
        &self,
        pattern: &str,
        root: &Path,
        include: Option<&str>,
        max_results: usize,
    ) -> crate::Result<Vec<GrepMatch>> {
        let re = Regex::new(pattern).map_err(|e| {
            crate::error::Error::Config(format!("invalid grep pattern '{pattern}': {e}"))
        })?;
        let include_matcher = include.map(glob_matcher);

        let mut matches = Vec::new();
        let cap = max_results.min(GREP_MAX_RESULTS);
        let walker = ignore::WalkBuilder::new(root)
            .hidden(true)
            .require_git(false)
            .build();
        for entry in walker.flatten() {
            if matches.len() >= cap {
                break;
            }
            let path = entry.path();
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            if let Some(matcher) = &include_matcher {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !matcher(name) {
                    continue;
                }
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.len() > MAX_FILE_BYTES {
                continue;
            }
            let Ok(body) = std::fs::read_to_string(path) else {
                continue; // binary or non-UTF8: skipped
            };
            let display = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
            for (idx, line) in body.lines().enumerate() {
                if re.is_match(line) {
                    matches.push(GrepMatch {
                        path: display.to_string(),
                        line: idx + 1,
                        text: line.trim().chars().take(300).collect(),
                    });
                    if matches.len() >= cap {
                        break;
                    }
                }
            }
        }
        Ok(matches)
    }

    /// List files under `root` whose relative path matches a glob pattern
    /// (`*`, `**`, `?`; e.g. `**/*.rs` or `crates/*/Cargo.toml`).
    ///
    /// # Errors
    ///
    /// Returns an error when walking the directory fails.
    pub fn glob(&self, pattern: &str, root: &Path) -> crate::Result<Vec<String>> {
        let matcher = glob_matcher(pattern);
        let mut out = Vec::new();
        let walker = ignore::WalkBuilder::new(root)
            .hidden(true)
            .require_git(false)
            .build();
        for entry in walker.flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap_or(path);
            if rel.as_os_str().is_empty() {
                continue;
            }
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if matcher(&rel_str) {
                out.push(rel_str);
            }
        }
        out.sort();
        Ok(out)
    }
}

/// Build a matcher closure for a simple glob (`*`, `?`, `**` path segments).
fn glob_matcher(pattern: &str) -> impl Fn(&str) -> bool + Send + Sync + 'static {
    let pattern = pattern.to_string();
    move |candidate: &str| glob_match(&pattern, candidate)
}

/// Minimal glob over relative paths: `**` crosses separators (and matches
/// zero segments), `*` and `?` stay within one segment.
fn glob_match(pattern: &str, candidate: &str) -> bool {
    let pat_segs: Vec<&str> = pattern.split('/').collect();
    let cand_segs: Vec<&str> = candidate.split('/').collect();
    pmatch(&pat_segs, &cand_segs)
}

fn pmatch(p: &[&str], c: &[&str]) -> bool {
    match p.first() {
        None => c.is_empty(),
        Some(&"**") => (0..=c.len()).any(|skip| pmatch(&p[1..], &c[skip..])),
        Some(pat) => {
            if c.is_empty() {
                return false;
            }
            wildcard_match(pat, c[0]) && pmatch(&p[1..], &c[1..])
        },
    }
}

/// Match a pattern (single segment, `*`/`?` aware) against a path when the
/// pattern carries no separator, else segment-wise.
#[cfg(test)]
fn segment_match(pattern: &str, candidate: &str) -> bool {
    if !pattern.contains('/') {
        let file = final_segment(candidate);
        return wildcard_match(pattern, file);
    }
    glob_match(pattern, candidate)
}

#[cfg(test)]
fn final_segment(candidate: &str) -> &str {
    candidate.rsplit('/').next().unwrap_or(candidate)
}

/// `*` matches within a segment, `?` exactly one char.
fn wildcard_match(pattern: &str, text: &str) -> bool {
    fn inner(p: &[u8], t: &[u8]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some(b'*'), _) => {
                for skip in 0..=t.len() {
                    if inner(&p[1..], &t[skip..]) {
                        return true;
                    }
                }
                false
            },
            (Some(b'?'), Some(_)) => inner(&p[1..], &t[1..]),
            (Some(&c), Some(&d)) if c == d => inner(&p[1..], &t[1..]),
            _ => false,
        }
    }
    inner(pattern.as_bytes(), text.as_bytes())
}

/// Convenience: resolve a user-supplied root that may be relative.
#[must_use]
pub fn resolve_root(root: &str) -> PathBuf {
    let p = Path::new(root);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("src/deep")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "fn alpha() {}\nfn beta() {}\n").unwrap();
        std::fs::write(
            root.join("src/deep/util.rs"),
            "const DEPTH: u8 = 1; // alpha deep\n",
        )
        .unwrap();
        std::fs::write(root.join("README.md"), "# alpha demo\n").unwrap();
        std::fs::write(root.join("src/ignored.log"), "alpha in a log\n").unwrap();
        std::fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        (dir, root)
    }

    #[test]
    fn grep_finds_matches_and_respects_gitignore() {
        let (_d, root) = scratch();
        let tool = SearchTool::new();
        let hits = tool.grep("alpha", &root, None, 50).unwrap();
        let paths: Vec<&str> = hits.iter().map(|m| m.path.as_str()).collect();
        assert!(paths.contains(&"src/lib.rs"));
        assert!(paths.contains(&"src/deep/util.rs"));
        assert!(paths.contains(&"README.md"));
        assert!(
            !paths.iter().any(|p| p.ends_with(".log")),
            "gitignored files must be skipped"
        );
        assert_eq!(
            hits.iter().find(|m| m.path == "src/lib.rs").unwrap().line,
            1
        );
    }

    #[test]
    fn grep_include_filter_and_invalid_regex() {
        let (_d, root) = scratch();
        let tool = SearchTool::new();
        let hits = tool.grep("alpha", &root, Some("*.rs"), 50).unwrap();
        assert!(hits.iter().all(|m| m.path.ends_with(".rs")));
        assert!(tool.grep("([bad", &root, None, 50).is_err());
    }

    #[test]
    fn glob_matches_recursive_pattern() {
        let (_d, root) = scratch();
        let tool = SearchTool::new();
        let rs = tool.glob("**/*.rs", &root).unwrap();
        assert_eq!(rs, vec!["src/deep/util.rs", "src/lib.rs"]);
        let top = tool.glob("*.md", &root).unwrap();
        assert_eq!(top, vec!["README.md"]);
    }

    #[test]
    fn wildcard_semantics() {
        assert!(wildcard_match("*.rs", "lib.rs"));
        // No separator: basename fallback (ripgrep-glob-like).
        assert!(segment_match("*.rs", "lib.rs"));
        assert!(segment_match("*.rs", "src/lib.rs"));
        // Separator pattern: strict segment-wise matching.
        assert!(segment_match("src/*.rs", "src/lib.rs"));
        assert!(!segment_match("src/*.rs", "src/deep/util.rs"));
        // `**` crosses segments and matches zero segments.
        assert!(glob_match("**/*.rs", "src/deep/util.rs"));
        assert!(glob_match("**/lib.rs", "lib.rs"));
        assert!(!glob_match("src/**/*.rs", "README.md"));
    }
}
