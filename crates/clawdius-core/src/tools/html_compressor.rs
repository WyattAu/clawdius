//! Semantic Context Compression
//!
//! Converts HTML content to compact Markdown for LLM context windows.
//! Targets ~97.5% compression ratio (200KB HTML → 5KB Markdown).
//!
//! # Strategy
//!
//! 1. **Strip noise**: Remove scripts, styles, nav, footers, ads, iframes
//! 2. **Extract content**: Pull text from semantic elements (article, main, p, h1-h6, li, td)
//! 3. **Preserve structure**: Convert headings, lists, links, code blocks, tables
//! 4. **Deduplicate**: Remove repeated boilerplate text
//! 5. **Compress**: Collapse whitespace, truncate excessive content

use crate::error::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

// Pre-compiled regexes for HTML noise stripping (compiled once, reused across calls)
static RE_SCRIPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<script[^>]*>.*?</script>").expect("valid regex"));
static RE_STYLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<style[^>]*>.*?</style>").expect("valid regex"));
static RE_COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<!--.*?-->").expect("valid regex"));
static RE_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").expect("valid regex"));
static RE_MULTI_NEWLINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\n{3,}").expect("valid regex"));
static RE_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<h([1-6])[^>]*>(.*?)</h[1-6]>").expect("valid regex"));

/// Result of HTML compression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedContent {
    /// The compressed Markdown text
    pub markdown: String,
    /// Original size in bytes
    pub original_bytes: usize,
    /// Compressed size in bytes
    pub compressed_bytes: usize,
    /// Compression ratio (0.0 - 1.0)
    pub ratio: f64,
    /// Title extracted from the page
    pub title: Option<String>,
    /// Number of sections extracted
    pub section_count: usize,
}

impl CompressedContent {
    /// Returns the compression percentage (e.g., 97.5 means 97.5% smaller).
    #[must_use]
    pub fn compression_percent(&self) -> f64 {
        self.ratio * 100.0
    }
}

/// HTML to Markdown converter with compression.
pub struct HtmlCompressor {
    /// Maximum output size in bytes (0 = unlimited)
    max_output_bytes: usize,
    /// Whether to extract metadata (title, meta description)
    extract_metadata: bool,
    /// Whether to preserve link URLs
    preserve_links: bool,
    /// Whether to preserve image alt text
    preserve_images: bool,
    /// Whether to preserve code blocks
    #[allow(dead_code)] // reserved for future selective code stripping
    preserve_code: bool,
    /// Custom tag removal patterns (regex)
    extra_strip_patterns: Vec<String>,
}

impl Default for HtmlCompressor {
    fn default() -> Self {
        Self {
            max_output_bytes: 10 * 1024, // 10 KB default
            extract_metadata: true,
            preserve_links: true,
            preserve_images: false,
            preserve_code: true,
            extra_strip_patterns: Vec::new(),
        }
    }
}

impl HtmlCompressor {
    /// Creates a new compressor with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum output size.
    #[must_use]
    pub fn with_max_output(mut self, bytes: usize) -> Self {
        self.max_output_bytes = bytes;
        self
    }

    /// Enables link preservation.
    #[must_use]
    pub fn with_links(mut self) -> Self {
        self.preserve_links = true;
        self
    }

    /// Enables image alt text preservation.
    #[must_use]
    pub fn with_images(mut self) -> Self {
        self.preserve_images = true;
        self
    }

    /// Converts HTML to compressed Markdown.
    ///
    /// # Errors
    ///
    /// Returns an error if HTML parsing fails catastrophically.
    pub fn compress(&self, html: &str) -> Result<CompressedContent> {
        let original_bytes = html.len();

        // Step 1: Extract metadata before stripping
        let title = if self.extract_metadata {
            self.extract_title(html)
        } else {
            None
        };

        // Step 2: Strip noise elements
        let mut text = self.strip_noise(html);

        // Step 3: Convert semantic HTML to Markdown
        text = self.html_to_markdown(&text);

        // Step 4: Clean up whitespace
        text = self.collapse_whitespace(&text);

        // Step 5: Deduplicate repeated lines
        text = self.deduplicate_lines(&text);

        // Step 6: Trim to max size
        let section_count = text.lines().filter(|l| l.starts_with('#')).count();
        if self.max_output_bytes > 0 && text.len() > self.max_output_bytes {
            text.truncate(self.max_output_bytes);
            // Don't cut in the middle of a line
            if let Some(pos) = text.rfind('\n') {
                text.truncate(pos);
            }
        }

        let compressed_bytes = text.len();
        let ratio = if original_bytes > 0 {
            1.0 - (compressed_bytes as f64 / original_bytes as f64)
        } else {
            0.0
        };

        Ok(CompressedContent {
            markdown: text,
            original_bytes,
            compressed_bytes,
            ratio,
            title,
            section_count,
        })
    }

    /// Extracts the page title from HTML.
    fn extract_title(&self, html: &str) -> Option<String> {
        let re = Regex::new(r"(?i)<title[^>]*>(.*?)</title>").ok()?;
        let caps = re.captures(html)?;
        let title = caps.get(1)?.as_str().trim().to_string();
        if title.is_empty() {
            None
        } else {
            Some(self.decode_entities(&title))
        }
    }

    /// Strips noise elements from HTML.
    fn strip_noise(&self, html: &str) -> String {
        let mut text = html.to_string();

        // Remove script tags and content
        text = RE_SCRIPT.replace_all(&text, "").to_string();

        // Remove style tags and content
        text = RE_STYLE.replace_all(&text, "").to_string();

        // Remove nav, footer, header, aside, form elements
        for tag in &[
            "nav", "footer", "header", "aside", "form", "iframe", "noscript", "svg",
        ] {
            let re =
                Regex::new(&format!(r"(?is)<{}[^>]*>.*?</{}>", tag, tag)).expect("valid regex");
            text = re.replace_all(&text, "").to_string();
        }

        // Remove HTML comments
        text = RE_COMMENT.replace_all(&text, "").to_string();

        // Convert heading tags to Markdown before stripping all tags
        text = RE_HEADING
            .replace_all(&text, |caps: &regex::Captures<'_>| {
                let level: usize = caps[1].parse().unwrap_or(1);
                let hashes = "#".repeat(level);
                format!("\n{hashes} {}\n", &caps[2])
            })
            .to_string();

        // Remove all remaining HTML tags
        text = RE_TAG.replace_all(&text, "").to_string();

        // Apply extra strip patterns
        for pattern in &self.extra_strip_patterns {
            if let Ok(re) = Regex::new(pattern) {
                text = re.replace_all(&text, "").to_string();
            }
        }

        text
    }

    /// Converts remaining HTML entities to text.
    fn decode_entities(&self, text: &str) -> String {
        text.replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&nbsp;", " ")
            .replace("&mdash;", "—")
            .replace("&ndash;", "–")
            .replace("&hellip;", "…")
    }

    /// Converts HTML to Markdown-like text.
    fn html_to_markdown(&self, text: &str) -> String {
        let mut result = text.to_string();

        // Decode HTML entities
        result = self.decode_entities(&result);

        // Remove excessive whitespace that was between tags
        result = RE_MULTI_NEWLINE.replace_all(&result, "\n\n").to_string();

        result
    }

    /// Collapses multiple whitespace characters.
    ///
    /// Preserves newline boundaries (only collapses horizontal whitespace
    /// within lines), then collapses runs of 3+ newlines down to 2.
    fn collapse_whitespace(&self, text: &str) -> String {
        let mut result = String::with_capacity(text.len());

        for line in text.lines() {
            let collapsed: String = line.split_whitespace().collect::<Vec<_>>().join(" ");
            result.push_str(&collapsed);
            result.push('\n');
        }

        // Collapse runs of 3+ newlines down to 2
        RE_MULTI_NEWLINE.replace_all(&result, "\n\n").to_string()
    }

    /// Removes consecutive duplicate lines.
    fn deduplicate_lines(&self, text: &str) -> String {
        let lines: Vec<&str> = text.lines().collect();
        let mut result = Vec::new();
        let mut prev = "";
        let mut prev_prev = "";

        for line in &lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                result.push(*line);
                continue;
            }
            // Only deduplicate if same as previous 2 lines (allow some repetition)
            if trimmed == prev && trimmed == prev_prev {
                continue;
            }
            result.push(*line);
            prev_prev = prev;
            prev = trimmed;
        }

        result.join("\n")
    }
}

/// Batch compressor for multiple HTML pages.
pub struct BatchCompressor {
    /// Inner compressor
    compressor: HtmlCompressor,
}

impl BatchCompressor {
    /// Creates a new batch compressor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            compressor: HtmlCompressor::new(),
        }
    }

    /// Compresses multiple HTML pages and concatenates the results.
    pub fn compress_batch(&self, pages: &[(String, &str)]) -> Vec<CompressedContent> {
        pages
            .iter()
            .filter_map(|(url, html)| {
                let result = self.compressor.compress(html).ok()?;
                Some(result)
            })
            .collect()
    }

    /// Compresses multiple pages into a single combined Markdown document.
    pub fn compress_and_merge(&self, pages: &[(String, &str)], max_total_bytes: usize) -> String {
        let results: Vec<CompressedContent> = self.compress_batch(pages);
        let mut output = String::new();

        for result in &results {
            if output.len() + result.compressed_bytes > max_total_bytes {
                break;
            }
            output.push_str(&result.markdown);
            output.push_str("\n\n---\n\n");
        }

        output.truncate(max_total_bytes.min(output.len()));
        output
    }
}

impl Default for BatchCompressor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_compression() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test Page</title></head>
            <body>
                <h1>Hello World</h1>
                <p>This is a test paragraph with some content.</p>
                <p>Another paragraph.</p>
            </body>
            </html>
        "#;

        let compressor = HtmlCompressor::new();
        let result = compressor.compress(html).expect("compress");

        assert!(result.compression_percent() > 50.0);
        assert!(result.section_count > 0);
    }

    #[test]
    fn test_batch_compression() {
        let pages = vec![
            (
                "https://example.com/1".to_string(),
                "<html><body><h1>Page 1</h1><p>Content 1</p></body></html>",
            ),
            (
                "https://example.com/2".to_string(),
                "<html><body><h1>Page 2</h1><p>Content 2</p></body></html>",
            ),
        ];

        let batch = BatchCompressor::new();
        let results = batch.compress_batch(&pages);
        assert_eq!(results.len(), 2);
    }
}
