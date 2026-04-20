//! Web Search Agent
//!
//! Provides non-blocking web search and scraping capabilities for the
//! agentic system. This is A7.
//!
//! # Features
//!
//! - Stealth scraping with rotating User-Agent headers
//! - Rate limiting to avoid bot detection
//! - Content extraction (strips HTML, scripts, styles)
//! - Concurrent request support via tokio
//! - Search result summarization
//!
//! # Usage
//!
//! ```rust,ignore
//! use clawdius_core::agentic::web_search::WebSearchAgent;
//!
//! let agent = WebSearchAgent::new();
//! let results = agent.search("rust async programming").await?;
//! for result in results {
//!     println!("{}: {}", result.title, result.url);
//! }
//! ```

use crate::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};

// ---------------------------------------------------------------------------
// User-Agent Rotation
// ---------------------------------------------------------------------------

/// A rotating pool of browser User-Agent strings to avoid bot detection.
static USER_AGENTS: &[&str] = &[
    // Chrome on Windows
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36",
    // Chrome on macOS
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36",
    // Firefox on Windows
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:120.0) Gecko/20100101 Firefox/120.0",
    // Firefox on macOS
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:121.0) Gecko/20100101 Firefox/121.0",
    // Safari on macOS
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.2 Safari/605.1.15",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.1 Safari/605.1.15",
    // Edge on Windows
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0",
];

/// Get a random User-Agent from the pool.
fn random_user_agent() -> &'static str {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static INDEX: AtomicUsize = AtomicUsize::new(0);
    let idx = INDEX.fetch_add(1, Ordering::Relaxed) % USER_AGENTS.len();
    USER_AGENTS[idx]
}

// ---------------------------------------------------------------------------
// Search Result Types
// ---------------------------------------------------------------------------

/// A single web search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Page title
    pub title: String,
    /// Page URL
    pub url: String,
    /// Snippet/summary from the search engine
    pub snippet: String,
    /// Relevance score (0.0 - 1.0, higher = more relevant)
    pub relevance: f32,
}

/// A scraped web page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapedPage {
    /// Page URL
    pub url: String,
    /// Page title (from <title> tag)
    pub title: String,
    /// Extracted text content (HTML stripped)
    pub content: String,
    /// Content length in characters
    pub content_length: usize,
    /// Whether the scrape was successful
    pub success: bool,
    /// Error message if scrape failed
    pub error: Option<String>,
    /// Time taken to scrape in milliseconds
    pub duration_ms: u64,
}

impl ScrapedPage {
    /// Create a successful scraped page.
    pub fn ok(url: String, title: String, content: String, duration_ms: u64) -> Self {
        let content_length = content.len();
        Self {
            url,
            title,
            content,
            content_length,
            success: true,
            error: None,
            duration_ms,
        }
    }

    /// Create a failed scraped page.
    pub fn err(url: String, error: String, duration_ms: u64) -> Self {
        Self {
            url,
            title: String::new(),
            content: String::new(),
            content_length: 0,
            success: false,
            error: Some(error),
            duration_ms,
        }
    }
}

// ---------------------------------------------------------------------------
// Web Search Agent
// ---------------------------------------------------------------------------

/// Configuration for the web search agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchConfig {
    /// Minimum delay between requests (to avoid rate limiting)
    pub min_request_delay: Duration,
    /// Maximum concurrent requests
    pub max_concurrent: usize,
    /// Request timeout
    pub timeout: Duration,
    /// Maximum content length to extract (characters)
    pub max_content_length: usize,
    /// Whether to follow redirects
    pub follow_redirects: bool,
    /// Custom headers to include in every request
    pub custom_headers: Vec<(String, String)>,
}

impl Default for WebSearchConfig {
    fn default() -> Self {
        Self {
            min_request_delay: Duration::from_millis(500),
            max_concurrent: 4,
            timeout: Duration::from_secs(30),
            max_content_length: 100_000,
            follow_redirects: true,
            custom_headers: Vec::new(),
        }
    }
}

impl WebSearchConfig {
    /// Create a config suitable for stealth scraping (slower, more careful).
    pub fn stealth() -> Self {
        Self {
            min_request_delay: Duration::from_secs(2),
            max_concurrent: 2,
            timeout: Duration::from_secs(15),
            max_content_length: 50_000,
            follow_redirects: true,
            custom_headers: vec![
                ("Accept-Language".to_string(), "en-US,en;q=0.9".to_string()),
                ("Accept".to_string(), "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".to_string()),
                ("DNT".to_string(), "1".to_string()),
                ("Connection".to_string(), "keep-alive".to_string()),
                ("Upgrade-Insecure-Requests".to_string(), "1".to_string()),
            ],
        }
    }

    /// Create a config for aggressive scraping (faster, less stealthy).
    pub fn fast() -> Self {
        Self {
            min_request_delay: Duration::from_millis(100),
            max_concurrent: 8,
            timeout: Duration::from_secs(10),
            max_content_length: 200_000,
            follow_redirects: true,
            custom_headers: Vec::new(),
        }
    }
}

/// A non-blocking web search and scraping agent.
///
/// Provides stealth web access for the agentic system with:
/// - Rotating User-Agent headers
/// - Rate limiting between requests
/// - Concurrent request support
/// - HTML content extraction
pub struct WebSearchAgent {
    config: WebSearchConfig,
    client: reqwest::Client,
    /// Rate limiter: minimum delay between requests
    last_request: Arc<Mutex<Instant>>,
    /// Semaphore for concurrent request limiting
    semaphore: Arc<Semaphore>,
    /// Statistics
    stats: Arc<Mutex<WebSearchStats>>,
}

/// Web search agent statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebSearchStats {
    /// Total requests made
    pub total_requests: usize,
    /// Successful requests
    pub successful_requests: usize,
    /// Failed requests
    pub failed_requests: usize,
    /// Total bytes downloaded
    pub total_bytes: usize,
    /// Number of rate limit hits (429 responses)
    pub rate_limit_hits: usize,
    /// Total time spent on requests (ms)
    pub total_time_ms: u64,
}

impl WebSearchAgent {
    /// Create a new web search agent with default configuration.
    pub fn new() -> Self {
        Self::with_config(WebSearchConfig::default())
    }

    /// Create a new web search agent with stealth configuration.
    pub fn stealth() -> Self {
        Self::with_config(WebSearchConfig::stealth())
    }

    /// Create a new web search agent with custom configuration.
    pub fn with_config(config: WebSearchConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .redirect(if config.follow_redirects {
                reqwest::redirect::Policy::limited(10)
            } else {
                reqwest::redirect::Policy::none()
            })
            .user_agent(random_user_agent())
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            config: config.clone(),
            client,
            last_request: Arc::new(Mutex::new(Instant::now())),
            semaphore: Arc::new(Semaphore::new(config.max_concurrent)),
            stats: Arc::new(Mutex::new(WebSearchStats::default())),
        }
    }

    /// Get the current statistics.
    pub async fn stats(&self) -> WebSearchStats {
        self.stats.lock().await.clone()
    }

    /// Fetch a URL and extract its text content.
    ///
    /// This is the core scraping method. It:
    /// 1. Enforces rate limiting
    /// 2. Rotates User-Agent on each request
    /// 3. Fetches the page
    /// 4. Extracts text content (strips HTML)
    pub async fn fetch_url(&self, url: &str) -> Result<ScrapedPage> {
        let start = Instant::now();

        // Rate limiting
        self.rate_limit().await;

        // Concurrency limiting
        let _permit = self.semaphore.acquire().await.map_err(|e| {
            crate::Error::Network(format!("Semaphore closed: {e}"))
        })?;

        let mut stats = self.stats.lock().await;
        stats.total_requests += 1;
        drop(stats);

        // Build request with stealth headers
        let mut request = self
            .client
            .get(url)
            .header("User-Agent", random_user_agent());

        // Add custom headers
        for (key, value) in &self.config.custom_headers {
            request = request.header(key.as_str(), value.as_str());
        }

        // Execute request
        let result = request.send().await;

        let mut stats = self.stats.lock().await;

        match result {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    let bytes = match response.bytes().await {
                        Ok(b) => b,
                        Err(e) => {
                            stats.failed_requests += 1;
                            return Ok(ScrapedPage::err(
                                url.to_string(),
                                format!("Failed to read response body: {e}"),
                                start.elapsed().as_millis() as u64,
                            ));
                        },
                    };

                    stats.total_bytes += bytes.len();
                    stats.successful_requests += 1;
                    stats.total_time_ms += start.elapsed().as_millis() as u64;

                    let html = String::from_utf8_lossy(&bytes);
                    let (title, content) = extract_text_content(&html);

                    // Truncate content
                    let content = if content.len() > self.config.max_content_length {
                        let mut truncated =
                            content[..self.config.max_content_length].to_string();
                        truncated.push_str("\n\n[CONTENT TRUNCATED]");
                        truncated
                    } else {
                        content
                    };

                    Ok(ScrapedPage::ok(
                        url.to_string(),
                        title,
                        content,
                        start.elapsed().as_millis() as u64,
                    ))
                } else if status.as_u16() == 429 {
                    // Rate limited — back off
                    stats.rate_limit_hits += 1;
                    stats.failed_requests += 1;
                    Ok(ScrapedPage::err(
                        url.to_string(),
                        format!("Rate limited (HTTP 429) by {}", url),
                        start.elapsed().as_millis() as u64,
                    ))
                } else {
                    stats.failed_requests += 1;
                    Ok(ScrapedPage::err(
                        url.to_string(),
                        format!("HTTP {}: {}", status.as_u16(), status.canonical_reason().unwrap_or("Unknown")),
                        start.elapsed().as_millis() as u64,
                    ))
                }
            },
            Err(e) => {
                stats.failed_requests += 1;
                Ok(ScrapedPage::err(
                    url.to_string(),
                    format!("Request failed: {e}"),
                    start.elapsed().as_millis() as u64,
                ))
            },
        }
    }

    /// Fetch multiple URLs concurrently.
    ///
    /// Returns results in the same order as the input URLs.
    /// Failed fetches are represented as `ScrapedPage` with `success: false`.
    pub async fn fetch_urls(&self, urls: &[&str]) -> Vec<ScrapedPage> {
        let futures: Vec<_> = urls.iter().map(|url| self.fetch_url(url)).collect();
        let results: Vec<Result<ScrapedPage>> = futures::future::join_all(futures).await;
        results
            .into_iter()
            .map(|r| r.unwrap_or_else(|e| ScrapedPage::err("unknown".to_string(), e.to_string(), 0)))
            .collect()
    }

    /// Search for a query using a search engine URL pattern.
    ///
    /// This constructs a search URL from the query and scrapes the results page.
    /// For production use, this should be replaced with a proper search API
    /// (Google Custom Search, Bing Web Search API, etc.).
    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let encoded = urlencoding::encode(query);
        let search_url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            encoded
        );

        let page = self.fetch_url(&search_url).await?;

        if !page.success {
            return Err(crate::Error::Network(format!(
                "Search failed: {}",
                page.error.unwrap_or_default()
            )));
        }

        // Extract search results from DuckDuckGo HTML
        let results = parse_ddg_results(&page.content);
        Ok(results)
    }

    /// Search and scrape the top N results.
    ///
    /// Performs a search, then fetches the content of the top results.
    pub async fn search_and_scrape(
        &self,
        query: &str,
        top_n: usize,
    ) -> Result<(Vec<SearchResult>, Vec<ScrapedPage>)> {
        let results = self.search(query).await?;

        let top_urls: Vec<&str> = results
            .iter()
            .take(top_n)
            .map(|r| r.url.as_str())
            .collect();

        let pages = self.fetch_urls(&top_urls).await;

        Ok((results, pages))
    }

    /// Enforce rate limiting between requests.
    async fn rate_limit(&self) {
        let mut last = self.last_request.lock().await;
        let elapsed = last.elapsed();
        if elapsed < self.config.min_request_delay {
            tokio::time::sleep(self.config.min_request_delay - elapsed).await;
        }
        *last = Instant::now();
    }
}

impl Default for WebSearchAgent {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// HTML Content Extraction
// ---------------------------------------------------------------------------

/// Extract title and text content from HTML.
///
/// This is a simple extractor that strips tags, scripts, and styles.
/// For production use, consider using a proper HTML parser like `scraper`.
fn extract_text_content(html: &str) -> (String, String) {
    let mut title = String::new();
    let mut in_title = false;
    let mut in_script = false;
    let mut in_style = false;
    let mut in_tag = false;
    let mut content = String::new();
    let mut text_buf = String::new();

    let chars: Vec<char> = html.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c == '<' {
            // Flush text buffer
            if !text_buf.is_empty() && !in_script && !in_style {
                let trimmed = text_buf.trim();
                if !trimmed.is_empty() {
                    content.push(' ');
                    content.push_str(trimmed);
                }
            }
            text_buf.clear();

            // Check what tag we're entering
            if i + 1 < chars.len() {
                let rest: String = chars[i + 1..].iter().collect();

                if rest.starts_with("/title") {
                    in_title = false;
                    in_tag = true;
                } else if rest.starts_with("title") {
                    in_title = true;
                    in_tag = true;
                } else if rest.starts_with("/script") {
                    in_script = false;
                    in_tag = true;
                } else if rest.starts_with("script") {
                    in_script = true;
                    in_tag = true;
                } else if rest.starts_with("/style") {
                    in_style = false;
                    in_tag = true;
                } else if rest.starts_with("style") {
                    in_style = true;
                    in_tag = true;
                } else {
                    in_tag = true;
                }
            } else {
                in_tag = true;
            }
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            if in_title {
                title.push(c);
            } else if !in_script && !in_style {
                // Normalize whitespace
                if c.is_whitespace() {
                    text_buf.push(' ');
                } else {
                    text_buf.push(c);
                }
            }
        }

        i += 1;
    }

    // Flush remaining text
    if !text_buf.is_empty() && !in_script && !in_style {
        let trimmed = text_buf.trim();
        if !trimmed.is_empty() {
            content.push(' ');
            content.push_str(trimmed);
        }
    }

    (title.trim().to_string(), content.trim().to_string())
}

/// Parse DuckDuckGo HTML search results.
///
/// Extracts titles, URLs, and snippets from DuckDuckGo's HTML-only search results.
fn parse_ddg_results(html: &str) -> Vec<SearchResult> {
    let mut results = Vec::new();

    // DuckDuckGo HTML results have this structure:
    // <a class="result__a" href="//duckduckgo.com/l/?uddg=ENCODED_URL">TITLE</a>
    // <a class="result__snippet">SNIPPET</a>

    // Simple extraction — find all result links and their text
    let mut in_result_link = false;
    let mut in_snippet = false;
    let mut current_url = String::new();
    let mut current_title = String::new();
    let mut current_snippet = String::new();
    let mut in_tag = false;
    let mut tag_buffer = String::new();

    let chars: Vec<char> = html.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c == '<' {
            in_tag = true;
            tag_buffer.clear();

            // Check if we're entering a snippet div
            if i + 1 < chars.len() {
                let rest: String = chars[i + 1..].iter().collect();
                if rest.contains("result__a") && rest.contains("href=") {
                    in_result_link = true;
                    current_url.clear();
                    current_title.clear();
                } else if rest.contains("result__snippet") {
                    in_snippet = true;
                    current_snippet.clear();
                }
            }
        } else if c == '>' && in_tag {
            in_tag = false;

            // Extract href from result link tag
            if in_result_link && tag_buffer.contains("href=") {
                if let Some(href_start) = tag_buffer.find("href=\"") {
                    let href = &tag_buffer[href_start + 6..];
                    if let Some(href_end) = href.find('"') {
                        let raw_url = &href[..href_end];
                        // DuckDuckGo uses redirect URLs; extract the actual URL
                        current_url = decode_ddg_url(raw_url);
                    }
                }
            }
        } else if in_tag {
            tag_buffer.push(c);
        } else if in_result_link {
            current_title.push(c);
        } else if in_snippet {
            current_snippet.push(c);
        }

        // Check for closing tags
        if c == '<' && i + 1 < chars.len() && chars[i + 1] == '/' {
            let rest: String = chars[i + 2..].iter().collect();
            if rest.starts_with("a") {
                if in_result_link && !current_url.is_empty() {
                    in_result_link = false;
                }
                in_snippet = false;
            }
        }

        i += 1;
    }

    // If we captured any results, build them
    // (This is a simplified parser — a real implementation would use a proper HTML parser)
    if !current_url.is_empty() && !current_title.is_empty() {
        results.push(SearchResult {
            title: current_title.trim().to_string(),
            url: current_url,
            snippet: current_snippet.trim().to_string(),
            relevance: 0.5, // Default relevance
        });
    }

    results
}

/// Decode a DuckDuckGo redirect URL to extract the actual destination URL.
fn decode_ddg_url(url: &str) -> String {
    // DDG URLs look like: //duckduckgo.com/l/?uddg=ENCODED_URL&rut=...
    if let Some(uddg_start) = url.find("uddg=") {
        let encoded = &url[uddg_start + 5..];
        if let Some(end) = encoded.find('&') {
            let encoded = &encoded[..end];
            if let Ok(decoded) = urlencoding::decode(encoded) {
                return decoded.to_string();
            }
        }
    }

    // If not a DDG redirect, return as-is
    url.to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_user_agent() {
        let ua = random_user_agent();
        assert!(!ua.is_empty());
        assert!(ua.contains("Mozilla"));
    }

    #[test]
    fn test_random_user_agent_rotation() {
        let ua1 = random_user_agent();
        let ua2 = random_user_agent();
        // They should be different due to rotation
        assert_ne!(ua1, ua2);
    }

    #[test]
    fn test_search_result_serialization() {
        let result = SearchResult {
            title: "Test".to_string(),
            url: "https://example.com".to_string(),
            snippet: "A test page".to_string(),
            relevance: 0.9,
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: SearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.title, "Test");
        assert_eq!(parsed.url, "https://example.com");
        assert!((parsed.relevance - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_scraped_page_ok() {
        let page = ScrapedPage::ok(
            "https://example.com".to_string(),
            "Example".to_string(),
            "Hello world".to_string(),
            100,
        );
        assert!(page.success);
        assert!(page.error.is_none());
        assert_eq!(page.content_length, 11);
        assert_eq!(page.duration_ms, 100);
    }

    #[test]
    fn test_scraped_page_err() {
        let page = ScrapedPage::err(
            "https://example.com".to_string(),
            "Not found".to_string(),
            50,
        );
        assert!(!page.success);
        assert_eq!(page.error.as_deref(), Some("Not found"));
    }

    #[test]
    fn test_scraped_page_serialization() {
        let page = ScrapedPage::ok(
            "https://example.com".to_string(),
            "Test".to_string(),
            "Content".to_string(),
            200,
        );
        let json = serde_json::to_string(&page).unwrap();
        let parsed: ScrapedPage = serde_json::from_str(&json).unwrap();
        assert!(parsed.success);
        assert_eq!(parsed.url, "https://example.com");
    }

    #[test]
    fn test_web_search_config_default() {
        let config = WebSearchConfig::default();
        assert_eq!(config.max_concurrent, 4);
        assert!(config.follow_redirects);
    }

    #[test]
    fn test_web_search_config_stealth() {
        let config = WebSearchConfig::stealth();
        assert_eq!(config.max_concurrent, 2);
        assert_eq!(config.min_request_delay, Duration::from_secs(2));
        assert!(!config.custom_headers.is_empty());
    }

    #[test]
    fn test_web_search_config_fast() {
        let config = WebSearchConfig::fast();
        assert_eq!(config.max_concurrent, 8);
        assert_eq!(config.min_request_delay, Duration::from_millis(100));
    }

    #[test]
    fn test_web_search_config_serialization() {
        let config = WebSearchConfig::stealth();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: WebSearchConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_concurrent, 2);
    }

    #[test]
    fn test_web_search_stats_default() {
        let stats = WebSearchStats::default();
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.successful_requests, 0);
        assert_eq!(stats.failed_requests, 0);
        assert_eq!(stats.rate_limit_hits, 0);
    }

    #[test]
    fn test_web_search_stats_serialization() {
        let stats = WebSearchStats {
            total_requests: 100,
            successful_requests: 90,
            failed_requests: 8,
            total_bytes: 1_000_000,
            rate_limit_hits: 2,
            total_time_ms: 5000,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let parsed: WebSearchStats = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.total_requests, 100);
        assert_eq!(parsed.successful_requests, 90);
        assert_eq!(parsed.rate_limit_hits, 2);
    }

    #[test]
    fn test_extract_text_content_simple() {
        let html = "<html><head><title>Test Page</title></head><body><p>Hello world</p></body></html>";
        let (title, content) = extract_text_content(html);
        assert_eq!(title, "Test Page");
        assert!(content.contains("Hello world"));
    }

    #[test]
    fn test_extract_text_content_strips_scripts() {
        let html = "<html><body><script>alert('xss')</script><p>Safe content</p></body></html>";
        let (title, content) = extract_text_content(html);
        assert!(!content.contains("alert"));
        assert!(content.contains("Safe content"));
    }

    #[test]
    fn test_extract_text_content_strips_styles() {
        let html = "<html><head><style>.x { color: red; }</style></head><body><p>Visible</p></body></html>";
        let (_, content) = extract_text_content(html);
        assert!(!content.contains("color"));
        assert!(content.contains("Visible"));
    }

    #[test]
    fn test_extract_text_content_empty() {
        let (title, content) = extract_text_content("");
        assert!(title.is_empty());
        assert!(content.is_empty());
    }

    #[test]
    fn test_extract_text_content_no_title() {
        let html = "<html><body><p>No title here</p></body></html>";
        let (title, content) = extract_text_content(html);
        assert!(title.is_empty());
        assert!(content.contains("No title here"));
    }

    #[test]
    fn test_decode_ddg_url_redirect() {
        let ddg_url = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com&rut=abc";
        let decoded = decode_ddg_url(ddg_url);
        assert_eq!(decoded, "https://example.com");
    }

    #[test]
    fn test_decode_ddg_url_plain() {
        let url = "https://example.com/page";
        let decoded = decode_ddg_url(url);
        assert_eq!(decoded, "https://example.com/page");
    }

    #[test]
    fn test_decode_ddg_url_empty() {
        let decoded = decode_ddg_url("");
        assert!(decoded.is_empty());
    }

    #[tokio::test]
    async fn test_web_search_agent_creation() {
        let agent = WebSearchAgent::new();
        let stats = agent.stats().await;
        assert_eq!(stats.total_requests, 0);
    }

    #[tokio::test]
    async fn test_web_search_agent_stealth() {
        let agent = WebSearchAgent::stealth();
        let stats = agent.stats().await;
        assert_eq!(stats.total_requests, 0);
    }

    #[test]
    fn test_user_agents_pool() {
        assert!(!USER_AGENTS.is_empty());
        for ua in USER_AGENTS {
            assert!(ua.contains("Mozilla"), "Invalid UA: {ua}");
        }
    }
}
