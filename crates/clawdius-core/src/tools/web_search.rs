//! Web search tool supporting multiple providers for LLM grounding

use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::error::Result;

#[derive(Debug, Error)]
pub enum WebSearchError {
    #[error("HTTP request failed: {0}")]
    HttpFailed(String),
    #[error("Failed to parse search results: {0}")]
    ParseFailed(String),
    #[error("URL fetch failed: {0}")]
    FetchFailed(String),
    #[error("HTML extraction failed: {0}")]
    ExtractionFailed(String),
    #[error("Provider not configured: {0}")]
    NotConfigured(String),
    #[error("Rate limited: {0}")]
    RateLimited(String),
}

impl From<WebSearchError> for crate::error::Error {
    fn from(e: WebSearchError) -> Self {
        crate::error::Error::Tool(e.to_string())
    }
}

#[derive(Debug, Clone, Default)]
pub enum SearchProvider {
    #[default]
    DuckDuckGo,
    Google {
        api_key: String,
        cse_id: String,
    },
    Bing {
        api_key: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundedResponse {
    pub content: String,
    pub sources: Vec<SearchResult>,
    pub confidence: f32,
}

pub struct WebSearchTool {
    client: Client,
    provider: SearchProvider,
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new(SearchProvider::default())
    }
}

impl WebSearchTool {
    #[must_use]
    pub fn new(provider: SearchProvider) -> Self {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (compatible; Clawdius/0.2.0)")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { client, provider }
    }

    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        match &self.provider {
            SearchProvider::DuckDuckGo => self.search_duckduckgo(query, limit).await,
            SearchProvider::Google { api_key, cse_id } => {
                self.search_google(query, limit, api_key, cse_id).await
            },
            SearchProvider::Bing { api_key } => self.search_bing(query, limit, api_key).await,
        }
    }

    async fn search_duckduckgo(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            urlencoding::encode(query)
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| WebSearchError::HttpFailed(e.to_string()))?;

        let html = response
            .text()
            .await
            .map_err(|e| WebSearchError::HttpFailed(e.to_string()))?;

        self.parse_duckduckgo_html(&html, limit)
    }

    fn parse_duckduckgo_html(&self, html: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();

        let result_regex =
            Regex::new(r#"<a[^>]*class="result__a"[^>]*href="([^"]+)"[^>]*>([^<]+)</a>"#)
                .map_err(|e| WebSearchError::ParseFailed(e.to_string()))?;

        let snippet_regex =
            Regex::new(r#"<a[^>]*class="result__snippet"[^>]*>([^<]*(?:<[^>]+>[^<]*)*)</a>"#)
                .map_err(|e| WebSearchError::ParseFailed(e.to_string()))?;

        for cap in result_regex.captures_iter(html).take(limit) {
            let url = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
            let title = cap.get(2).map(|m| m.as_str()).unwrap_or_default();

            let url = self.decode_ddg_redirect(url);

            let snippet = snippet_regex
                .captures_iter(html)
                .next()
                .and_then(|s| s.get(1).map(|m| m.as_str()))
                .unwrap_or_default();

            let snippet = self.strip_html_tags(snippet);

            results.push(SearchResult {
                title: self.decode_html_entities(title),
                url,
                snippet: self.decode_html_entities(&snippet),
                source: "DuckDuckGo".to_string(),
            });
        }

        Ok(results)
    }

    fn decode_ddg_redirect(&self, url: &str) -> String {
        if url.contains("duckduckgo.com/l/?uddg=") {
            if let Some(encoded) = url.split("uddg=").nth(1) {
                if let Some(actual_url) = encoded.split('&').next() {
                    return urlencoding::decode(actual_url);
                }
            }
        }
        url.to_string()
    }

    async fn search_google(
        &self,
        query: &str,
        limit: usize,
        api_key: &str,
        cse_id: &str,
    ) -> Result<Vec<SearchResult>> {
        let url = format!(
            "https://www.googleapis.com/customsearch/v1?key={}&cx={}&q={}&num={}",
            api_key,
            cse_id,
            urlencoding::encode(query),
            limit
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| WebSearchError::HttpFailed(e.to_string()))?;

        if response.status() == 429 {
            return Err(WebSearchError::RateLimited("Google API rate limited".into()).into());
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| WebSearchError::ParseFailed(e.to_string()))?;

        let results = json
            .get("items")
            .and_then(|items| items.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        Some(SearchResult {
                            title: item.get("title")?.as_str()?.to_string(),
                            url: item.get("link")?.as_str()?.to_string(),
                            snippet: item
                                .get("snippet")
                                .and_then(|s| s.as_str())
                                .unwrap_or_default()
                                .to_string(),
                            source: "Google".to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }

    async fn search_bing(
        &self,
        query: &str,
        limit: usize,
        api_key: &str,
    ) -> Result<Vec<SearchResult>> {
        let url = format!(
            "https://api.bing.microsoft.com/v7.0/search?q={}&count={}",
            urlencoding::encode(query),
            limit
        );

        let response = self
            .client
            .get(&url)
            .header("Ocp-Apim-Subscription-Key", api_key)
            .send()
            .await
            .map_err(|e| WebSearchError::HttpFailed(e.to_string()))?;

        if response.status() == 429 {
            return Err(WebSearchError::RateLimited("Bing API rate limited".into()).into());
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| WebSearchError::ParseFailed(e.to_string()))?;

        let results = json
            .get("webPages")
            .and_then(|wp| wp.get("value"))
            .and_then(|items| items.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        Some(SearchResult {
                            title: item.get("name")?.as_str()?.to_string(),
                            url: item.get("url")?.as_str()?.to_string(),
                            snippet: item
                                .get("snippet")
                                .and_then(|s| s.as_str())
                                .unwrap_or_default()
                                .to_string(),
                            source: "Bing".to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }

    pub async fn fetch_page(&self, url: &str) -> Result<String> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| WebSearchError::FetchFailed(e.to_string()))?;

        let html = response
            .text()
            .await
            .map_err(|e| WebSearchError::FetchFailed(e.to_string()))?;

        Ok(self.extract_text(&html))
    }

    fn extract_text(&self, html: &str) -> String {
        let text = self.strip_html_tags(html);

        let text = self.decode_html_entities(&text);

        let whitespace_regex = Regex::new(r"\s+").unwrap();
        let text = whitespace_regex.replace_all(&text, " ").to_string();

        let lines: Vec<&str> = text.lines().collect();
        let mut result = String::new();
        let mut prev_empty = false;

        for line in &lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                if !prev_empty {
                    result.push('\n');
                    prev_empty = true;
                }
            } else {
                if !result.is_empty() && !result.ends_with('\n') {
                    result.push(' ');
                }
                result.push_str(trimmed);
                prev_empty = false;
            }
        }

        result.trim().to_string()
    }

    fn strip_html_tags(&self, html: &str) -> String {
        let mut result = String::with_capacity(html.len());
        let mut inside_tag = false;
        let mut inside_script = false;
        let mut inside_style = false;
        let mut tag_name = String::new();
        let mut chars = html.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '<' {
                inside_tag = true;
                tag_name.clear();

                if let Some(&next) = chars.peek() {
                    if next == '!' {
                        while let Some(&ch) = chars.peek() {
                            chars.next();
                            if ch == '>' {
                                break;
                            }
                        }
                        inside_tag = false;
                        continue;
                    }
                }
                continue;
            }

            if inside_tag {
                if c == '>' {
                    let tag_lower = tag_name.to_lowercase();
                    if tag_lower == "script" || tag_lower == "style" {
                        inside_script = tag_lower == "script";
                        inside_style = tag_lower == "style";
                    } else if tag_lower == "/script" {
                        inside_script = false;
                    } else if tag_lower == "/style" {
                        inside_style = false;
                    } else if tag_lower == "br"
                        || tag_lower == "p"
                        || tag_lower == "div"
                        || tag_lower == "/p"
                        || tag_lower == "/div"
                        || tag_lower == "li"
                        || tag_lower == "/li"
                        || tag_lower == "h1"
                        || tag_lower == "h2"
                        || tag_lower == "h3"
                        || tag_lower == "h4"
                        || tag_lower == "h5"
                        || tag_lower == "h6"
                        || tag_lower == "/h1"
                        || tag_lower == "/h2"
                        || tag_lower == "/h3"
                        || tag_lower == "/h4"
                        || tag_lower == "/h5"
                        || tag_lower == "/h6"
                        || tag_lower == "tr"
                        || tag_lower == "/tr"
                    {
                        result.push('\n');
                    }
                    inside_tag = false;
                } else if c.is_ascii_alphabetic() || c == '/' {
                    tag_name.push(c);
                }
                continue;
            }

            if !inside_script && !inside_style {
                result.push(c);
            }
        }

        result
    }

    fn decode_html_entities(&self, text: &str) -> String {
        let mut result = text.to_string();

        let entities = [
            ("&amp;", "&"),
            ("&lt;", "<"),
            ("&gt;", ">"),
            ("&quot;", "\""),
            ("&apos;", "'"),
            ("&nbsp;", " "),
            ("&#39;", "'"),
            ("&mdash;", "—"),
            ("&ndash;", "–"),
            ("&hellip;", "..."),
            ("&rsquo;", "'"),
            ("&lsquo;", "'"),
            ("&rdquo;", "\""),
            ("&ldquo;", "\""),
        ];

        for (entity, replacement) in entities {
            result = result.replace(entity, replacement);
        }

        let numeric_entity = Regex::new(r"&#(\d+);").unwrap();
        result = numeric_entity
            .replace_all(&result, |caps: &regex::Captures<'_>| {
                caps.get(1)
                    .and_then(|m| m.as_str().parse::<u32>().ok())
                    .and_then(char::from_u32)
                    .map(|c| c.to_string())
                    .unwrap_or_default()
            })
            .to_string();

        let hex_entity = Regex::new(r"&#x([0-9a-fA-F]+);").unwrap();
        result = hex_entity
            .replace_all(&result, |caps: &regex::Captures<'_>| {
                caps.get(1)
                    .and_then(|m| u32::from_str_radix(m.as_str(), 16).ok())
                    .and_then(char::from_u32)
                    .map(|c| c.to_string())
                    .unwrap_or_default()
            })
            .to_string();

        result
    }

    pub async fn search_and_fetch(
        &self,
        query: &str,
        limit: usize,
        fetch_top: usize,
    ) -> Result<Vec<(SearchResult, Option<String>)>> {
        let results = self.search(query, limit).await?;

        let mut enriched = Vec::new();
        for (i, result) in results.into_iter().enumerate() {
            let content = if i < fetch_top {
                self.fetch_page(&result.url).await.ok()
            } else {
                None
            };
            enriched.push((result, content));
        }

        Ok(enriched)
    }
}

#[must_use]
pub fn format_results_for_llm(results: &[SearchResult]) -> String {
    let mut output = String::new();

    for (i, result) in results.iter().enumerate() {
        output.push_str(&format!(
            "[{}] {}\n   URL: {}\n   {}\n\n",
            i + 1,
            result.title,
            result.url,
            result.snippet
        ));
    }

    output
}

#[must_use]
pub fn format_grounded_response(response: &GroundedResponse) -> String {
    let mut output = response.content.clone();

    if !response.sources.is_empty() {
        output.push_str("\n\n---\n**Sources:**\n");
        for (i, source) in response.sources.iter().enumerate() {
            output.push_str(&format!("{}. [{}]({})\n", i + 1, source.title, source.url));
        }
    }

    output
}

mod urlencoding {
    pub fn encode(s: &str) -> String {
        url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
    }

    pub fn decode(s: &str) -> String {
        url::form_urlencoded::parse(s.as_bytes())
            .map(|(k, _)| k.into_owned())
            .next()
            .unwrap_or_else(|| s.to_string())
    }
}
