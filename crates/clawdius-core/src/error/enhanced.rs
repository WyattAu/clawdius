//! Enhanced error messages with context and suggestions

use std::error::Error;
use std::fmt;

/// Enhanced error with context and suggestions
#[derive(Debug)]
pub struct EnhancedError {
    /// Error message
    message: String,

    /// Error context (what was happening)
    context: Option<String>,

    /// Suggested actions
    suggestions: Vec<String>,

    /// Related documentation links
    doc_links: Vec<String>,

    /// Error code for lookup
    error_code: Option<String>,

    /// Source error (if wrapping)
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl EnhancedError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            context: None,
            suggestions: vec![],
            doc_links: vec![],
            error_code: None,
            source: None,
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }

    pub fn with_doc_link(mut self, link: impl Into<String>) -> Self {
        self.doc_links.push(link.into());
        self
    }

    pub fn with_error_code(mut self, code: impl Into<String>) -> Self {
        self.error_code = Some(code.into());
        self
    }

    #[must_use]
    pub fn with_source(mut self, source: Box<dyn Error + Send + Sync>) -> Self {
        self.source = Some(source);
        self
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn context(&self) -> Option<&str> {
        self.context.as_deref()
    }

    #[must_use]
    pub fn suggestions(&self) -> &[String] {
        &self.suggestions
    }

    #[must_use]
    pub fn doc_links(&self) -> &[String] {
        &self.doc_links
    }

    #[must_use]
    pub fn error_code(&self) -> Option<&str> {
        self.error_code.as_deref()
    }

    #[must_use]
    pub fn format_pretty(&self) -> String {
        let mut output = String::new();

        if let Some(code) = &self.error_code {
            output.push_str(&format!("Error [{}]: {}\n\n", code, self.message));
        } else {
            output.push_str(&format!("Error: {}\n\n", self.message));
        }

        if let Some(ctx) = &self.context {
            output.push_str(&format!("Context: {ctx}\n\n"));
        }

        if !self.suggestions.is_empty() {
            output.push_str("Suggestions:\n");
            for (i, suggestion) in self.suggestions.iter().enumerate() {
                output.push_str(&format!("  {}. {}\n", i + 1, suggestion));
            }
            output.push('\n');
        }

        if !self.doc_links.is_empty() {
            output.push_str("Documentation:\n");
            for link in &self.doc_links {
                output.push_str(&format!("  - {link}\n"));
            }
            output.push('\n');
        }

        if let Some(source) = &self.source {
            output.push_str(&format!("Caused by: {source}\n"));
        }

        output
    }
}

impl fmt::Display for EnhancedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for EnhancedError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|s| s.as_ref() as &(dyn Error + 'static))
    }
}

/// Common error helpers
pub struct ErrorHelpers;

impl ErrorHelpers {
    #[must_use]
    pub fn llm_api_error(provider: &str, status: u16, message: &str) -> EnhancedError {
        let suggestion = if status == 429 {
            "You've hit a rate limit. Wait a moment and try again."
        } else if status >= 500 {
            "The API service is experiencing issues. Try again later."
        } else {
            "Check the error message for specific issues."
        };

        EnhancedError::new(format!("{provider} API request failed (HTTP {status})"))
            .with_context(format!("Calling {provider} LLM API"))
            .with_suggestion("Check your API key is valid and has sufficient credits")
            .with_suggestion("Verify your network connection")
            .with_suggestion(suggestion)
            .with_doc_link("https://docs.anthropic.com/api/errors")
            .with_error_code(format!("LLM_API_{status}"))
            .with_source(Box::new(std::io::Error::other(message.to_string())))
    }

    #[must_use]
    pub fn file_not_found(path: &str) -> EnhancedError {
        EnhancedError::new(format!("File not found: {path}"))
            .with_context("Attempting to read file")
            .with_suggestion("Check the file path is correct")
            .with_suggestion("Verify the file exists")
            .with_suggestion("Ensure you have permission to access the file")
            .with_error_code("FILE_NOT_FOUND")
    }

    #[must_use]
    pub fn invalid_config(field: &str, value: &str) -> EnhancedError {
        EnhancedError::new(format!("Invalid configuration for '{field}': {value}"))
            .with_context("Parsing configuration file")
            .with_suggestion(format!(
                "Check the '{field}' field in .clawdius/config.toml"
            ))
            .with_suggestion("Refer to documentation for valid values")
            .with_doc_link("https://clawdius.dev/docs/configuration")
            .with_error_code("CONFIG_INVALID")
    }

    #[must_use]
    pub fn tool_execution_failed(tool: &str, error: &str) -> EnhancedError {
        EnhancedError::new(format!("Tool '{tool}' execution failed"))
            .with_context(format!("Executing {tool} tool"))
            .with_suggestion("Check tool parameters are valid")
            .with_suggestion("Verify required dependencies are installed")
            .with_error_code(format!("TOOL_{}_FAILED", tool.to_uppercase()))
            .with_source(Box::new(std::io::Error::other(error.to_string())))
    }

    #[must_use]
    pub fn session_not_found(id: &str) -> EnhancedError {
        EnhancedError::new(format!("Session not found: {id}"))
            .with_context("Loading session")
            .with_suggestion("Check the session ID is correct")
            .with_suggestion("List available sessions with 'clawdius sessions'")
            .with_error_code("SESSION_NOT_FOUND")
    }

    #[must_use]
    pub fn out_of_context(max_tokens: usize) -> EnhancedError {
        EnhancedError::new("Context window limit exceeded")
            .with_context("Processing conversation")
            .with_suggestion(format!(
                "Reduce context size (current limit: {max_tokens} tokens)"
            ))
            .with_suggestion("Use session compaction to summarize old messages")
            .with_suggestion("Start a new session for a fresh context")
            .with_error_code("CONTEXT_OVERFLOW")
    }

    #[must_use]
    pub fn sandbox_violation(command: &str) -> EnhancedError {
        EnhancedError::new(format!("Sandbox violation: '{command}' not allowed"))
            .with_context("Executing shell command in sandbox")
            .with_suggestion("This command is blocked for security")
            .with_suggestion("Check .clawdius/config.toml for allowed commands")
            .with_suggestion("Use --no-sandbox flag with caution (not recommended)")
            .with_doc_link("https://clawdius.dev/docs/security/sandbox")
            .with_error_code("SANDBOX_VIOLATION")
    }

    #[must_use]
    pub fn api_key_missing(provider: &str, env_var: &str) -> EnhancedError {
        EnhancedError::new(format!("API key not set for {provider}"))
            .with_context("Initializing LLM provider")
            .with_suggestion(format!("Set the {env_var} environment variable"))
            .with_suggestion("Or add it to your keyring with: clawdius auth set-key <provider>")
            .with_suggestion(format!("Example: export {env_var}=your-api-key"))
            .with_doc_link("https://clawdius.dev/docs/configuration#api-keys")
            .with_error_code(format!("{}_API_KEY_MISSING", provider.to_uppercase()))
    }

    #[must_use]
    pub fn unknown_provider(provider: &str, supported: &[&str]) -> EnhancedError {
        EnhancedError::new(format!("Unknown provider: {provider}"))
            .with_context("Creating LLM provider")
            .with_suggestion(format!("Supported providers: {}", supported.join(", ")))
            .with_suggestion("Check for typos in your configuration")
            .with_error_code("UNKNOWN_PROVIDER")
    }

    #[must_use]
    pub fn database_error(operation: &str, details: &str) -> EnhancedError {
        EnhancedError::new(format!("Database error during {operation}"))
            .with_context("Database operation")
            .with_suggestion("Check that the database file exists and is readable")
            .with_suggestion("Verify you have write permissions")
            .with_suggestion("Try running 'clawdius db repair' if the database is corrupted")
            .with_error_code("DATABASE_ERROR")
            .with_source(Box::new(std::io::Error::other(details.to_string())))
    }

    #[must_use]
    pub fn timeout_error(operation: &str, duration_secs: u64) -> EnhancedError {
        EnhancedError::new(format!("{operation} timed out after {duration_secs}s"))
            .with_context(format!("Waiting for {operation}"))
            .with_suggestion("Try increasing the timeout in your configuration")
            .with_suggestion("Check if the operation is taking longer than expected")
            .with_suggestion("Verify network connectivity if this is a remote operation")
            .with_error_code("TIMEOUT")
    }

    #[must_use]
    pub fn rate_limited(retry_after_ms: u64) -> EnhancedError {
        let retry_secs = retry_after_ms / 1000;
        EnhancedError::new("Rate limit exceeded")
            .with_context("Calling LLM API")
            .with_suggestion(format!("Wait {retry_secs} seconds before retrying"))
            .with_suggestion("Consider reducing request frequency")
            .with_suggestion("Check your API usage limits")
            .with_error_code("RATE_LIMITED")
    }

    #[must_use]
    pub fn retry_exhausted(attempts: u32, last_error: &str) -> EnhancedError {
        EnhancedError::new(format!("All {attempts} retry attempts failed"))
            .with_context("LLM API call with retry")
            .with_suggestion("Check if the service is experiencing issues")
            .with_suggestion("Verify your network connection is stable")
            .with_suggestion("Consider increasing the retry count in configuration")
            .with_error_code("RETRY_EXHAUSTED")
            .with_source(Box::new(std::io::Error::other(last_error.to_string())))
    }
}

impl From<crate::Error> for EnhancedError {
    fn from(error: crate::Error) -> Self {
        match &error {
            crate::Error::Config(msg) => {
                if msg.contains("API_KEY") {
                    let provider = if msg.contains("ANTHROPIC") {
                        "Anthropic"
                    } else if msg.contains("OPENAI") {
                        "OpenAI"
                    } else if msg.contains("ZAI") {
                        "Z.AI"
                    } else {
                        "unknown"
                    };
                    let env_var = format!("{}_API_KEY", provider.to_uppercase().replace('.', ""));
                    ErrorHelpers::api_key_missing(provider, &env_var)
                } else {
                    EnhancedError::new(msg.clone())
                        .with_context("Configuration")
                        .with_error_code("CONFIG_ERROR")
                }
            }
            crate::Error::Llm(msg) | crate::Error::LlmProvider { message: msg, .. } => {
                EnhancedError::new(msg.clone())
                    .with_context("LLM operation")
                    .with_suggestion("Check the error message for details")
                    .with_error_code("LLM_ERROR")
            }
            crate::Error::RateLimited { retry_after_ms } => {
                ErrorHelpers::rate_limited(*retry_after_ms)
            }
            crate::Error::ContextLimit { current, limit } => {
                EnhancedError::new(format!("Context limit exceeded: {current}/{limit} tokens"))
                    .with_context("Processing conversation")
                    .with_suggestion("Use session compaction to reduce context size")
                    .with_suggestion("Start a new session")
                    .with_error_code("CONTEXT_LIMIT")
            }
            crate::Error::ToolExecution { tool, reason } => {
                ErrorHelpers::tool_execution_failed(tool, reason)
            }
            crate::Error::SessionNotFound { id } => ErrorHelpers::session_not_found(id),
            crate::Error::Sandbox(msg) => ErrorHelpers::sandbox_violation(msg),
            crate::Error::Timeout(duration) => {
                ErrorHelpers::timeout_error("Operation", duration.as_secs())
            }
            crate::Error::RetryExhausted(attempts) => {
                ErrorHelpers::retry_exhausted(*attempts, "Multiple failures")
            }
            _ => EnhancedError::new(error.to_string()).with_error_code("GENERAL_ERROR"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhanced_error_creation() {
        let error = EnhancedError::new("Test error")
            .with_context("Testing")
            .with_suggestion("Try this");

        assert_eq!(error.message(), "Test error");
        assert!(error.context().is_some());
        assert_eq!(error.suggestions().len(), 1);
    }

    #[test]
    fn test_pretty_format() {
        let error = ErrorHelpers::file_not_found("/test/path");
        let formatted = error.format_pretty();

        assert!(formatted.contains("File not found"));
        assert!(formatted.contains("Suggestions:"));
    }

    #[test]
    fn test_llm_api_error() {
        let error = ErrorHelpers::llm_api_error("Anthropic", 429, "Rate limit");
        assert!(error.error_code().is_some());
        assert!(!error.suggestions().is_empty());
    }

    #[test]
    fn test_api_key_missing() {
        let error = ErrorHelpers::api_key_missing("Anthropic", "ANTHROPIC_API_KEY");
        assert!(error.message().contains("API key"));
        assert!(error
            .suggestions()
            .iter()
            .any(|s| s.contains("ANTHROPIC_API_KEY")));
    }

    #[test]
    fn test_sandbox_violation() {
        let error = ErrorHelpers::sandbox_violation("rm -rf /");
        assert!(error.message().contains("Sandbox violation"));
        assert!(error.error_code().unwrap().contains("SANDBOX"));
    }

    #[test]
    fn test_from_error() {
        let original = crate::Error::SessionNotFound {
            id: "test-123".to_string(),
        };
        let enhanced: EnhancedError = original.into();

        assert!(enhanced.message().contains("Session not found"));
        assert!(enhanced.error_code().unwrap().contains("SESSION"));
    }
}
