use async_trait::async_trait;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::llm::ChatMessage;
use crate::llm::LlmClient;
use crate::rpc::handlers::Handler;
use crate::rpc::types::{Request, Response};

#[derive(Debug, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub prefix: String,
    #[serde(default)]
    pub suffix: String,
    pub language: String,
    pub file_path: String,
    #[serde(default)]
    pub line: u32,
    #[serde(default)]
    pub character: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub text: String,
}

/// Completion cache entry
#[derive(Debug, Clone)]
struct CacheEntry {
    completion: String,
    timestamp: Instant,
}

pub struct CompletionHandler {
    llm: Option<Arc<dyn LlmClient>>,
    cache: Arc<RwLock<LruCache<String, CacheEntry>>>,
    cache_ttl: Duration,
    timeout: Duration,
}

impl CompletionHandler {
    #[must_use]
    pub fn new() -> Self {
        let cache_size = NonZeroUsize::new(100).unwrap_or(NonZeroUsize::MIN);
        Self {
            llm: None,
            cache: Arc::new(RwLock::new(LruCache::new(cache_size))),
            cache_ttl: Duration::from_secs(300), // 5 minutes
            timeout: Duration::from_secs(5),     // 5 seconds
        }
    }

    pub fn with_llm(llm: Arc<dyn LlmClient>) -> Self {
        let mut handler = Self::new();
        handler.llm = Some(llm);
        handler
    }

    fn cache_key(&self, req: &CompletionRequest) -> String {
        // Use last 100 chars of prefix for cache key
        let prefix_snippet = if req.prefix.len() > 100 {
            &req.prefix[req.prefix.len() - 100..]
        } else {
            &req.prefix
        };

        format!("{}:{}:{}", req.language, req.file_path, prefix_snippet)
    }

    async fn get_cached(&self, key: &str) -> Option<String> {
        let mut cache = self.cache.write().await;
        if let Some(entry) = cache.get(key) {
            if entry.timestamp.elapsed() < self.cache_ttl {
                return Some(entry.completion.clone());
            }
            cache.pop(key);
        }
        None
    }

    async fn cache_completion(&self, key: String, completion: String) {
        let mut cache = self.cache.write().await;
        cache.put(
            key,
            CacheEntry {
                completion,
                timestamp: Instant::now(),
            },
        );
    }

    fn normalize_language<'a>(&self, lang: &'a str) -> &'a str {
        match lang {
            "typescript" | "typescriptreact" => "TypeScript",
            "javascript" | "javascriptreact" => "JavaScript",
            "python" => "Python",
            "rust" => "Rust",
            "go" => "Go",
            "java" => "Java",
            "c" | "cpp" | "c++" => "C++",
            "ruby" => "Ruby",
            "php" => "PHP",
            "swift" => "Swift",
            "kotlin" => "Kotlin",
            _ => lang,
        }
    }

    fn extract_completion(&self, response: &str, _prefix: &str) -> String {
        let completion = response.trim();

        if completion.starts_with("```") {
            let lines: Vec<&str> = completion.lines().collect();
            if lines.len() > 2 {
                return lines[1..lines.len() - 1].join("\n");
            }
        }

        completion.to_string()
    }
}

#[async_trait]
impl Handler for CompletionHandler {
    async fn handle(&self, request: Request) -> Response {
        let params = match request.params {
            Some(p) => p,
            None => return Response::invalid_params(request.id, "Missing parameters"),
        };

        let completion_req: CompletionRequest = match serde_json::from_value(params) {
            Ok(r) => r,
            Err(e) => {
                return Response::invalid_params(request.id, format!("Invalid parameters: {e}"))
            },
        };

        // Check cache first
        let cache_key = self.cache_key(&completion_req);
        if let Some(cached) = self.get_cached(&cache_key).await {
            return Response::success(request.id, CompletionResponse { text: cached });
        }

        // Try LLM if available
        if let Some(ref llm) = self.llm {
            let messages = self.build_messages(&completion_req);

            // Execute with timeout
            let result = tokio::time::timeout(self.timeout, llm.chat(messages)).await;

            match result {
                Ok(Ok(response)) => {
                    let text = self.extract_completion(&response, &completion_req.prefix);
                    // Cache the result
                    self.cache_completion(cache_key, text.clone()).await;
                    Response::success(request.id, CompletionResponse { text })
                },
                Ok(Err(e)) => {
                    warn!("LLM completion failed: {}, falling back to mock", e);
                    let mock_completion = self.generate_smart_completion(&completion_req);
                    Response::success(request.id, mock_completion)
                },
                Err(_) => {
                    warn!("LLM completion timed out, falling back to mock");
                    let mock_completion = self.generate_smart_completion(&completion_req);
                    Response::success(request.id, mock_completion)
                },
            }
        } else {
            debug!("No LLM configured, using smart mock completion");
            let mock_completion = self.generate_smart_completion(&completion_req);
            Response::success(request.id, mock_completion)
        }
    }
}

impl CompletionHandler {
    /// Generate smart completions when LLM is unavailable
    /// Uses language-specific patterns and context
    fn generate_smart_completion(&self, req: &CompletionRequest) -> CompletionResponse {
        let lines: Vec<&str> = req.prefix.lines().collect();
        let last_line = lines.last().unwrap_or(&"");
        let trimmed = last_line.trim();

        let text = match req.language.as_str() {
            "rust" => self.rust_completion(trimmed, &lines),
            "python" => self.python_completion(trimmed, &lines),
            "javascript" | "typescript" => self.js_completion(trimmed, &lines),
            "go" => self.go_completion(trimmed, &lines),
            _ => self.generic_completion(trimmed, &lines),
        };

        CompletionResponse { text }
    }

    fn rust_completion(&self, line: &str, _lines: &[&str]) -> String {
        if line.starts_with("fn ") && line.contains('{') {
            // Function definition - suggest common patterns
            if line.contains("async") {
                "\n    // TODO: Implement async function\n    Ok(())\n".to_string()
            } else {
                "\n    // TODO: Implement function\n".to_string()
            }
        } else if line.starts_with("impl ") {
            "\n    // TODO: Implement trait methods\n".to_string()
        } else if line.starts_with("struct ") {
            " {\n    // Fields\n}".to_string()
        } else if line.starts_with("enum ") {
            " {\n    // Variants\n}".to_string()
        } else if line.starts_with("//") {
            // Continue comment
            String::new()
        } else if line.contains("todo!") {
            // Already has todo, don't add more
            String::new()
        } else {
            String::new()
        }
    }

    fn python_completion(&self, line: &str, _lines: &[&str]) -> String {
        if line.starts_with("def ") && line.contains(':') {
            "\n    \"\"\"TODO: Add docstring\"\"\"\n    pass\n".to_string()
        } else if line.starts_with("class ") && line.contains(':') {
            "\n    \"\"\"TODO: Add class docstring\"\"\"\n    pass\n".to_string()
        } else if line.starts_with("async def ") && line.contains(':') {
            "\n    \"\"\"TODO: Add async docstring\"\"\"\n    pass\n".to_string()
        } else {
            String::new()
        }
    }

    fn js_completion(&self, line: &str, _lines: &[&str]) -> String {
        if (line.starts_with("function ")
            || line.starts_with("const ")
            || line.starts_with("async "))
            && line.contains('{')
        {
            "\n  // TODO: Implement\n".to_string()
        } else if line.starts_with("class ") && line.contains('{') {
            "\n  constructor() {\n    // TODO: Initialize\n  }\n".to_string()
        } else {
            String::new()
        }
    }

    fn go_completion(&self, line: &str, _lines: &[&str]) -> String {
        if line.starts_with("func ") && line.contains('{') {
            "\n\t// TODO: Implement\n".to_string()
        } else if line.starts_with("type ") && line.contains("struct") {
            " {\n\t// Fields\n}".to_string()
        } else {
            String::new()
        }
    }

    fn generic_completion(&self, _line: &str, _lines: &[&str]) -> String {
        String::new()
    }

    fn build_messages(&self, req: &CompletionRequest) -> Vec<ChatMessage> {
        let language = self.normalize_language(&req.language);

        let system_prompt = format!(
            "You are an expert code completion AI. Complete the following {language} code.\n\
            Rules:\n\
            1. Only return the completion, not the entire code\n\
            2. Do not include any explanations or comments unless they are part of the code\n\
            3. Continue from where the cursor is positioned\n\
            4. Keep completions concise and relevant (max 5-10 lines)\n\
            5. Match the existing code style and indentation"
        );

        let mut user_content = String::new();

        // Include last 2000 chars for context
        if req.prefix.len() > 2000 {
            user_content.push_str("...[earlier code]...\n");
            user_content.push_str(&req.prefix[req.prefix.len() - 2000..]);
        } else {
            user_content.push_str(&req.prefix);
        }

        // Add suffix if available
        if !req.suffix.is_empty() {
            user_content.push_str("\n\n[CURSOR POSITION]\n\n");
            user_content.push_str(&req.suffix[..req.suffix.len().min(500)]);
        }

        vec![
            ChatMessage {
                role: crate::llm::ChatRole::System,
                content: system_prompt,
            },
            ChatMessage {
                role: crate::llm::ChatRole::User,
                content: user_content,
            },
        ]
    }
}

impl Default for CompletionHandler {
    fn default() -> Self {
        Self::new()
    }
}
