use std::path::Path;

use tiktoken_rs::CoreBPE;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodeChunk {
    pub id: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub language: String,
    pub content: String,
    pub token_count: usize,
}

#[must_use] 
pub fn detect_language(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("toml") => "toml",
        Some("md") => "markdown",
        Some("py") => "python",
        Some("js") => "javascript",
        Some("ts") => "typescript",
        Some("tsx") => "typescript",
        Some("json") => "json",
        Some("yaml" | "yml") => "yaml",
        Some("txt") => "text",
        Some("lock") => "lockfile",
        _ => "unknown",
    }
}

#[must_use] 
pub fn is_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("rs" | "toml" | "md" | "py" | "js" | "ts" | "tsx" | "json" | "yaml" |
"yml" | "txt" | "lock")
    )
}

pub struct CodebaseIndexer {
    tokenizer: CoreBPE,
    max_chunk_tokens: usize,
    overlap_tokens: usize,
    ignore_patterns: Vec<String>,
}

impl Default for CodebaseIndexer {
    fn default() -> Self {
        Self {
            tokenizer: tiktoken_rs::cl100k_base().expect("failed to load cl100k tokenizer"),
            max_chunk_tokens: 512,
            overlap_tokens: 50,
            ignore_patterns: vec![
                "target/".to_string(),
                "node_modules/".to_string(),
                ".git/".to_string(),
                "__pycache__/".to_string(),
            ],
        }
    }
}

impl CodebaseIndexer {
    #[must_use] 
    pub fn new() -> Self {
        Self::default()
    }

    pub fn index_directory(&self, root: &Path) -> Result<Vec<CodeChunk>, String> {
        let mut chunks = Vec::new();
        self.index_path(root, root, &mut chunks)?;
        Ok(chunks)
    }

    fn index_path(
        &self,
        root: &Path,
        path: &Path,
        chunks: &mut Vec<CodeChunk>,
    ) -> Result<(), String> {
        if !path.is_dir() {
            return Ok(());
        }

        let relative = path.strip_prefix(root).unwrap_or(path);
        let rel_str = relative.to_string_lossy();
        if self.ignore_patterns.iter().any(|p| {
            let pattern = p.trim_end_matches('/');
            rel_str == pattern
                || rel_str.starts_with(&format!("{pattern}/"))
                || rel_str.starts_with(&format!("{pattern}\\"))
        }) {
            return Ok(());
        }

        let entries = match std::fs::read_dir(path) {
            Ok(e) => e,
            Err(e) => {
                return Err(format!(
                    "Failed to read directory {}: {}",
                    path.display(),
                    e
                ))
            },
        };

        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                self.index_path(root, &entry_path, chunks)?;
            } else if is_source_file(&entry_path) {
                if let Err(e) = self.index_file(root, &entry_path, chunks) {
                    eprintln!("Warning: Failed to index {}: {}", entry_path.display(), e);
                }
            }
        }

        Ok(())
    }

    fn index_file(
        &self,
        root: &Path,
        path: &Path,
        chunks: &mut Vec<CodeChunk>,
    ) -> Result<(), String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

        if content.trim().is_empty() {
            return Ok(());
        }

        let relative = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let language = detect_language(path);
        let lines: Vec<&str> = content.lines().collect();

        let mut current_chunk_lines: Vec<&str> = Vec::new();
        let mut current_chunk_tokens: usize = 0;
        let mut current_start: usize = 0;
        let mut chunk_counter: usize = 0;

        for (i, line) in lines.iter().enumerate() {
            let line_tokens = self.tokenizer.encode_with_special_tokens(line).len();

            if current_chunk_tokens + line_tokens > self.max_chunk_tokens
                && !current_chunk_lines.is_empty()
            {
                let chunk_text = current_chunk_lines.join("\n");
                let chunk_tokens = self.tokenizer.encode_with_special_tokens(&chunk_text).len();

                chunks.push(CodeChunk {
                    id: format!(
                        "{}_{}",
                        relative.replace(['/', '.'], "_"),
                        chunk_counter
                    ),
                    file_path: relative.clone(),
                    start_line: current_start,
                    end_line: i,
                    language: language.to_string(),
                    content: chunk_text,
                    token_count: chunk_tokens,
                });

                current_chunk_lines.clear();
                current_chunk_tokens = 0;
                current_start = i.saturating_sub((self.overlap_tokens / 4).max(1));
                chunk_counter += 1;

                for overlap_line in &lines[current_start..=i] {
                    let overlap_tokens = self
                        .tokenizer
                        .encode_with_special_tokens(overlap_line)
                        .len();
                    current_chunk_tokens += overlap_tokens;
                    current_chunk_lines.push(overlap_line);
                }
            } else {
                current_chunk_tokens += line_tokens;
                current_chunk_lines.push(line);
            }
        }

        if !current_chunk_lines.is_empty() {
            let chunk_text = current_chunk_lines.join("\n");
            let chunk_tokens = self.tokenizer.encode_with_special_tokens(&chunk_text).len();

            chunks.push(CodeChunk {
                id: format!(
                    "{}_{}",
                    relative.replace(['/', '.'], "_"),
                    chunk_counter
                ),
                file_path: relative,
                start_line: current_start,
                end_line: lines.len().saturating_sub(1),
                language: language.to_string(),
                content: chunk_text,
                token_count: chunk_tokens,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language(Path::new("foo.rs")), "rust");
        assert_eq!(detect_language(Path::new("bar.toml")), "toml");
        assert_eq!(detect_language(Path::new("readme.md")), "markdown");
        assert_eq!(detect_language(Path::new("main.py")), "python");
        assert_eq!(detect_language(Path::new("app.js")), "javascript");
        assert_eq!(detect_language(Path::new("index.ts")), "typescript");
        assert_eq!(detect_language(Path::new("comp.tsx")), "typescript");
        assert_eq!(detect_language(Path::new("data.json")), "json");
        assert_eq!(detect_language(Path::new("cfg.yaml")), "yaml");
        assert_eq!(detect_language(Path::new("cfg.yml")), "yaml");
        assert_eq!(detect_language(Path::new("notes.txt")), "text");
        assert_eq!(detect_language(Path::new("Cargo.lock")), "lockfile");
        assert_eq!(detect_language(Path::new("noext")), "unknown");
    }

    #[test]
    fn test_is_source_file_includes() {
        assert!(is_source_file(Path::new("main.rs")));
        assert!(is_source_file(Path::new("Cargo.toml")));
        assert!(is_source_file(Path::new("README.md")));
        assert!(is_source_file(Path::new("app.py")));
        assert!(is_source_file(Path::new("lib.js")));
        assert!(is_source_file(Path::new("index.ts")));
        assert!(is_source_file(Path::new("app.tsx")));
        assert!(is_source_file(Path::new("data.json")));
        assert!(is_source_file(Path::new("cfg.yaml")));
        assert!(is_source_file(Path::new("cfg.yml")));
        assert!(is_source_file(Path::new("notes.txt")));
        assert!(is_source_file(Path::new("Cargo.lock")));
    }

    #[test]
    fn test_is_source_file_excludes() {
        assert!(!is_source_file(Path::new("image.png")));
        assert!(!is_source_file(Path::new("archive.zip")));
        assert!(!is_source_file(Path::new("binary.exe")));
        assert!(!is_source_file(Path::new("style.css")));
        assert!(!is_source_file(Path::new("noext")));
    }

    #[test]
    fn test_index_file_basic() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

        let indexer = CodebaseIndexer::new();
        let mut chunks = Vec::new();
        indexer
            .index_file(dir.path(), &file_path, &mut chunks)
            .unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].language, "rust");
        assert_eq!(chunks[0].file_path, "test.rs");
        assert!(chunks[0].content.contains("fn main"));
        assert!(chunks[0].token_count > 0);
    }

    #[test]
    fn test_index_file_empty() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("empty.rs");
        std::fs::write(&file_path, "").unwrap();

        let indexer = CodebaseIndexer::new();
        let mut chunks = Vec::new();
        indexer
            .index_file(dir.path(), &file_path, &mut chunks)
            .unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_index_file_whitespace_only() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("whitespace.rs");
        std::fs::write(&file_path, "   \n  \n  \n").unwrap();

        let indexer = CodebaseIndexer::new();
        let mut chunks = Vec::new();
        indexer
            .index_file(dir.path(), &file_path, &mut chunks)
            .unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_index_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
        std::fs::write(dir.path().join("b.py"), "def b(): pass\n").unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub/c.toml"), "[package]\n").unwrap();

        let indexer = CodebaseIndexer::new();
        let chunks = indexer.index_directory(dir.path()).unwrap();

        let file_paths: std::collections::HashSet<_> =
            chunks.iter().map(|c| c.file_path.clone()).collect();
        assert!(file_paths.contains("a.rs"));
        assert!(file_paths.contains("b.py"));
        assert!(file_paths.contains("sub/c.toml"));
    }

    #[test]
    fn test_index_directory_ignores_patterns() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("target")).unwrap();
        std::fs::write(dir.path().join("target/compiled.rs"), "ignored\n").unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

        let indexer = CodebaseIndexer::new();
        let chunks = indexer.index_directory(dir.path()).unwrap();

        let file_paths: Vec<_> = chunks.iter().map(|c| c.file_path.as_str()).collect();
        assert!(file_paths.contains(&"main.rs"));
        assert!(!file_paths.contains(&"target/compiled.rs"));
    }

    #[test]
    fn test_chunking_large_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("large.rs");
        let lines: Vec<String> = (0..200)
            .map(|i| format!("fn function_{}() {{ let x = {}; }}", i, i))
            .collect();
        std::fs::write(&file_path, lines.join("\n")).unwrap();

        let indexer = CodebaseIndexer::new();
        let mut chunks = Vec::new();
        indexer
            .index_file(dir.path(), &file_path, &mut chunks)
            .unwrap();

        assert!(
            chunks.len() > 1,
            "large file should produce multiple chunks"
        );
        for chunk in &chunks {
            assert!(chunk.token_count > 0);
            assert_eq!(chunk.language, "rust");
        }
    }
}
