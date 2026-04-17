//! Multi-Model Review Engine
//!
//! Runs independent code reviews from multiple LLM providers concurrently,
//! then fuses the results into a unified review. This is the M4 milestone.

use crate::llm::providers::LlmClient;
use crate::llm::{ChatMessage, ChatRole, LlmConfig};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Configuration for a single reviewer in the multi-model pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewerConfig {
    /// Human-readable name for this reviewer (e.g., "Claude Code Quality")
    pub name: String,
    /// LLM configuration for this reviewer
    pub llm_config: LlmConfig,
    /// Specialized review focus area
    pub focus: ReviewFocus,
}

/// The focus area for a reviewer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReviewFocus {
    /// General code quality, readability, maintainability
    CodeQuality,
    /// Security vulnerabilities, injection risks, auth issues
    Security,
    /// Performance, memory usage, algorithmic complexity
    Performance,
    /// Error handling, edge cases, robustness
    Robustness,
    /// API design, interface consistency
    ApiDesign,
    /// Test coverage, test quality, testability
    Testing,
    /// General / all areas
    General,
}

impl ReviewFocus {
    pub fn display_name(&self) -> &'static str {
        match self {
            ReviewFocus::CodeQuality => "Code Quality",
            ReviewFocus::Security => "Security",
            ReviewFocus::Performance => "Performance",
            ReviewFocus::Robustness => "Robustness",
            ReviewFocus::ApiDesign => "API Design",
            ReviewFocus::Testing => "Testing",
            ReviewFocus::General => "General",
        }
    }

    /// Returns a specialized system prompt for this review focus.
    pub fn system_prompt(&self) -> String {
        match self {
            ReviewFocus::CodeQuality => {
                "You are an expert code reviewer focused on code quality.\n\
                 Evaluate:\n\
                 1. Readability and clarity\n\
                 2. Naming conventions and consistency\n\
                 3. Code organization and modularity\n\
                 4. DRY principle adherence\n\
                 5. Documentation quality\n\
                 Rate each area 1-5 and provide specific suggestions."
                    .to_string()
            },
            ReviewFocus::Security => "You are a security-focused code reviewer.\n\
                 Evaluate:\n\
                 1. Input validation and sanitization\n\
                 2. Authentication and authorization patterns\n\
                 3. Secrets and credentials handling\n\
                 4. SQL/command injection risks\n\
                 5. Unsafe operations (if applicable)\n\
                 Rate each area 1-5. Flag any critical security issues."
                .to_string(),
            ReviewFocus::Performance => "You are a performance-focused code reviewer.\n\
                 Evaluate:\n\
                 1. Algorithmic complexity (Big O)\n\
                 2. Memory allocation patterns\n\
                 3. Unnecessary copies or clones\n\
                 4. I/O efficiency\n\
                 5. Concurrency and parallelism opportunities\n\
                 Rate each area 1-5 and suggest optimizations."
                .to_string(),
            ReviewFocus::Robustness => "You are a robustness-focused code reviewer.\n\
                 Evaluate:\n\
                 1. Error handling completeness\n\
                 2. Edge case coverage\n\
                 3. Resource cleanup (drop, cleanup on error)\n\
                 4. Input boundary validation\n\
                 5. Graceful degradation\n\
                 Rate each area 1-5 and identify gaps."
                .to_string(),
            ReviewFocus::ApiDesign => "You are an API design reviewer.\n\
                 Evaluate:\n\
                 1. Interface clarity and consistency\n\
                 2. Parameter naming and types\n\
                 3. Error type design\n\
                 4. Backwards compatibility\n\
                 5. Documentation of public APIs\n\
                 Rate each area 1-5."
                .to_string(),
            ReviewFocus::Testing => "You are a testing-focused code reviewer.\n\
                 Evaluate:\n\
                 1. Test coverage of critical paths\n\
                 2. Edge case and error path testing\n\
                 3. Test readability and maintainability\n\
                 4. Mock/stub usage appropriateness\n\
                 5. Testability of the code under review\n\
                 Rate each area 1-5."
                .to_string(),
            ReviewFocus::General => {
                "You are a senior staff engineer doing a comprehensive code review.\n\
                 Evaluate all aspects:\n\
                 - Correctness: Does the code do what it's supposed to?\n\
                 - Edge cases: Are error paths handled?\n\
                 - Security: Any vulnerabilities?\n\
                 - Performance: Any anti-patterns?\n\
                 - Style: Consistent with project conventions?\n\
                 Rate each area 1-5 and provide specific feedback."
                    .to_string()
            },
        }
    }
}

/// Result from a single reviewer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    /// Which reviewer produced this result
    pub reviewer_name: String,
    /// Review focus area
    pub focus: ReviewFocus,
    /// The full review text
    pub review_text: String,
    /// Individual findings
    pub findings: Vec<ReviewFinding>,
    /// Overall score (1-5) for this focus area
    pub overall_score: u8,
    /// Whether any critical issues were found
    pub has_critical_issues: bool,
    /// Token usage for this review
    pub tokens_used: usize,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// A single review finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFinding {
    /// Severity: 1 (info) to 5 (critical)
    pub severity: u8,
    /// File or location reference (if applicable)
    pub location: Option<String>,
    /// Description of the finding
    pub description: String,
    /// Suggested fix (if applicable)
    pub suggestion: Option<String>,
}

/// Fused review from multiple reviewers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusedReview {
    /// Individual review results
    pub reviews: Vec<ReviewResult>,
    /// Unified summary across all reviewers
    pub summary: String,
    /// Aggregated findings, deduplicated and sorted by severity
    pub aggregated_findings: Vec<ReviewFinding>,
    /// Average score across all reviewers
    pub average_score: f32,
    /// Whether any reviewer found critical issues
    pub has_critical_issues: bool,
    /// Total tokens used across all reviewers
    pub total_tokens: usize,
    /// Total duration across all reviewers
    pub total_duration_ms: u64,
}

/// The multi-model review engine.
pub struct ReviewEngine {
    /// Configured reviewers
    reviewers: Vec<ReviewerConfig>,
}

impl ReviewEngine {
    /// Create a new review engine with the given reviewer configurations.
    pub fn new(reviewers: Vec<ReviewerConfig>) -> Self {
        Self { reviewers }
    }

    /// Create a default review engine with two reviewers:
    /// one for code quality and one for security.
    pub fn default_dual_review(primary_config: LlmConfig) -> Self {
        let security_config = LlmConfig {
            model: primary_config.model.clone(),
            ..primary_config.clone()
        };

        Self {
            reviewers: vec![
                ReviewerConfig {
                    name: "Code Quality Reviewer".to_string(),
                    llm_config: primary_config,
                    focus: ReviewFocus::CodeQuality,
                },
                ReviewerConfig {
                    name: "Security Reviewer".to_string(),
                    llm_config: security_config,
                    focus: ReviewFocus::Security,
                },
            ],
        }
    }

    /// Run all reviewers concurrently and fuse the results.
    pub async fn review(&self, code: &str, context: &str) -> Result<FusedReview> {
        let mut handles = Vec::new();

        for reviewer_config in &self.reviewers {
            let code = code.to_string();
            let context = context.to_string();
            let config = reviewer_config.clone();

            handles.push(tokio::spawn(async move {
                Self::run_single_reviewer(&config, &code, &context).await
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(Ok(result)) => results.push(result),
                Ok(Err(e)) => {
                    eprintln!("Reviewer failed: {e}");
                    // Don't fail the entire review if one reviewer fails
                },
                Err(e) => {
                    eprintln!("Reviewer task panicked: {e}");
                },
            }
        }

        Ok(Self::fuse_reviews(results))
    }

    /// Run a single reviewer.
    async fn run_single_reviewer(
        config: &ReviewerConfig,
        code: &str,
        context: &str,
    ) -> Result<ReviewResult> {
        let start = std::time::Instant::now();

        // Create the LLM provider
        let provider = crate::llm::create_provider(&config.llm_config)?;

        let system_prompt = config.focus.system_prompt();
        let user_message = format!(
            "## Code to Review\n\n```\n{code}\n```\n\n## Context\n{context}\n\n\
             Provide your review focusing on {}. End with an overall score (1-5).",
            config.focus.display_name()
        );

        let messages = vec![
            ChatMessage {
                role: ChatRole::System,
                content: system_prompt,
            },
            ChatMessage {
                role: ChatRole::User,
                content: user_message,
            },
        ];

        let review_text = match provider.chat(messages).await {
            Ok(output) => output,
            Err(_) => {
                // Fallback for models that don't support system messages
                let combined = format!(
                    "[Instructions: {}]\n\n[Code]\n```\n{code}\n```\n\n[Context]\n{context}",
                    config.focus.system_prompt(),
                    code = code,
                    context = context
                );
                let fallback_messages = vec![ChatMessage {
                    role: ChatRole::User,
                    content: combined,
                }];
                provider.chat(fallback_messages).await?
            },
        };

        let tokens = provider.count_tokens(&review_text);
        let duration_ms = start.elapsed().as_millis() as u64;

        // Extract findings and score from the review text
        let (findings, overall_score, has_critical) = Self::parse_review_output(&review_text);

        Ok(ReviewResult {
            reviewer_name: config.name.clone(),
            focus: config.focus.clone(),
            review_text,
            findings,
            overall_score,
            has_critical_issues: has_critical,
            tokens_used: tokens,
            duration_ms,
        })
    }

    /// Parse review output to extract findings, score, and critical issues.
    fn parse_review_output(text: &str) -> (Vec<ReviewFinding>, u8, bool) {
        let mut findings = Vec::new();
        let mut overall_score: u8 = 3; // default middle score
        let mut has_critical = false;

        // Try to extract the overall score (look for "overall score: X" or "score: X/5")
        for line in text.lines() {
            let lower = line.to_lowercase();
            if lower.contains("overall score") || lower.contains("score:") {
                // Extract the first number found
                if let Some(num_str) = line
                    .split(|c: char| !c.is_ascii_digit())
                    .filter(|s| !s.is_empty())
                    .nth(0)
                {
                    if let Ok(num) = num_str.parse::<u8>() {
                        if num >= 1 && num <= 5 {
                            overall_score = num;
                        }
                    }
                }
            }

            // Check for critical issues
            if lower.contains("critical")
                || lower.contains("severity: 5")
                || lower.contains("severity:5")
            {
                has_critical = true;
            }

            // Check for numbered findings
            if let Some(severity_str) = Self::extract_severity(line) {
                let description = line
                    .split(|c: char| c == '.' || c == ')')
                    .nth(1)
                    .unwrap_or("")
                    .trim()
                    .to_string();

                if !description.is_empty() {
                    findings.push(ReviewFinding {
                        severity: severity_str,
                        location: None,
                        description,
                        suggestion: None,
                    });

                    if severity_str >= 5 {
                        has_critical = true;
                    }
                }
            }
        }

        // If no findings were parsed, create a single summary finding
        if findings.is_empty() {
            // Take the first non-empty line as the main finding
            let summary = text
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("Review completed")
                .trim()
                .to_string();

            let severity = if overall_score <= 2 { 4 } else { 2 };

            findings.push(ReviewFinding {
                severity,
                location: None,
                description: summary,
                suggestion: None,
            });
        }

        (findings, overall_score, has_critical)
    }

    /// Try to extract a severity number from a line.
    fn extract_severity(line: &str) -> Option<u8> {
        let lower = line.to_lowercase();
        let keywords = [
            ("severity: ", 1),
            ("severity:", 1),
            ("[critical]", 5),
            ("[high]", 4),
            ("[medium]", 3),
            ("[low]", 2),
            ("[info]", 1),
        ];

        for (keyword, base_severity) in &keywords {
            if let Some(pos) = lower.find(keyword) {
                let after = &lower[pos + keyword.len()..];
                // Try to parse a number after the keyword
                if let Some(num_str) = after
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse::<u8>()
                    .ok()
                {
                    if num_str >= 1 && num_str <= 5 {
                        return Some(num_str);
                    }
                }
                // Use the base severity from the keyword
                return Some(*base_severity);
            }
        }

        None
    }

    /// Fuse multiple review results into a unified review.
    fn fuse_reviews(reviews: Vec<ReviewResult>) -> FusedReview {
        if reviews.is_empty() {
            return FusedReview {
                reviews: Vec::new(),
                summary: "No reviews completed.".to_string(),
                aggregated_findings: Vec::new(),
                average_score: 0.0,
                has_critical_issues: false,
                total_tokens: 0,
                total_duration_ms: 0,
            };
        }

        let total_tokens: usize = reviews.iter().map(|r| r.tokens_used).sum();
        let total_duration_ms: u64 = reviews.iter().map(|r| r.duration_ms).sum();
        let average_score =
            reviews.iter().map(|r| r.overall_score as f32).sum::<f32>() / reviews.len() as f32;
        let has_critical = reviews.iter().any(|r| r.has_critical_issues);

        // Aggregate findings, sorted by severity (descending)
        let mut all_findings: Vec<ReviewFinding> =
            reviews.iter().flat_map(|r| r.findings.clone()).collect();
        all_findings.sort_by(|a, b| b.severity.cmp(&a.severity));

        // Deduplicate findings with similar descriptions (simple approach)
        let aggregated_findings = Self::deduplicate_findings(all_findings);

        // Generate summary
        let mut summary = String::new();
        summary.push_str(&format!(
            "## Fused Review ({} reviewers, avg score: {:.1}/5)\n\n",
            reviews.len(),
            average_score
        ));

        for review in &reviews {
            summary.push_str(&format!(
                "### {} ({})\nScore: {}/5\n\n{}\n\n",
                review.reviewer_name,
                review.focus.display_name(),
                review.overall_score,
                &review.review_text[..review.review_text.len().min(500)]
            ));
            if review.review_text.len() > 500 {
                summary.push_str("...\n\n");
            }
        }

        if has_critical {
            summary.push_str("⚠️ **Critical issues found. Address before merging.**\n");
        }

        FusedReview {
            reviews,
            summary,
            aggregated_findings,
            average_score,
            has_critical_issues: has_critical,
            total_tokens,
            total_duration_ms,
        }
    }

    /// Simple deduplication of findings based on description similarity.
    fn deduplicate_findings(findings: Vec<ReviewFinding>) -> Vec<ReviewFinding> {
        let mut result = Vec::new();
        let mut seen: Vec<String> = Vec::new();

        for finding in findings {
            // Normalize description for comparison
            let normalized = finding.description.to_lowercase();
            let is_duplicate = seen.iter().any(|prev| {
                // Check word overlap: count words from shorter that appear in longer
                let prev_words: Vec<&str> = prev.split_whitespace().collect();
                let curr_words: Vec<&str> = normalized.split_whitespace().collect();
                let (shorter, longer) = if prev_words.len() < curr_words.len() {
                    (&prev_words, &curr_words)
                } else {
                    (&curr_words, &prev_words)
                };
                if shorter.is_empty() {
                    return false;
                }
                let overlap = shorter.iter().filter(|w| longer.contains(w)).count();
                let similarity = overlap as f32 / shorter.len() as f32;
                similarity > 0.8 // 80% word overlap threshold
            });

            if !is_duplicate {
                seen.push(normalized);
                result.push(finding);
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use tokio::sync::mpsc;

    struct MockReviewLlm {
        response: String,
    }

    impl MockReviewLlm {
        fn new(response: &str) -> Self {
            Self {
                response: response.to_string(),
            }
        }
    }

    #[async_trait]
    impl LlmClient for MockReviewLlm {
        async fn chat(&self, _messages: Vec<ChatMessage>) -> Result<String> {
            Ok(self.response.clone())
        }

        async fn chat_stream(&self, _messages: Vec<ChatMessage>) -> Result<mpsc::Receiver<String>> {
            let (tx, rx) = mpsc::channel(1);
            let _ = tx.send(self.response.clone()).await;
            Ok(rx)
        }

        fn count_tokens(&self, text: &str) -> usize {
            text.split_whitespace().count()
        }
    }

    #[test]
    fn test_review_focus_display_names() {
        assert_eq!(ReviewFocus::CodeQuality.display_name(), "Code Quality");
        assert_eq!(ReviewFocus::Security.display_name(), "Security");
        assert_eq!(ReviewFocus::Performance.display_name(), "Performance");
        assert_eq!(ReviewFocus::General.display_name(), "General");
    }

    #[test]
    fn test_review_focus_system_prompts() {
        for focus in &[
            ReviewFocus::CodeQuality,
            ReviewFocus::Security,
            ReviewFocus::Performance,
            ReviewFocus::Robustness,
            ReviewFocus::ApiDesign,
            ReviewFocus::Testing,
            ReviewFocus::General,
        ] {
            let prompt = focus.system_prompt();
            assert!(!prompt.is_empty());
            assert!(prompt.len() > 50);
            assert!(prompt.contains("1.")); // Has numbered items
        }
    }

    #[test]
    fn test_review_finding_serialization() {
        let finding = ReviewFinding {
            severity: 4,
            location: Some("src/main.rs:42".to_string()),
            description: "Memory leak in loop".to_string(),
            suggestion: Some("Use scoped lifetime".to_string()),
        };
        let json = serde_json::to_string(&finding).unwrap();
        let parsed: ReviewFinding = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.severity, 4);
        assert_eq!(parsed.description, "Memory leak in loop");
    }

    #[test]
    fn test_review_result_serialization() {
        let result = ReviewResult {
            reviewer_name: "Test Reviewer".to_string(),
            focus: ReviewFocus::Security,
            review_text: "Looks good overall.".to_string(),
            findings: vec![ReviewFinding {
                severity: 2,
                location: None,
                description: "Minor issue".to_string(),
                suggestion: None,
            }],
            overall_score: 4,
            has_critical_issues: false,
            tokens_used: 100,
            duration_ms: 500,
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: ReviewResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.reviewer_name, "Test Reviewer");
        assert_eq!(parsed.overall_score, 4);
        assert!(!parsed.has_critical_issues);
    }

    #[test]
    fn test_fused_review_serialization() {
        let fused = FusedReview {
            reviews: Vec::new(),
            summary: "No reviews.".to_string(),
            aggregated_findings: Vec::new(),
            average_score: 3.5,
            has_critical_issues: false,
            total_tokens: 200,
            total_duration_ms: 1000,
        };
        let json = serde_json::to_string(&fused).unwrap();
        let parsed: FusedReview = serde_json::from_str(&json).unwrap();
        assert!(!parsed.has_critical_issues);
        assert_eq!(parsed.total_tokens, 200);
    }

    #[test]
    fn test_parse_review_output_with_score() {
        let text = "## Review\n\nSome feedback here.\n\nOverall score: 4/5\n\nGood code.";
        let (findings, score, has_critical) = ReviewEngine::parse_review_output(text);
        assert_eq!(score, 4);
        assert!(!has_critical);
        assert!(!findings.is_empty());
    }

    #[test]
    fn test_parse_review_output_with_critical() {
        let text = "## Review\n\n[CRITICAL] SQL injection vulnerability in query.\n\nSeverity: 5\n\nOverall score: 2";
        let (findings, score, has_critical) = ReviewEngine::parse_review_output(text);
        assert_eq!(score, 2);
        assert!(has_critical);
    }

    #[test]
    fn test_parse_review_output_default_score() {
        let text = "Just some text without a score.";
        let (_findings, score, has_critical) = ReviewEngine::parse_review_output(text);
        assert_eq!(score, 3); // default
        assert!(!has_critical);
    }

    #[test]
    fn test_extract_severity() {
        assert_eq!(
            ReviewEngine::extract_severity("Severity: 5 - critical issue"),
            Some(5)
        );
        assert_eq!(
            ReviewEngine::extract_severity("[CRITICAL] This is bad"),
            Some(5)
        );
        assert_eq!(
            ReviewEngine::extract_severity("[HIGH] Important issue"),
            Some(4)
        );
        assert_eq!(ReviewEngine::extract_severity("[LOW] Minor thing"), Some(2));
        assert_eq!(ReviewEngine::extract_severity("No severity here"), None);
    }

    #[test]
    fn test_deduplicate_findings() {
        let findings = vec![
            ReviewFinding {
                severity: 4,
                location: None,
                description: "Memory leak in the main processing loop".to_string(),
                suggestion: None,
            },
            ReviewFinding {
                severity: 4,
                location: None,
                description: "Memory leak in the main processing loop".to_string(), // exact duplicate
                suggestion: None,
            },
            ReviewFinding {
                severity: 2,
                location: None,
                description: "Missing documentation for public API".to_string(),
                suggestion: None,
            },
        ];
        let deduped = ReviewEngine::deduplicate_findings(findings);
        assert_eq!(deduped.len(), 2); // First and third should survive
    }

    #[test]
    fn test_fuse_reviews_empty() {
        let fused = ReviewEngine::fuse_reviews(Vec::new());
        assert!(fused.reviews.is_empty());
        assert_eq!(fused.average_score, 0.0);
        assert!(!fused.has_critical_issues);
    }

    #[test]
    fn test_fuse_reviews_multiple() {
        let reviews = vec![
            ReviewResult {
                reviewer_name: "Quality".to_string(),
                focus: ReviewFocus::CodeQuality,
                review_text: "Good code.".to_string(),
                findings: Vec::new(),
                overall_score: 4,
                has_critical_issues: false,
                tokens_used: 50,
                duration_ms: 100,
            },
            ReviewResult {
                reviewer_name: "Security".to_string(),
                focus: ReviewFocus::Security,
                review_text: "Critical auth issue.".to_string(),
                findings: vec![ReviewFinding {
                    severity: 5,
                    location: None,
                    description: "Auth bypass".to_string(),
                    suggestion: None,
                }],
                overall_score: 2,
                has_critical_issues: true,
                tokens_used: 80,
                duration_ms: 200,
            },
        ];
        let fused = ReviewEngine::fuse_reviews(reviews);
        assert_eq!(fused.reviews.len(), 2);
        assert!((fused.average_score - 3.0).abs() < 0.1);
        assert!(fused.has_critical_issues);
        assert_eq!(fused.total_tokens, 130);
        assert_eq!(fused.total_duration_ms, 300);
        assert!(!fused.summary.is_empty());
    }

    #[test]
    fn test_reviewer_config_serialization() {
        let config = ReviewerConfig {
            name: "Test".to_string(),
            llm_config: LlmConfig {
                provider: "openrouter".to_string(),
                model: "test-model".to_string(),
                api_key: Some("key".to_string()),
                base_url: None,
                max_tokens: 100,
            },
            focus: ReviewFocus::Security,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ReviewerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "Test");
        assert_eq!(parsed.focus, ReviewFocus::Security);
    }
}
