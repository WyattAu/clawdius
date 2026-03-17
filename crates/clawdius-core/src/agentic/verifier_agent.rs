//! Verifier Agent
//!
//! Verifies the generated code and results.

use crate::error::Result;
use serde::{Deserialize, Serialize};

/// Agent responsible for verifying generated code.
#[derive(Debug, Default)]
pub struct VerifierAgent {
    /// Verification rules
    rules: Vec<VerificationRule>,
}

impl VerifierAgent {
    /// Creates a new verifier agent.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rules: Self::default_rules(),
        }
    }

    /// Creates a verifier with custom rules.
    #[must_use]
    pub fn with_rules(rules: Vec<VerificationRule>) -> Self {
        Self { rules }
    }

    fn default_rules() -> Vec<VerificationRule> {
        vec![
            VerificationRule {
                id: "syntax".to_string(),
                name: "Syntax Check".to_string(),
                severity: IssueSeverity::Blocking,
                enabled: true,
            },
            VerificationRule {
                id: "tests".to_string(),
                name: "Test Pass".to_string(),
                severity: IssueSeverity::Blocking,
                enabled: true,
            },
            VerificationRule {
                id: "lint".to_string(),
                name: "Lint Check".to_string(),
                severity: IssueSeverity::Warning,
                enabled: true,
            },
            VerificationRule {
                id: "types".to_string(),
                name: "Type Check".to_string(),
                severity: IssueSeverity::Blocking,
                enabled: true,
            },
            VerificationRule {
                id: "security".to_string(),
                name: "Security Scan".to_string(),
                severity: IssueSeverity::Warning,
                enabled: true,
            },
        ]
    }

    /// Verifies the changes against the task request.
    pub async fn verify_changes(
        &self,
        changes: &[super::FileChange],
        request: &super::TaskRequest,
    ) -> Result<VerificationResult> {
        let mut issues = Vec::new();

        // Run each enabled rule
        for rule in &self.rules {
            if !rule.enabled {
                continue;
            }

            let rule_issues = self.run_rule(rule, changes, request).await?;
            issues.extend(rule_issues);
        }

        // Calculate overall status
        let passed = !issues.iter().any(|i| i.is_blocking());
        let warnings = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Warning)
            .count();
        let errors = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Blocking)
            .count();

        Ok(VerificationResult {
            passed,
            issues,
            warnings_count: warnings,
            errors_count: errors,
            verified_files: changes.iter().map(|c| c.path.clone()).collect(),
        })
    }

    async fn run_rule(
        &self,
        rule: &VerificationRule,
        changes: &[super::FileChange],
        _request: &super::TaskRequest,
    ) -> Result<Vec<VerificationIssue>> {
        let issues = match rule.id.as_str() {
            "syntax" => self.check_syntax(changes).await?,
            "tests" => self.check_tests(changes).await?,
            "lint" => self.check_lint(changes).await?,
            "types" => self.check_types(changes).await?,
            "security" => self.check_security(changes).await?,
            _ => Vec::new(),
        };

        Ok(issues)
    }

    async fn check_syntax(&self, changes: &[super::FileChange]) -> Result<Vec<VerificationIssue>> {
        let mut issues = Vec::new();

        for change in changes {
            let content = &change.new;
            // Basic syntax checks
            if content.contains("TODO") || content.contains("FIXME") {
                issues.push(VerificationIssue {
                    id: format!("syntax-todo-{}", change.path),
                    severity: IssueSeverity::Warning,
                    message: "Code contains TODO/FIXME comment".to_string(),
                    file: change.path.clone(),
                    line: None,
                    column: None,
                    rule: "syntax".to_string(),
                    can_fix: false,
                    fix_suggestion: None,
                });
            }

            // Check for unmatched braces (basic)
            let open_braces = content.matches('{').count();
            let close_braces = content.matches('}').count();
            if open_braces != close_braces {
                issues.push(VerificationIssue {
                    id: format!("syntax-braces-{}", change.path),
                    severity: IssueSeverity::Blocking,
                    message: format!(
                        "Unmatched braces: {} open, {} close",
                        open_braces, close_braces
                    ),
                    file: change.path.clone(),
                    line: None,
                    column: None,
                    rule: "syntax".to_string(),
                    can_fix: false,
                    fix_suggestion: None,
                });
            }
        }

        Ok(issues)
    }

    async fn check_tests(&self, changes: &[super::FileChange]) -> Result<Vec<VerificationIssue>> {
        // In a real implementation, this would run the test suite
        let _ = changes;
        Ok(Vec::new())
    }

    async fn check_lint(&self, changes: &[super::FileChange]) -> Result<Vec<VerificationIssue>> {
        let mut issues = Vec::new();

        for change in changes {
            let content = &change.new;
            // Basic lint checks
            if content.contains("unwrap()") {
                issues.push(VerificationIssue {
                    id: format!("lint-unwrap-{}", change.path),
                    severity: IssueSeverity::Warning,
                    message: "Use of unwrap() detected - consider proper error handling"
                        .to_string(),
                    file: change.path.clone(),
                    line: None,
                    column: None,
                    rule: "lint".to_string(),
                    can_fix: true,
                    fix_suggestion: Some(
                        "Consider using expect() or proper error handling".to_string(),
                    ),
                });
            }
        }

        Ok(issues)
    }

    async fn check_types(&self, changes: &[super::FileChange]) -> Result<Vec<VerificationIssue>> {
        // In a real implementation, this would run type checking
        let _ = changes;
        Ok(Vec::new())
    }

    async fn check_security(
        &self,
        changes: &[super::FileChange],
    ) -> Result<Vec<VerificationIssue>> {
        let mut issues = Vec::new();

        for change in changes {
            let content = &change.new;
            // Basic security checks
            let dangerous_patterns = [
                ("password", "Potential hardcoded password"),
                ("api_key", "Potential hardcoded API key"),
                ("secret", "Potential hardcoded secret"),
                ("private_key", "Potential hardcoded private key"),
            ];

            for (pattern, message) in dangerous_patterns {
                if content.to_lowercase().contains(pattern) {
                    issues.push(VerificationIssue {
                        id: format!("security-{}-{}", pattern, change.path),
                        severity: IssueSeverity::Warning,
                        message: message.to_string(),
                        file: change.path.clone(),
                        line: None,
                        column: None,
                        rule: "security".to_string(),
                        can_fix: true,
                        fix_suggestion: Some(
                            "Use environment variables or secure storage".to_string(),
                        ),
                    });
                }
            }
        }

        Ok(issues)
    }

    /// Adds a custom verification rule.
    pub fn add_rule(&mut self, rule: VerificationRule) {
        self.rules.push(rule);
    }

    /// Enables or disables a rule by ID.
    pub fn set_rule_enabled(&mut self, rule_id: &str, enabled: bool) {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.id == rule_id) {
            rule.enabled = enabled;
        }
    }

    /// Returns all rules.
    #[must_use]
    pub fn get_rules(&self) -> &[VerificationRule] {
        &self.rules
    }
}

impl Default for VerificationResult {
    fn default() -> Self {
        Self {
            passed: true,
            issues: Vec::new(),
            warnings_count: 0,
            errors_count: 0,
            verified_files: Vec::new(),
        }
    }
}

/// Result of verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether verification passed (no blocking issues)
    pub passed: bool,
    /// All issues found
    pub issues: Vec<VerificationIssue>,
    /// Number of warnings
    pub warnings_count: usize,
    /// Number of errors
    pub errors_count: usize,
    /// Files that were verified
    pub verified_files: Vec<String>,
}

impl VerificationResult {
    /// Creates a successful verification result.
    #[must_use]
    pub fn success() -> Self {
        Self {
            passed: true,
            issues: Vec::new(),
            warnings_count: 0,
            errors_count: 0,
            verified_files: Vec::new(),
        }
    }

    /// Returns blocking issues.
    #[must_use]
    pub fn blocking_issues(&self) -> Vec<&VerificationIssue> {
        self.issues.iter().filter(|i| i.is_blocking()).collect()
    }

    /// Returns fixable issues.
    #[must_use]
    pub fn fixable_issues(&self) -> Vec<&VerificationIssue> {
        self.issues.iter().filter(|i| i.can_fix).collect()
    }
}

/// A single verification issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationIssue {
    /// Unique issue ID
    pub id: String,
    /// Issue severity
    pub severity: IssueSeverity,
    /// Human-readable message
    pub message: String,
    /// File where the issue was found
    pub file: String,
    /// Line number (if known)
    pub line: Option<u32>,
    /// Column number (if known)
    pub column: Option<u32>,
    /// Rule that detected this issue
    pub rule: String,
    /// Whether this issue can be auto-fixed
    pub can_fix: bool,
    /// Suggested fix
    pub fix_suggestion: Option<String>,
}

impl VerificationIssue {
    /// Returns true if this is a blocking issue.
    #[must_use]
    pub const fn is_blocking(&self) -> bool {
        matches!(
            self.severity,
            IssueSeverity::Blocking | IssueSeverity::Critical
        )
    }

    /// Creates a blocking issue.
    #[must_use]
    pub fn blocking(id: String, message: String, file: String) -> Self {
        Self {
            id,
            severity: IssueSeverity::Blocking,
            message,
            file,
            line: None,
            column: None,
            rule: String::new(),
            can_fix: false,
            fix_suggestion: None,
        }
    }

    /// Creates a warning issue.
    #[must_use]
    pub fn warning(id: String, message: String, file: String) -> Self {
        Self {
            id,
            severity: IssueSeverity::Warning,
            message,
            file,
            line: None,
            column: None,
            rule: String::new(),
            can_fix: false,
            fix_suggestion: None,
        }
    }
}

/// Severity of a verification issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    /// Informational only
    Info,
    /// Warning - non-blocking
    Warning,
    /// Blocking - must be fixed
    Blocking,
    /// Critical - stops all execution
    Critical,
}

impl IssueSeverity {
    /// Returns the priority for this severity (higher = more urgent).
    #[must_use]
    pub const fn priority(&self) -> u32 {
        match self {
            Self::Info => 1,
            Self::Warning => 2,
            Self::Blocking => 3,
            Self::Critical => 4,
        }
    }
}

/// A verification rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationRule {
    /// Unique rule ID
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Default severity
    pub severity: IssueSeverity,
    /// Whether the rule is enabled
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verifier_creation() {
        let verifier = VerifierAgent::new();
        assert!(!verifier.rules.is_empty());
    }

    #[tokio::test]
    async fn test_verify_empty_changes() {
        let verifier = VerifierAgent::new();
        let request = super::super::TaskRequest {
            id: "test".to_string(),
            description: "test".to_string(),
            target_files: vec![],
            mode: super::super::GenerationMode::single_pass(),
            test_strategy: super::super::TestExecutionStrategy::skip(),
            apply_workflow: super::super::ApplyWorkflow::preview_only(),
            context: super::super::TaskContext::default(),
            trust_level: super::super::TrustLevel::medium(),
        };

        let result = verifier.verify_changes(&[], &request).await.unwrap();
        assert!(result.passed);
    }

    #[tokio::test]
    async fn test_verify_with_unmatched_braces() {
        let verifier = VerifierAgent::new();
        let changes = vec![super::super::FileChange {
            path: "test.rs".to_string(),
            change_type: super::super::ChangeType::Modified,
            original: None,
            new: "fn main() { let x = 1; ".to_string(),
            diff: String::new(),
        }];
        let request = super::super::TaskRequest {
            id: "test".to_string(),
            description: "test".to_string(),
            target_files: vec![],
            mode: super::super::GenerationMode::single_pass(),
            test_strategy: super::super::TestExecutionStrategy::skip(),
            apply_workflow: super::super::ApplyWorkflow::preview_only(),
            context: super::super::TaskContext::default(),
            trust_level: super::super::TrustLevel::medium(),
        };

        let result = verifier.verify_changes(&changes, &request).await.unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn test_verification_issue_blocking() {
        let issue = VerificationIssue::blocking(
            "test".to_string(),
            "Error".to_string(),
            "file.rs".to_string(),
        );
        assert!(issue.is_blocking());
    }

    #[test]
    fn test_verification_issue_warning() {
        let issue = VerificationIssue::warning(
            "test".to_string(),
            "Warning".to_string(),
            "file.rs".to_string(),
        );
        assert!(!issue.is_blocking());
    }
}
