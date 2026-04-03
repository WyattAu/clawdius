use crate::error::Result;
use crate::llm::{ChatMessage, ChatRole, LlmClient};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecoveryConfig {
    pub max_retries: usize,
    pub include_compiler_output: bool,
    pub include_test_output: bool,
}

impl Default for ErrorRecoveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            include_compiler_output: true,
            include_test_output: true,
        }
    }
}

impl ErrorRecoveryConfig {
    #[must_use]
    pub fn new(max_retries: usize) -> Self {
        Self::default().with_max_retries(max_retries)
    }

    #[must_use]
    pub fn with_max_retries(mut self, max_retries: usize) -> Self {
        self.max_retries = max_retries;
        self
    }

    #[must_use]
    pub fn with_compiler_output(mut self, include: bool) -> Self {
        self.include_compiler_output = include;
        self
    }

    #[must_use]
    pub fn with_test_output(mut self, include: bool) -> Self {
        self.include_test_output = include;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilationError {
    pub file_path: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub message: String,
    pub error_code: Option<String>,
}

impl CompilationError {
    #[must_use]
    pub fn new(message: String) -> Self {
        Self {
            file_path: None,
            line: None,
            column: None,
            message,
            error_code: None,
        }
    }

    #[must_use]
    pub fn with_location(
        mut self,
        file_path: Option<String>,
        line: Option<u32>,
        column: Option<u32>,
    ) -> Self {
        self.file_path = file_path;
        self.line = line;
        self.column = column;
        self
    }

    #[must_use]
    pub fn with_error_code(mut self, code: Option<String>) -> Self {
        self.error_code = code;
        self
    }

    #[must_use]
    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref code) = self.error_code {
            parts.push(format!("[{}]", code));
        }
        if let Some(ref path) = self.file_path {
            let loc = match (self.line, self.column) {
                (Some(l), Some(c)) => format!("{}:{}:{}", path, l, c),
                (Some(l), None) => format!("{}:{}", path, l),
                _ => path.clone(),
            };
            parts.push(loc);
        }
        parts.push(self.message.clone());
        parts.join(": ")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecoveryResult {
    pub fixed_code: String,
    pub retries_used: usize,
    pub errors_remaining: Vec<CompilationError>,
    pub success: bool,
}

pub fn parse_compiler_output(output: &str) -> Vec<CompilationError> {
    let mut errors = Vec::new();

    let rust_error_re =
        Regex::new(r"error\[([A-Z]\d+)\]: (.+?)(?:\n|$)\s*(?:.*?--> (.*?):(\d+):(\d+))?")
            .expect("rust error regex must compile");

    let rust_error_re_no_code =
        Regex::new(r"error(?:\[(.+?)\])?: (.+?)(?:\n|$)\s*(?:.*?--> (.*?):(\d+):(\d+))?")
            .expect("rust error regex (no code) must compile");

    let python_re = Regex::new(r#"File "(.*?)", line (\d+)(?:, in .+?)?\n(.+?)(?:\n|$)"#)
        .expect("python error regex must compile");

    let typescript_re = Regex::new(r"(.*?\((\d+),(\d+)\)):\s*error\s+(TS\d+):\s*(.+?)(?:\n|$)")
        .expect("typescript error regex must compile");

    if rust_error_re.is_match(output) || rust_error_re_no_code.is_match(output) {
        for caps in rust_error_re.captures_iter(output) {
            let mut err = CompilationError::new(
                caps.get(2)
                    .map_or(String::new(), |m| m.as_str().to_string()),
            );
            err.error_code = caps.get(1).map(|m| m.as_str().to_string());
            err.file_path = caps.get(3).map(|m| m.as_str().to_string());
            err.line = caps.get(4).and_then(|m| m.as_str().parse().ok());
            err.column = caps.get(5).and_then(|m| m.as_str().parse().ok());
            errors.push(err);
        }
        for caps in rust_error_re_no_code.captures_iter(output) {
            if errors.iter().any(|e| {
                e.error_code.is_some()
                    && caps.get(1).is_some()
                    && e.error_code == caps.get(1).map(|m| m.as_str().to_string())
            }) {
                continue;
            }
            let mut err = CompilationError::new(
                caps.get(2)
                    .map_or(String::new(), |m| m.as_str().to_string()),
            );
            err.error_code = caps.get(1).map(|m| m.as_str().to_string());
            err.file_path = caps.get(3).map(|m| m.as_str().to_string());
            err.line = caps.get(4).and_then(|m| m.as_str().parse().ok());
            err.column = caps.get(5).and_then(|m| m.as_str().parse().ok());
            errors.push(err);
        }
    } else if python_re.is_match(output) {
        for caps in python_re.captures_iter(output) {
            let mut err = CompilationError::new(
                caps.get(3)
                    .map_or(String::new(), |m| m.as_str().to_string()),
            );
            err.file_path = caps.get(1).map(|m| m.as_str().to_string());
            err.line = caps.get(2).and_then(|m| m.as_str().parse().ok());
            errors.push(err);
        }
    } else if typescript_re.is_match(output) {
        for caps in typescript_re.captures_iter(output) {
            let mut err = CompilationError::new(
                caps.get(4)
                    .map_or(String::new(), |m| m.as_str().to_string()),
            );
            err.file_path = Some(caps.get(1).map_or("", |m| m.as_str()).to_string());
            err.line = caps.get(2).and_then(|m| m.as_str().parse().ok());
            err.column = caps.get(3).and_then(|m| m.as_str().parse().ok());
            err.error_code = caps.get(4).map(|m| m.as_str().to_string());
            errors.push(err);
        }
    } else {
        let trimmed = output.trim();
        if !trimmed.is_empty() {
            errors.push(CompilationError::new(trimmed.to_string()));
        }
    }

    errors
}

pub fn build_fix_prompt(
    original_code: &str,
    errors: &[CompilationError],
    language: &str,
) -> String {
    let lang_instructions = match language.to_lowercase().as_str() {
        "rust" => {
            "Make sure all types are annotated correctly, all imports are present, \
             and all trait bounds are satisfied. Check for missing `mut`, incorrect lifetimes, \
             and unused variables."
        },
        "python" => {
            "Check for indentation errors, missing imports, undefined variables, \
             incorrect function signatures, and type annotation issues."
        },
        "typescript" | "javascript" => {
            "Check for type mismatches, missing type annotations, incorrect property access, \
             and import/export issues."
        },
        "go" => {
            "Check for missing imports, incorrect type usage, unused variables, \
             and missing return statements."
        },
        _ => "Fix all syntax and type errors.",
    };

    let mut error_list = String::new();
    for (i, err) in errors.iter().enumerate() {
        error_list.push_str(&format!("{}. {}\n", i + 1, err.display()));
    }

    format!(
        "The following {lang} code has compilation errors. Fix ALL errors.\n\n\
         {lang_instructions}\n\n\
         ## Original Code\n\
         ```{lang}\n{original_code}\n```\n\n\
         ## Errors\n\
         {error_list}\n\
         ## Instructions\n\
         Respond with ONLY the complete fixed code. No explanations, no markdown \
         fences, no commentary. Output just the corrected code ready to compile.\n",
        lang = language,
        original_code = original_code,
        lang_instructions = lang_instructions,
        error_list = error_list,
    )
}

fn strip_code_block(response: &str) -> String {
    let code_block_re =
        Regex::new(r"```(?:\w*)\n([\s\S]*?)```").expect("code block regex must compile");

    if let Some(caps) = code_block_re.captures(response) {
        if let Some(m) = caps.get(1) {
            return m.as_str().trim().to_string();
        }
    }

    response.trim().to_string()
}

pub struct ErrorRecovery {
    config: ErrorRecoveryConfig,
    llm: Arc<dyn LlmClient>,
}

impl ErrorRecovery {
    #[must_use]
    pub fn new(llm: Arc<dyn LlmClient>) -> Self {
        Self {
            config: ErrorRecoveryConfig::default(),
            llm,
        }
    }

    #[must_use]
    pub fn with_config(llm: Arc<dyn LlmClient>, config: ErrorRecoveryConfig) -> Self {
        Self { config, llm }
    }

    #[must_use]
    pub const fn config(&self) -> &ErrorRecoveryConfig {
        &self.config
    }

    pub async fn attempt_fix(
        &self,
        code: &str,
        errors: &[CompilationError],
        language: &str,
    ) -> Result<String> {
        let prompt = build_fix_prompt(code, errors, language);

        let messages = vec![
            ChatMessage {
                role: ChatRole::System,
                content: "You are an expert programmer. Fix the compilation errors in the code. \
                          Output ONLY the fixed code, nothing else."
                    .to_string(),
            },
            ChatMessage {
                role: ChatRole::User,
                content: prompt,
            },
        ];

        let response = self.llm.chat(messages).await?;
        Ok(strip_code_block(&response))
    }

    pub async fn recover(
        &self,
        code: &str,
        errors: &[CompilationError],
        language: &str,
    ) -> Result<ErrorRecoveryResult> {
        if errors.is_empty() {
            return Ok(ErrorRecoveryResult {
                fixed_code: code.to_string(),
                retries_used: 0,
                errors_remaining: Vec::new(),
                success: true,
            });
        }

        let mut current_code = code.to_string();
        let mut current_errors = errors.to_vec();

        for retry in 0..self.config.max_retries {
            match self
                .attempt_fix(&current_code, &current_errors, language)
                .await
            {
                Ok(fixed_code) => {
                    current_code = fixed_code;
                    current_errors = Vec::new();
                    return Ok(ErrorRecoveryResult {
                        fixed_code: current_code,
                        retries_used: retry + 1,
                        errors_remaining: current_errors,
                        success: true,
                    });
                },
                Err(e) => {
                    current_errors = vec![CompilationError::new(format!(
                        "LLM fix attempt failed: {e}"
                    ))];
                },
            }
        }

        Ok(ErrorRecoveryResult {
            fixed_code: current_code,
            retries_used: self.config.max_retries,
            errors_remaining: current_errors,
            success: false,
        })
    }

    pub async fn recover_with_verification<F, Fut>(
        &self,
        code: &str,
        compiler_output: &str,
        language: &str,
        verify_fn: F,
    ) -> Result<ErrorRecoveryResult>
    where
        F: Fn(String) -> Fut,
        Fut: std::future::Future<Output = String>,
    {
        let mut errors = parse_compiler_output(compiler_output);

        if errors.is_empty() {
            return Ok(ErrorRecoveryResult {
                fixed_code: code.to_string(),
                retries_used: 0,
                errors_remaining: Vec::new(),
                success: true,
            });
        }

        let mut current_code = code.to_string();

        for retry in 0..self.config.max_retries {
            match self.attempt_fix(&current_code, &errors, language).await {
                Ok(fixed_code) => {
                    let verification_output = verify_fn(fixed_code.clone()).await;
                    let new_errors = parse_compiler_output(&verification_output);

                    if new_errors.is_empty() {
                        return Ok(ErrorRecoveryResult {
                            fixed_code,
                            retries_used: retry + 1,
                            errors_remaining: Vec::new(),
                            success: true,
                        });
                    }

                    current_code = fixed_code;
                    errors = new_errors;
                },
                Err(e) => {
                    errors = vec![CompilationError::new(format!(
                        "LLM fix attempt failed: {e}"
                    ))];
                },
            }
        }

        Ok(ErrorRecoveryResult {
            fixed_code: current_code,
            retries_used: self.config.max_retries,
            errors_remaining: errors,
            success: false,
        })
    }
}

impl std::fmt::Debug for ErrorRecovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ErrorRecovery")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use tokio::sync::mpsc;

    struct MockFixLlm {
        fixed_code: String,
    }

    impl MockFixLlm {
        fn new(fixed_code: &str) -> Self {
            Self {
                fixed_code: fixed_code.to_string(),
            }
        }
    }

    #[async_trait]
    impl LlmClient for MockFixLlm {
        async fn chat(&self, _messages: Vec<ChatMessage>) -> Result<String> {
            Ok(self.fixed_code.clone())
        }

        async fn chat_stream(&self, _messages: Vec<ChatMessage>) -> Result<mpsc::Receiver<String>> {
            let (tx, rx) = mpsc::channel(1);
            let _ = tx.send("test".to_string()).await;
            Ok(rx)
        }

        fn count_tokens(&self, text: &str) -> usize {
            text.split_whitespace().count()
        }
    }

    #[test]
    fn test_default_config() {
        let config = ErrorRecoveryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert!(config.include_compiler_output);
        assert!(config.include_test_output);
    }

    #[test]
    fn test_config_builder() {
        let config = ErrorRecoveryConfig::new(5)
            .with_compiler_output(false)
            .with_test_output(false);
        assert_eq!(config.max_retries, 5);
        assert!(!config.include_compiler_output);
        assert!(!config.include_test_output);
    }

    #[test]
    fn test_parse_rust_errors() {
        let output = r#"error[E0425]: cannot find value `x` in this scope
   --> src/main.rs:10:5
    |
10  |     println!("{}", x);
    |                      ^ not found in this scope

error[E0308]: mismatched types
   --> src/main.rs:15:20
    |
15  |     let y: i32 = "hello";
    |                     ^^^^^ expected `i32`, found `&str`
"#;

        let errors = parse_compiler_output(output);
        assert_eq!(errors.len(), 2);

        assert_eq!(errors[0].error_code.as_deref(), Some("E0425"));
        assert!(errors[0].message.contains("cannot find value"));
        assert_eq!(errors[0].file_path.as_deref(), Some("src/main.rs"));
        assert_eq!(errors[0].line, Some(10));
        assert_eq!(errors[0].column, Some(5));

        assert_eq!(errors[1].error_code.as_deref(), Some("E0308"));
        assert!(errors[1].message.contains("mismatched types"));
        assert_eq!(errors[1].file_path.as_deref(), Some("src/main.rs"));
        assert_eq!(errors[1].line, Some(15));
    }

    #[test]
    fn test_parse_rust_error_no_code() {
        let output = r#"error: expected `;`, found `}`
   --> src/lib.rs:42:1
    |
42  | }
    | ^ expected `;`
"#;

        let errors = parse_compiler_output(output);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].error_code.is_none());
        assert!(errors[0].message.contains("expected"));
        assert_eq!(errors[0].file_path.as_deref(), Some("src/lib.rs"));
        assert_eq!(errors[0].line, Some(42));
    }

    #[test]
    fn test_parse_python_errors() {
        let output = r#"Traceback (most recent call last):
  File "script.py", line 42, in main
    undefined_var + 1
NameError: name 'undefined_var' is not defined
"#;

        let errors = parse_compiler_output(output);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].file_path.as_deref(), Some("script.py"));
        assert_eq!(errors[0].line, Some(42));
    }

    #[test]
    fn test_parse_typescript_errors() {
        let output = r#"src/index.ts(10,5): error TS2304: Cannot find name 'foo'.
src/index.ts(20,12): error TS2322: Type 'string' is not assignable to type 'number'.
"#;

        let errors = parse_compiler_output(output);
        assert_eq!(errors.len(), 2);

        assert_eq!(errors[0].error_code.as_deref(), Some("TS2304"));
        assert_eq!(errors[0].line, Some(10));
        assert_eq!(errors[0].column, Some(5));

        assert_eq!(errors[1].error_code.as_deref(), Some("TS2322"));
        assert_eq!(errors[1].line, Some(20));
        assert_eq!(errors[1].column, Some(12));
    }

    #[test]
    fn test_parse_generic_output() {
        let output = "Something went terribly wrong during compilation";
        let errors = parse_compiler_output(output);
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "Something went terribly wrong during compilation"
        );
    }

    #[test]
    fn test_parse_empty_output() {
        let errors = parse_compiler_output("");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_parse_whitespace_only_output() {
        let errors = parse_compiler_output("   \n\n  \t  ");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_build_fix_prompt_rust() {
        let errors = vec![CompilationError::new("cannot find value `x`".to_string())
            .with_error_code(Some("E0425".to_string()))
            .with_location(Some("src/main.rs".to_string()), Some(10), Some(5))];

        let prompt = build_fix_prompt("fn main() { x; }", &errors, "rust");
        assert!(prompt.contains("fn main() { x; }"));
        assert!(prompt.contains("E0425"));
        assert!(prompt.contains("src/main.rs:10:5"));
        assert!(prompt.contains("cannot find value"));
        assert!(prompt.contains("ONLY the complete fixed code"));
    }

    #[test]
    fn test_build_fix_prompt_python() {
        let errors = vec![CompilationError::new(
            "undefined_var not defined".to_string(),
        )];
        let prompt = build_fix_prompt("print(undefined_var)", &errors, "python");
        assert!(prompt.contains("indentation errors"));
    }

    #[test]
    fn test_build_fix_prompt_typescript() {
        let errors = vec![CompilationError::new("Cannot find name".to_string())];
        let prompt = build_fix_prompt("const x = foo;", &errors, "typescript");
        assert!(prompt.contains("type mismatches"));
    }

    #[test]
    fn test_strip_code_block() {
        let response = "Here is the fix:\n```rust\nfn main() {}\n```\nDone.";
        let stripped = strip_code_block(response);
        assert_eq!(stripped, "fn main() {}");
    }

    #[test]
    fn test_strip_code_block_no_fences() {
        let response = "fn main() {}";
        let stripped = strip_code_block(response);
        assert_eq!(stripped, "fn main() {}");
    }

    #[test]
    fn test_compilation_error_display() {
        let err = CompilationError::new("cannot find value".to_string())
            .with_error_code(Some("E0425".to_string()))
            .with_location(Some("src/main.rs".to_string()), Some(10), Some(5));
        let display = err.display();
        assert!(display.contains("[E0425]"));
        assert!(display.contains("src/main.rs:10:5"));
        assert!(display.contains("cannot find value"));
    }

    #[test]
    fn test_compilation_error_display_minimal() {
        let err = CompilationError::new("some error".to_string());
        let display = err.display();
        assert_eq!(display, "some error");
    }

    #[tokio::test]
    async fn test_recover_no_errors() {
        let llm = Arc::new(MockFixLlm::new("fn main() {}"));
        let recovery = ErrorRecovery::new(llm);

        let result = recovery.recover("fn main() {}", &[], "rust").await.unwrap();
        assert!(result.success);
        assert_eq!(result.retries_used, 0);
        assert_eq!(result.fixed_code, "fn main() {}");
    }

    #[tokio::test]
    async fn test_recover_with_fix() {
        let llm = Arc::new(MockFixLlm::new("fn main() { let x = 1; }"));
        let recovery = ErrorRecovery::new(llm);

        let errors = vec![CompilationError::new("cannot find value `x`".to_string())];
        let result = recovery
            .recover("fn main() { x; }", &errors, "rust")
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.retries_used, 1);
        assert_eq!(result.fixed_code, "fn main() { let x = 1; }");
    }

    #[tokio::test]
    async fn test_recover_max_retries() {
        struct FailingLlm;

        #[async_trait]
        impl LlmClient for FailingLlm {
            async fn chat(&self, _messages: Vec<ChatMessage>) -> Result<String> {
                Err(crate::error::Error::Llm("rate limited".to_string()))
            }

            async fn chat_stream(
                &self,
                _messages: Vec<ChatMessage>,
            ) -> Result<mpsc::Receiver<String>> {
                let (tx, rx) = mpsc::channel(1);
                let _ = tx.send("test".to_string()).await;
                Ok(rx)
            }

            fn count_tokens(&self, text: &str) -> usize {
                text.split_whitespace().count()
            }
        }

        let llm = Arc::new(FailingLlm);
        let config = ErrorRecoveryConfig::new(2);
        let recovery = ErrorRecovery::with_config(llm, config);

        let errors = vec![CompilationError::new("some error".to_string())];
        let result = recovery.recover("bad code", &errors, "rust").await.unwrap();
        assert!(!result.success);
        assert_eq!(result.retries_used, 2);
        assert!(!result.errors_remaining.is_empty());
    }

    #[tokio::test]
    async fn test_recover_with_verification() {
        let llm = Arc::new(MockFixLlm::new("fn main() { let x: i32 = 1; }"));
        let recovery = ErrorRecovery::new(llm);

        let compiler_output =
            "error[E0425]: cannot find value `x` in this scope\n--> src/main.rs:1:10";
        let result = recovery
            .recover_with_verification("fn main() { x; }", compiler_output, "rust", |_code| async {
                String::new()
            })
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.retries_used, 1);
    }

    #[tokio::test]
    async fn test_recover_with_verification_still_failing() {
        let llm = Arc::new(MockFixLlm::new("fn main() { let y: i32 = 1; }"));
        let config = ErrorRecoveryConfig::new(2);
        let recovery = ErrorRecovery::with_config(llm, config);

        let compiler_output =
            "error[E0425]: cannot find value `x` in this scope\n--> src/main.rs:1:10";
        let result = recovery
            .recover_with_verification("fn main() { x; }", compiler_output, "rust", |_code| async {
                "error[E0425]: cannot find value `y` in this scope\n--> src/main.rs:1:10"
                    .to_string()
            })
            .await
            .unwrap();
        assert!(!result.success);
        assert_eq!(result.retries_used, 2);
        assert!(!result.errors_remaining.is_empty());
    }
}
