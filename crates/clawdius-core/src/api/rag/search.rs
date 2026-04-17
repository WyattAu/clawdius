use super::indexer::CodeChunk;

pub struct SearchResult {
    pub chunk: CodeChunk,
    pub score: f64,
}

pub struct CodebaseSearch {
    chunks: Vec<CodeChunk>,
}

impl CodebaseSearch {
    #[must_use] 
    pub const fn new(chunks: Vec<CodeChunk>) -> Self {
        Self { chunks }
    }

    #[must_use] 
    pub fn search(&self, query: &str, max_results: usize) -> Vec<SearchResult> {
        let query_lower = query.to_lowercase();
        let query_terms: Vec<&str> = query_lower.split_whitespace().collect();

        let mut results: Vec<SearchResult> = self
            .chunks
            .iter()
            .filter_map(|chunk| {
                let content_lower = chunk.content.to_lowercase();
                let matches: usize = query_terms
                    .iter()
                    .filter(|term| content_lower.contains(*term))
                    .count();

                if matches == 0 {
                    return None;
                }

                let match_score = matches as f64 / query_terms.len() as f64;
                let length_penalty = 1.0 / (1.0 + (chunk.token_count as f64 / 1000.0));
                let file_relevance = if chunk.language == "rust" { 1.2 } else { 1.0 };

                Some(SearchResult {
                    chunk: chunk.clone(),
                    score: match_score * length_penalty * file_relevance,
                })
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(max_results);
        results
    }

    #[must_use] 
    pub fn list_files(&self) -> Vec<String> {
        let mut files: Vec<String> = self.chunks.iter().map(|c| c.file_path.clone()).collect();
        files.sort();
        files.dedup();
        files
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chunk(id: &str, file_path: &str, language: &str, content: &str) -> CodeChunk {
        CodeChunk {
            id: id.to_string(),
            file_path: file_path.to_string(),
            start_line: 0,
            end_line: content.lines().count(),
            language: language.to_string(),
            content: content.to_string(),
            token_count: content.len() / 4,
        }
    }

    #[test]
    fn test_search_finds_matching_chunks() {
        let chunks = vec![
            make_chunk("1", "main.rs", "rust", "fn main() { println!(\"hello\"); }"),
            make_chunk("2", "lib.rs", "rust", "pub fn helper() { let x = 1; }"),
            make_chunk("3", "README.md", "markdown", "This is a readme file."),
        ];

        let search = CodebaseSearch::new(chunks);
        let results = search.search("println", 5);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].chunk.id, "1");
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn test_search_multiple_terms() {
        let chunks = vec![
            make_chunk("1", "main.rs", "rust", "fn main() { let x = 1; }"),
            make_chunk(
                "2",
                "utils.rs",
                "rust",
                "fn helper() { let x = 2; println!(\"x\"); }",
            ),
        ];

        let search = CodebaseSearch::new(chunks);
        let results = search.search("helper println", 5);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].chunk.id, "2");
    }

    #[test]
    fn test_search_respects_max_results() {
        let chunks: Vec<CodeChunk> = (0..10)
            .map(|i| {
                make_chunk(
                    &format!("{}", i),
                    &format!("file{}.rs", i),
                    "rust",
                    &format!("fn func_{}() {{ let x = {}; }}", i, i),
                )
            })
            .collect();

        let search = CodebaseSearch::new(chunks);
        let results = search.search("fn let", 3);

        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_search_no_matches() {
        let chunks = vec![make_chunk("1", "main.rs", "rust", "fn main() {}")];
        let search = CodebaseSearch::new(chunks);
        let results = search.search("nonexistent_query_xyz", 5);

        assert!(results.is_empty());
    }

    #[test]
    fn test_search_rust_boost() {
        let rust_content = "fn foo() { bar(); }";
        let py_content = "def foo(): bar()";
        let rust_tokens = rust_content.len() / 4;
        let py_tokens = py_content.len() / 4;

        let mut rust_chunk = make_chunk("1", "main.rs", "rust", rust_content);
        rust_chunk.token_count = rust_tokens;
        let mut py_chunk = make_chunk("2", "main.py", "python", py_content);
        py_chunk.token_count = py_tokens;

        let search = CodebaseSearch::new(vec![rust_chunk, py_chunk]);
        let results = search.search("foo bar", 5);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].chunk.language, "rust");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn test_list_files_unique_sorted() {
        let chunks = vec![
            make_chunk("1", "z.rs", "rust", "content"),
            make_chunk("2", "a.rs", "rust", "content"),
            make_chunk("3", "a.rs", "rust", "more content"),
            make_chunk("4", "b.rs", "rust", "content"),
        ];

        let search = CodebaseSearch::new(chunks);
        let files = search.list_files();

        assert_eq!(files, vec!["a.rs", "b.rs", "z.rs"]);
    }

    #[test]
    fn test_list_files_empty() {
        let search = CodebaseSearch::new(vec![]);
        let files = search.list_files();
        assert!(files.is_empty());
    }
}
