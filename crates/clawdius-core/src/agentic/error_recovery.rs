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
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            file_path: None,
            line: None,
            column: None,
            message: message.into(),
            error_code: None,
        }
    }

    #[must_use]
    pub fn with_file(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    #[must_use]
    pub fn with_location(mut self, line: u32, column: u32) -> Self {
        self.line = Some(line);
        self.column = Some(column);
        self
    }

    /// Full location setter (file, line, column) for backward compatibility.
    #[must_use]
    pub fn with_full_location(
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
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.error_code = Some(code.into());
        self
    }

    /// Alias for `with_code` — accepts `Option<String>` for backward compatibility.
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

    /// Categorize this error based on its message and code.
    pub fn categorize(&self, language: &str) -> ErrorCategory {
        ErrorCategory::classify(&self.message, self.error_code.as_deref(), language)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCategory {
    /// Missing import, use statement, or module declaration.
    MissingImport,
    /// Type mismatch, incompatible types, wrong annotations.
    TypeMismatch,
    /// Undefined variable, function, or identifier.
    Undefined,
    /// Syntax error, malformed code structure.
    Syntax,
    /// Lifetime error (Rust-specific).
    Lifetime,
    /// Borrow checker error (Rust-specific).
    BorrowChecker,
    /// Trait bound not satisfied, missing implementation.
    TraitBound,
    /// Macro error, expansion failure.
    Macro,
    /// Unused variable/import/function (warning-level).
    Unused,
    /// Feature gate or attribute error.
    Feature,
    /// Module resolution or path error.
    ModuleResolution,
    /// Configuration or build system error.
    Configuration,
    /// Any other error not matching known patterns.
    Other,
}

impl ErrorCategory {
    /// Categorize a compilation error based on its message and error code.
    pub fn classify(message: &str, error_code: Option<&str>, language: &str) -> Self {
        let msg_lower = message.to_lowercase();
        let code_upper = error_code.map(|c| c.to_uppercase());

        // Rust-specific patterns
        if language == "rust" {
            // Missing import
            if msg_lower.contains("cannot find value")
                || msg_lower.contains("unresolved import")
                || msg_lower.contains("use of undeclared type")
                || code_upper.as_deref() == Some("E0432")
                || code_upper.as_deref() == Some("E0531")
                || code_upper.as_deref() == Some("E0433")
            {
                return Self::MissingImport;
            }
            // Type mismatch
            if msg_lower.contains("mismatched types")
                || msg_lower.contains("expected")
                || msg_lower.contains("found `")
                || code_upper.as_deref() == Some("E0308")
            {
                return Self::TypeMismatch;
            }
            // Undefined
            if msg_lower.contains("cannot find")
                || msg_lower.contains("not found in this scope")
                || code_upper.as_deref() == Some("E0425")
            {
                return Self::Undefined;
            }
            // Lifetime
            if msg_lower.contains("lifetime")
                || code_upper.as_deref() == Some("E0597")
                || code_upper.as_deref() == Some("E0621")
                || code_upper.as_deref() == Some("E0106")
            {
                return Self::Lifetime;
            }
            // Borrow checker
            if msg_lower.contains("cannot borrow")
                || msg_lower.contains("cannot move")
                || code_upper.as_deref() == Some("E0382")
                || code_upper.as_deref() == Some("E0596")
            {
                return Self::BorrowChecker;
            }
            // Trait bound
            if msg_lower.contains("trait bound")
                || msg_lower.contains("the following trait is not implemented")
                || msg_lower.contains("not satisfied")
                || code_upper.as_deref() == Some("E0277")
            {
                return Self::TraitBound;
            }
            // Macro
            if msg_lower.contains("macro")
                || code_upper.as_deref() == Some("E0425") // some macro expansion errors
            {
                return Self::Macro;
            }
            // Unused
            if msg_lower.contains("unused")
                || code_upper.as_deref() == Some("E0277")
                || code_upper.as_deref() == Some("E0451")
            {
                return Self::Unused;
            }
            // Feature
            if msg_lower.contains("feature")
                || code_upper.as_deref() == Some("E0554")
            {
                return Self::Feature;
            }
        }

        // TypeScript/JavaScript patterns
        if language == "typescript" || language == "javascript" {
            if msg_lower.contains("cannot find module")
                || msg_lower.contains("has no exported member")
                || msg_lower.contains("is not defined")
                || error_code.map_or(false, |c| c.starts_with("TS23"))
            {
                return Self::MissingImport;
            }
            if msg_lower.contains("type")
                && (msg_lower.contains("is not assignable")
                    || msg_lower.contains("is not compatible")
                    || error_code.map_or(false, |c| c.starts_with("TS23")))
            {
                return Self::TypeMismatch;
            }
            if msg_lower.contains("is not defined")
                || error_code.map_or(false, |c| c.starts_with("TS23"))
            {
                return Self::Undefined;
            }
        }

        // Python patterns
        if language == "python" {
            if msg_lower.contains("name ")
                && (msg_lower.contains("is not defined")
                    || msg_lower.contains("is not built-in"))
            {
                return Self::Undefined;
            }
            if msg_lower.contains("no module named")
                || msg_lower.contains("importerror")
                || msg_lower.contains("modulenotfounderror")
            {
                return Self::MissingImport;
            }
            if msg_lower.contains("typeerror")
                || msg_lower.contains("unsupported operand")
            {
                return Self::TypeMismatch;
            }
            if msg_lower.contains("indentationerror")
                || msg_lower.contains("unexpected indent")
            {
                return Self::Syntax;
            }
        }

        // Go patterns
        if language == "go" {
            if msg_lower.contains("undefined:")
                || msg_lower.contains("undeclared name")
            {
                return Self::Undefined;
            }
            if msg_lower.contains("cannot use")
                || msg_lower.contains("invalid operation")
            {
                return Self::TypeMismatch;
            }
            if msg_lower.contains("imported but not used")
                || msg_lower.contains("undefined:")
            {
                return Self::MissingImport;
            }
            if msg_lower.contains("syntax error")
                || msg_lower.contains("expected")
            {
                return Self::Syntax;
            }
        }

        Self::Other
    }

    /// Get a strategy hint for this error category.
    pub fn strategy_hint(&self) -> &'static str {
        match self {
            Self::MissingImport => "Add the missing import statement at the top of the file. \
                Check the correct module path and import style for the language.",
            Self::TypeMismatch => "Check the types on both sides of the expression. \
                Add explicit type annotations if needed. Verify generic type arguments \
                and trait object safety.",
            Self::Undefined => "Verify the identifier is in scope. Check for typos, \
                missing imports, and correct scoping. Ensure the item is declared \
                before it is used.",
            Self::Syntax => "Fix the syntax error. Check for missing brackets, \
                incorrect punctuation, and malformed expressions.",
            Self::Lifetime => "Check ownership and borrowing. Add explicit lifetimes where \
                needed. Use references instead of moves where possible. Consider \
                cloning or using `Cow` for owned data.",
            Self::BorrowChecker => "Fix the ownership/borrowing conflict. Consider using \
                references, cloning, restructuring the code, or using interior mutability \
                patterns (`RefCell`, `Rc`, `Arc`).",
            Self::TraitBound => "Implement the required trait for the type, or add \
                trait bounds to generic parameters. Consider using a blanket implementation \
                or a wrapper type.",
            Self::Macro => "Fix the macro invocation. Check required arguments, \
                syntax, and delimiter handling. Verify the macro is imported.",
            Self::Unused => "Remove the unused item or prefix with an underscore \
                to suppress the warning.",
            Self::Feature => "Enable the required feature gate or use an alternative \
                approach that doesn't require the unstable feature.",
            Self::ModuleResolution => "Fix the module path. Check for correct \
                casing and directory structure. Verify the module is listed in \
                Cargo.toml / go.mod / package.json.",
            Self::Configuration => "Fix the build configuration. Check Cargo.toml \
                dependencies, compiler flags, and build settings.",
            Self::Other => "Analyze the error message carefully and apply the \
                appropriate fix.",
        }
    }

    /// Human-readable category name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::MissingImport => "Missing Import",
            Self::TypeMismatch => "Type Mismatch",
            Self::Undefined => "Undefined Identifier",
            Self::Syntax => "Syntax Error",
            Self::Lifetime => "Lifetime Error",
            Self::BorrowChecker => "Borrow Checker Error",
            Self::TraitBound => "Trait Bound Not Satisfied",
            Self::Macro => "Macro Error",
            Self::Unused => "Unused Item",
            Self::Feature => "Feature Gate Error",
            Self::ModuleResolution => "Module Resolution Error",
            Self::Configuration => "Configuration Error",
            Self::Other => "Other Error",
        }
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

    let go_re = Regex::new(r"(.*?:(\d+)(?::\d+)?)\s*(.+?)(?:\n|$)")
        .expect("go error regex must compile");

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
                caps.get(5)
                    .map_or(String::new(), |m| m.as_str().to_string()),
            );
            err.file_path = caps.get(1).map(|m| m.as_str().to_string());
            err.line = caps.get(2).and_then(|m| m.as_str().parse().ok());
            err.column = caps.get(3).and_then(|m| m.as_str().parse().ok());
            // TS error code is capture group 4
            err.error_code = caps.get(4).map(|m| m.as_str().to_string());
            errors.push(err);
        }
    } else if go_re.is_match(output) {
        for caps in go_re.captures_iter(output) {
            let mut err = CompilationError::new(
                caps.get(3)
                    .map_or(String::new(), |m| m.as_str().to_string()),
            );
            err.file_path = caps.get(1).map(|m| m.as_str().to_string());
            err.line = caps.get(2).and_then(|m| m.as_str().parse().ok());
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

/// Build a targeted fix prompt based on the error category.
/// This produces much better results than the generic "fix all errors" approach.
pub fn build_targeted_fix_prompt(
    original_code: &str,
    errors: &[CompilationError],
    language: &str,
) -> String {
    let mut error_list = String::new();
    for (i, err) in errors.iter().enumerate() {
        let category = err.categorize(language);
        error_list.push_str(&format!(
            "{}. [{}] {}\n",
            i + 1,
            category.display_name(),
            err.display()
        ));
    }

    let errors_by_file: std::collections::HashMap<&str, Vec<&CompilationError>> =
        errors.iter().fold(std::collections::HashMap::new(), |mut map, err| {
            map.entry(err.file_path.as_deref().unwrap_or("unknown"))
                .or_insert_with(|| Vec::new())
                .push(err);
            map
        });

    let files_note = if errors_by_file.len() > 1 {
        format!(
            "\nNote: Errors span {} files. Focus on fixing the errors in the provided file.\n",
            errors_by_file.len()
        )
    } else {
        String::new()
    };

    format!(
        "The following {lang} code has compilation errors. Fix ALL errors listed below.\n\n\
         ## Errors\n\
         {error_list}{files_note}\
         ## Original Code\n\
         ```{lang}\n{original_code}\n```\n\n\
         ## Instructions\n\
         Respond with ONLY the complete fixed code. No explanations, no markdown \
         fences, no commentary. Output just the corrected code ready to compile.\n",
        lang = language,
        error_list = error_list,
        files_note = files_note,
        original_code = original_code,
    )
}

/// Group errors by file path for targeted multi-file recovery.
pub fn group_errors_by_file(errors: &[CompilationError]) -> Vec<(&str, Vec<&CompilationError>)> {
    let mut map: std::collections::HashMap<&str, Vec<&CompilationError>> =
        std::collections::HashMap::new();
    for err in errors {
        map.entry(err.file_path.as_deref().unwrap_or("unknown"))
            .or_insert_with(Vec::new)
            .push(err);
    }
    map.into_iter().collect()
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
            .with_full_location(Some("src/main.rs".to_string()), Some(10), Some(5))];

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
            .with_full_location(Some("src/main.rs".to_string()), Some(10), Some(5));
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

    // ==================== ErrorCategory tests ====================

    #[test]
    fn test_categorize_rust_missing_import() {
        let err = CompilationError::new("cannot find value `x` in this scope")
            .with_code("E0425");
        // "cannot find value" matches MissingImport first
        assert_eq!(err.categorize("rust"), ErrorCategory::MissingImport);
    }

    #[test]
    fn test_categorize_rust_type_mismatch() {
        let err = CompilationError::new("mismatched types")
            .with_code("E0308");
        assert_eq!(err.categorize("rust"), ErrorCategory::TypeMismatch);
    }

    #[test]
    fn test_categorize_rust_borrow_checker() {
        let err = CompilationError::new("cannot borrow `x` as mutable")
            .with_code("E0382");
        assert_eq!(err.categorize("rust"), ErrorCategory::BorrowChecker);
    }

    #[test]
    fn test_categorize_rust_lifetime() {
        let err = CompilationError::new("`x` does not live long enough")
            .with_code("E0597");
        assert_eq!(err.categorize("rust"), ErrorCategory::Lifetime);
    }

    #[test]
    fn test_categorize_rust_trait_bound() {
        let err = CompilationError::new("the following trait is not implemented")
            .with_code("E0277");
        assert_eq!(err.categorize("rust"), ErrorCategory::TraitBound);
    }

    #[test]
    fn test_categorize_python_missing_import() {
        let err = CompilationError::new("No module named 'requests'");
        assert_eq!(err.categorize("python"), ErrorCategory::MissingImport);
    }

    #[test]
    fn test_categorize_python_undefined() {
        let err = CompilationError::new("name 'foo' is not defined");
        assert_eq!(err.categorize("python"), ErrorCategory::Undefined);
    }

    #[test]
    fn test_categorize_python_syntax() {
        let err = CompilationError::new("unexpected indent");
        assert_eq!(err.categorize("python"), ErrorCategory::Syntax);
    }

    #[test]
    fn test_categorize_go_undefined() {
        let err = CompilationError::new("undefined: foo");
        assert_eq!(err.categorize("go"), ErrorCategory::Undefined);
    }

    #[test]
    fn test_categorize_go_type_mismatch() {
        let err = CompilationError::new("cannot use x (type int) as type string");
        assert_eq!(err.categorize("go"), ErrorCategory::TypeMismatch);
    }

    #[test]
    fn test_categorize_unknown_language() {
        let err = CompilationError::new("some random error");
        assert_eq!(err.categorize("brainfuck"), ErrorCategory::Other);
    }

    #[test]
    fn test_error_category_display_name() {
        assert_eq!(ErrorCategory::MissingImport.display_name(), "Missing Import");
        assert_eq!(ErrorCategory::TypeMismatch.display_name(), "Type Mismatch");
        assert_eq!(ErrorCategory::BorrowChecker.display_name(), "Borrow Checker Error");
        assert_eq!(ErrorCategory::Other.display_name(), "Other Error");
    }

    #[test]
    fn test_error_category_strategy_hint() {
        let hint = ErrorCategory::BorrowChecker.strategy_hint();
        assert!(hint.contains("ownership"));
        assert!(hint.contains("RefCell"));

        let hint = ErrorCategory::Lifetime.strategy_hint();
        assert!(hint.contains("lifetime"));
        assert!(hint.contains("Cow"));
    }

    // ==================== Targeted prompt tests ====================

    #[test]
    fn test_build_targeted_fix_prompt() {
        let errors = vec![
            CompilationError::new("cannot find value `x` in this scope")
                .with_full_location(Some("src/main.rs".to_string()), Some(10), Some(5))
                .with_error_code(Some("E0425".to_string())),
        ];
        let prompt = build_targeted_fix_prompt("fn main() { x; }", &errors, "rust");
        assert!(prompt.contains("Missing Import"));
        assert!(prompt.contains("[E0425]"));
        assert!(prompt.contains("src/main.rs"));
        assert!(prompt.contains("fn main() { x; }"));
    }

    #[test]
    fn test_build_targeted_fix_prompt_multi_error() {
        let errors = vec![
            CompilationError::new("cannot find value `x` in this scope"),
            CompilationError::new("mismatched types"),
        ];
        let prompt = build_targeted_fix_prompt("fn main() {}", &errors, "rust");
        assert!(prompt.contains("1. ["));
        assert!(prompt.contains("2. ["));
    }

    #[test]
    fn test_group_errors_by_file() {
        let errors = vec![
            CompilationError::new("error1").with_file("a.rs"),
            CompilationError::new("error2").with_file("b.rs"),
            CompilationError::new("error3").with_file("a.rs"),
        ];
        let groups = group_errors_by_file(&errors);
        assert_eq!(groups.len(), 2);
        // Sort for deterministic assertion
        let mut groups = groups;
        groups.sort_by_key(|(f, _)| f.to_string());
        assert_eq!(groups[0].0, "a.rs");
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].0, "b.rs");
        assert_eq!(groups[1].1.len(), 1);
    }

    #[test]
    fn test_parse_go_errors() {
        let output = "main.go:10:5: undefined: foo\nmain.go:15:2: cannot use x (type int) as type string";
        let errors = parse_compiler_output(output);
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].file_path.as_deref(), Some("main.go:10:5"));
        assert_eq!(errors[0].line, Some(10));
        assert!(errors[0].message.contains("undefined"));
    }

    #[test]
    fn test_parse_go_errors_whitespace() {
        let errors = parse_compiler_output("   \n\n  \t  ");
        assert!(errors.is_empty());
    }
}
