use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParsedFileChange {
    pub path: String,
    pub content: String,
    pub language: Option<String>,
}

struct CodeBlock {
    language: Option<String>,
    content: String,
    preceding_text: String,
}

pub fn parse_llm_output(response: &str) -> Vec<ParsedFileChange> {
    let blocks = extract_code_blocks(response);
    let mut results = Vec::new();

    for block in &blocks {
        if is_example_block(block) {
            continue;
        }
        if let Some(path) = detect_path(block) {
            results.push(ParsedFileChange {
                path,
                content: strip_content(&block.content),
                language: extract_language(&block.language),
            });
        }
    }

    results
}

pub fn parse_llm_output_with_hint(response: &str, file_path: &str) -> Vec<ParsedFileChange> {
    let mut results = parse_llm_output(response);

    if results.is_empty() {
        let blocks = extract_code_blocks(response);
        if blocks.len() == 1 {
            let block = &blocks[0];
            results.push(ParsedFileChange {
                path: file_path.to_string(),
                content: strip_content(&block.content),
                language: extract_language(&block.language),
            });
        }
    }

    results
}

fn strip_content(content: &str) -> String {
    let mut lines: Vec<&str> = content.lines().collect();

    while lines.first().map_or(false, |l| l.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().map_or(false, |l| l.trim().is_empty()) {
        lines.pop();
    }

    lines.join("\n")
}

fn extract_code_blocks(text: &str) -> Vec<CodeBlock> {
    let mut blocks = Vec::new();
    let mut last_end = 0;
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'`' {
            let fence_len = count_backticks_at(text, i);
            if fence_len >= 3 {
                let line_end = text[i..].find('\n').map(|j| i + j).unwrap_or(len);
                let lang_tag = text[i + fence_len..line_end].trim();

                let search_start = if line_end < len {
                    line_end + 1
                } else {
                    line_end
                };
                if let Some(pos) = find_closing_fence(&text[search_start..], fence_len) {
                    let content_end = search_start + pos;
                    let content = &text[search_start..content_end];
                    let preceding = &text[last_end..i];

                    blocks.push(CodeBlock {
                        language: if lang_tag.is_empty() {
                            None
                        } else {
                            Some(lang_tag.to_string())
                        },
                        content: content.to_string(),
                        preceding_text: preceding.to_string(),
                    });

                    let closing_end = content_end + fence_len;
                    last_end = if closing_end < len && bytes[closing_end] == b'\n' {
                        closing_end + 1
                    } else {
                        closing_end
                    };
                    i = last_end;
                    continue;
                }
            }
        }
        i += 1;
    }

    blocks
}

fn count_backticks_at(text: &str, start: usize) -> usize {
    text[start..].chars().take_while(|&c| c == '`').count()
}

fn find_closing_fence(text: &str, fence_len: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i + fence_len <= len {
        if bytes[i] == b'`' && count_backticks_at(text, i) >= fence_len {
            let preceded_by_newline = i == 0 || bytes[i - 1] == b'\n';
            if preceded_by_newline {
                let after_fence = i + fence_len;
                let line_end = text[after_fence..]
                    .find('\n')
                    .unwrap_or(text.len() - after_fence);
                let rest = text[after_fence..after_fence + line_end].trim();
                if rest.is_empty() {
                    return Some(i);
                }
            }
        }
        i += 1;
    }

    None
}

fn is_example_block(block: &CodeBlock) -> bool {
    let lower = block.preceding_text.to_lowercase();
    let patterns = [
        "what not to do",
        "don't do this",
        "do not do this",
        "example of bad",
        "example of incorrect",
        "avoid this",
        "incorrect example",
        "wrong way",
        "bad example",
        "anti-pattern",
        "antipattern",
        "here's an example of what not to do",
        "not what you want",
        "this is wrong",
    ];
    patterns.iter().any(|p| lower.contains(p))
}

fn detect_path(block: &CodeBlock) -> Option<String> {
    if let Some(ref lang) = block.language {
        if let Some(colon_pos) = lang.find(':') {
            let path = lang[colon_pos + 1..].trim();
            if !path.is_empty() && looks_like_file_path(path) {
                return Some(clean_path(path));
            }
        }
    }

    if let Some(path) = extract_path_from_comment(&block.content) {
        return Some(path);
    }

    if let Some(path) = extract_path_from_preceding(&block.preceding_text) {
        return Some(path);
    }

    None
}

fn extract_path_from_comment(content: &str) -> Option<String> {
    let first_line = content.lines().next()?.trim();

    let comment_prefixes: &[&str] = &["// ", "# ", "-- ", "/* ", "<!-- ", "; ", "% "];

    for prefix in comment_prefixes {
        if let Some(rest) = first_line.strip_prefix(prefix) {
            let path = rest.trim();
            if looks_like_file_path(path) {
                let cleaned = path
                    .strip_suffix(" */")
                    .or_else(|| path.strip_suffix("*/"))
                    .unwrap_or(path);
                let cleaned = cleaned
                    .strip_suffix(" -->")
                    .or_else(|| cleaned.strip_suffix("-->"))
                    .unwrap_or(cleaned);
                return Some(clean_path(cleaned));
            }
        }
    }

    None
}

fn extract_path_from_preceding(text: &str) -> Option<String> {
    let re_file = Regex::new(r"(?im)^File:\s*(.+)").unwrap();
    if let Some(caps) = re_file.captures(text) {
        let path = caps[1].trim();
        if looks_like_file_path(path) {
            return Some(clean_path(path));
        }
    }

    let re_bold = Regex::new(r"\*\*(.+?)\*\*\s*:?\s*$").unwrap();
    if let Some(caps) = re_bold.captures(text) {
        let path = caps[1].trim();
        if looks_like_file_path(path) {
            return Some(clean_path(path));
        }
    }

    let re_header = Regex::new(r"(?m)^#{1,6}\s+(.+?)\s*$").unwrap();
    if let Some(caps) = re_header.captures(text) {
        let path = caps[1].trim();
        if looks_like_file_path(path) {
            return Some(clean_path(path));
        }
    }

    None
}

fn looks_like_file_path(s: &str) -> bool {
    if s.is_empty() || s.len() > 500 {
        return false;
    }
    if s.contains(|c: char| c == '\n' || c == '\r' || c == '\0') {
        return false;
    }

    let known_extensions: &[&str] = &[
        "rs", "py", "ts", "tsx", "js", "jsx", "go", "java", "c", "cpp", "h", "hpp", "rb", "toml",
        "yaml", "yml", "json", "xml", "html", "css", "scss", "sass", "md", "txt", "sh", "bash",
        "zsh", "fish", "sql", "lua", "ex", "exs", "hs", "ml", "swift", "kt", "dart", "zig", "nim",
        "v", "cu", "proto", "graphql", "gql", "tf", "hcl", "mod", "sum", "lock", "cfg", "ini",
    ];

    if s.contains('/') || s.contains('\\') {
        return true;
    }

    if let Some(dot_pos) = s.rfind('.') {
        let ext = &s[dot_pos + 1..];
        if known_extensions.contains(&ext) {
            return true;
        }
    }

    let known_files = [
        "Makefile",
        "Dockerfile",
        "docker-compose.yml",
        "docker-compose.yaml",
        "Cargo.lock",
        "go.mod",
        "go.sum",
        "package.json",
        "tsconfig.json",
        ".gitignore",
        ".env",
        "Gemfile",
        "Rakefile",
        "CMakeLists.txt",
    ];
    known_files.contains(&s)
}

fn clean_path(path: &str) -> String {
    path.trim().to_string()
}

fn extract_language(lang: &Option<String>) -> Option<String> {
    lang.as_ref()
        .map(|l| l.split(':').next().unwrap_or(l).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_file_with_path_comment() {
        let input = "Here's the implementation:\n\n```rust\n// src/main.rs\nfn hello() -> String {\n    \"Hello, world!\".to_string()\n}\n```";
        let results = parse_llm_output(input);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "src/main.rs");
        assert_eq!(results[0].language, Some("rust".to_string()));
        assert!(results[0].content.contains("fn hello"));
    }

    #[test]
    fn test_multiple_files_with_path_comments() {
        let input = "Here's the implementation:\n\n```rust\n// src/main.rs\nfn hello() -> String {\n    \"Hello\".to_string()\n}\n```\n\nAnd the test:\n\n```python\n# tests/test_main.py\ndef test_hello():\n    assert hello() == \"Hello\"\n```";
        let results = parse_llm_output(input);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path, "src/main.rs");
        assert_eq!(results[0].language, Some("rust".to_string()));
        assert_eq!(results[1].path, "tests/test_main.py");
        assert_eq!(results[1].language, Some("python".to_string()));
    }

    #[test]
    fn test_file_prefix_pattern() {
        let input =
            "File: src/utils.rs\n\n```rust\npub fn add(a: i32, b: i32) -> i32 { a + b }\n```";
        let results = parse_llm_output(input);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "src/utils.rs");
        assert!(results[0].content.contains("pub fn add"));
    }

    #[test]
    fn test_markdown_bold_path() {
        let input = "I'll create the following files:\n\n**src/main.rs**\n\n```rust\nfn main() { println!(\"Hello\"); }\n```\n\n**src/lib.rs**\n\n```rust\npub fn greet() -> &'static str { \"Hi\" }\n```";
        let results = parse_llm_output(input);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path, "src/main.rs");
        assert_eq!(results[1].path, "src/lib.rs");
    }

    #[test]
    fn test_markdown_header_path() {
        let input = "### src/main.rs\n\n```rust\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let results = parse_llm_output(input);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "src/main.rs");
    }

    #[test]
    fn test_no_parseable_files() {
        let input = "Here's some code:\n\n```rust\nfn main() {\n    println!(\"Hello\");\n}\n```\n\nNo file path provided.";
        let results = parse_llm_output(input);

        assert!(results.is_empty());
    }

    #[test]
    fn test_mixed_content() {
        let input = "Here's the main file:\n\n```rust\n// src/main.rs\nfn main() {}\n```\n\nAnd here's an explanation:\n\n```text\nThis is just an explanation.\n```\n\nAnd the lib:\n\n```rust\n// src/lib.rs\npub fn greet() {}\n```";
        let results = parse_llm_output(input);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path, "src/main.rs");
        assert_eq!(results[1].path, "src/lib.rs");
    }

    #[test]
    fn test_various_languages() {
        let input = "```python\n# src/app.py\ndef main(): pass\n```\n\n```go\n// main.go\npackage main\n```\n\n```typescript\n// src/index.ts\nexport const x = 1;\n```";
        let results = parse_llm_output(input);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].language, Some("python".to_string()));
        assert_eq!(results[0].path, "src/app.py");
        assert_eq!(results[1].language, Some("go".to_string()));
        assert_eq!(results[1].path, "main.go");
        assert_eq!(results[2].language, Some("typescript".to_string()));
        assert_eq!(results[2].path, "src/index.ts");
    }

    #[test]
    fn test_claude_style_output() {
        let input = "I'll create the following files:\n\n**src/main.rs**\n```rust\nfn main() {\n    println!(\"Hello, world!\");\n}\n```\n\n**src/lib.rs**\n```rust\npub fn greet() -> &'static str {\n    \"Hi\"\n}\n```\n\n**tests/integration.rs**\n```rust\n#[test]\nfn test_greet() {\n    assert_eq!(greet(), \"Hi\");\n}\n```";
        let results = parse_llm_output(input);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].path, "src/main.rs");
        assert_eq!(results[1].path, "src/lib.rs");
        assert_eq!(results[2].path, "tests/integration.rs");
        assert!(results[2].content.contains("#[test]"));
    }

    #[test]
    fn test_path_in_code_block_header() {
        let input = "```rust:src/main.rs\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let results = parse_llm_output(input);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "src/main.rs");
        assert_eq!(results[0].language, Some("rust".to_string()));
    }

    #[test]
    fn test_example_blocks_ignored() {
        let input = "Here's what NOT to do:\n\n```rust\nfn bad() {}\n```\n\nHere's the correct implementation:\n\n```rust\n// src/main.rs\nfn good() {}\n```";
        let results = parse_llm_output(input);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "src/main.rs");
    }

    #[test]
    fn test_content_whitespace_stripped() {
        let input = "```rust\n// src/main.rs\n\nfn main() {\n    println!(\"Hello\");\n}\n\n```";
        let results = parse_llm_output(input);

        assert_eq!(results.len(), 1);
        assert!(!results[0].content.starts_with('\n'));
        assert!(!results[0].content.ends_with('\n'));
        assert_eq!(results[0].content.lines().next().unwrap(), "// src/main.rs");
    }

    #[test]
    fn test_parse_with_hint_single_block() {
        let input = "```rust\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let results = parse_llm_output_with_hint(input, "src/main.rs");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "src/main.rs");
    }

    #[test]
    fn test_parse_with_hint_ignores_when_path_found() {
        let input = "```rust\n// src/actual.rs\nfn main() {}\n```";
        let results = parse_llm_output_with_hint(input, "src/wrong.rs");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "src/actual.rs");
    }

    #[test]
    fn test_parse_with_hint_multiple_blocks_no_path() {
        let input = "```rust\nfn main() {}\n```\n\n```python\ndef foo(): pass\n```";
        let results = parse_llm_output_with_hint(input, "src/main.rs");

        assert!(results.is_empty());
    }

    #[test]
    fn test_nested_backticks_outer() {
        let input = "Here's the file:\n\n````markdown\n// src/README.md\nSome docs with `inline code` here.\n````";
        let results = parse_llm_output(input);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "src/README.md");
    }

    #[test]
    fn test_sql_comment_path() {
        let input = "```sql\n-- migrations/001_create_users.sql\nCREATE TABLE users (id INT PRIMARY KEY);\n```";
        let results = parse_llm_output(input);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "migrations/001_create_users.sql");
    }
}
