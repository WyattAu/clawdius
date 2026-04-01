//! Proof verification types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// A Lean proof definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofDefinition {
    /// Name of the proof/theorem
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Dependencies (other proofs/modules required)
    pub dependencies: Vec<String>,
    /// The theorem statement
    pub theorem: String,
    /// The proof body
    pub proof: String,
}

impl ProofDefinition {
    /// Create a new proof definition
    pub fn new(name: impl Into<String>, theorem: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            dependencies: Vec::new(),
            theorem: theorem.into(),
            proof: String::new(),
        }
    }

    /// Add a description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Add a dependency
    pub fn with_dependency(mut self, dep: impl Into<String>) -> Self {
        self.dependencies.push(dep.into());
        self
    }

    /// Set the proof body
    pub fn with_proof(mut self, proof: impl Into<String>) -> Self {
        self.proof = proof.into();
        self
    }

    /// Render to Lean 4 source code
    #[must_use]
    pub fn to_lean_source(&self) -> String {
        let mut source = String::new();

        for dep in &self.dependencies {
            source.push_str(&format!("import {dep}\n"));
        }

        if !self.dependencies.is_empty() {
            source.push('\n');
        }

        if !self.description.is_empty() {
            source.push_str(&format!("/-- {} -/\n", self.description));
        }

        source.push_str(&self.theorem);
        source.push_str(" := by\n");
        source.push_str(&self.proof);
        source.push('\n');

        source
    }
}

/// A proof template with placeholders
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofTemplate {
    /// Template name
    pub name: String,
    /// Template description
    pub description: String,
    /// The template string with {placeholder} syntax
    pub template: String,
    /// List of placeholder names
    pub placeholders: Vec<String>,
}

impl ProofTemplate {
    /// Create a new proof template
    pub fn new(name: impl Into<String>, template: impl Into<String>) -> Self {
        let template_str = template.into();
        let placeholders = extract_placeholders(&template_str);

        Self {
            name: name.into(),
            description: String::new(),
            template: template_str,
            placeholders,
        }
    }

    /// Add a description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Render the template with provided values
    pub fn render(&self, values: &HashMap<String, String>) -> Result<String, TemplateError> {
        let mut result = self.template.clone();

        for placeholder in &self.placeholders {
            let value = values
                .get(placeholder)
                .ok_or_else(|| TemplateError::MissingPlaceholder(placeholder.clone()))?;
            result = result.replace(&format!("{{{placeholder}}}"), value);
        }

        Ok(result)
    }
}

/// Template rendering error
#[derive(Debug, Clone, thiserror::Error)]
pub enum TemplateError {
    /// Missing placeholder value
    #[error("Missing placeholder: {0}")]
    MissingPlaceholder(String),
}

fn extract_placeholders(template: &str) -> Vec<String> {
    let mut placeholders = Vec::new();
    let mut in_placeholder = false;
    let mut current = String::new();

    for ch in template.chars() {
        match ch {
            '{' => {
                in_placeholder = true;
                current.clear();
            },
            '}' if in_placeholder => {
                in_placeholder = false;
                if !current.is_empty() && !placeholders.contains(&current) {
                    placeholders.push(current.clone());
                }
            },
            _ if in_placeholder => {
                current.push(ch);
            },
            _ => {},
        }
    }

    placeholders
}

/// A Lean compilation/verification error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeanError {
    /// Line number (1-indexed)
    pub line: usize,
    /// Column number (1-indexed)
    pub column: usize,
    /// End line (for multi-line errors)
    pub end_line: Option<usize>,
    /// End column
    pub end_column: Option<usize>,
    /// Error message
    pub message: String,
    /// Error severity
    pub severity: LeanErrorSeverity,
}

impl LeanError {
    /// Create a new error
    pub fn new(line: usize, column: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            column,
            end_line: None,
            end_column: None,
            message: message.into(),
            severity: LeanErrorSeverity::Error,
        }
    }

    /// Create from a message only (unknown position)
    pub fn from_message(message: impl Into<String>) -> Self {
        Self {
            line: 0,
            column: 0,
            end_line: None,
            end_column: None,
            message: message.into(),
            severity: LeanErrorSeverity::Error,
        }
    }
}

/// Error severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeanErrorSeverity {
    /// Informational message
    Info,
    /// Warning
    Warning,
    /// Error
    Error,
}

/// Result of proof verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether verification succeeded
    pub success: bool,
    /// List of errors found
    pub errors: Vec<LeanError>,
    /// List of warnings
    pub warnings: Vec<String>,
    /// Time taken for verification
    pub duration: Duration,
    /// Output from Lean (for debugging)
    pub output: String,
}

impl VerificationResult {
    /// Create a successful result
    #[must_use]
    pub fn success(duration: Duration) -> Self {
        Self {
            success: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            duration,
            output: String::new(),
        }
    }

    /// Create a failed result
    #[must_use]
    pub fn failure(errors: Vec<LeanError>, duration: Duration) -> Self {
        Self {
            success: false,
            errors,
            warnings: Vec::new(),
            duration,
            output: String::new(),
        }
    }

    /// Check if there are any errors
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Check if there are any warnings
    #[must_use]
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proof_definition_to_source() {
        let proof = ProofDefinition::new("my_theorem", "theorem foo : True")
            .with_description("A test theorem")
            .with_dependency("Mathlib.Data.Nat.Basic")
            .with_proof("exact trivial");

        let source = proof.to_lean_source();
        assert!(source.contains("import Mathlib.Data.Nat.Basic"));
        assert!(source.contains("theorem foo : True"));
        assert!(source.contains("exact trivial"));
    }

    #[test]
    fn test_template_render() {
        let template = ProofTemplate::new("test", "theorem {name} : {prop} := by\n  {proof}");

        let mut values = HashMap::new();
        values.insert("name".to_string(), "foo".to_string());
        values.insert("prop".to_string(), "True".to_string());
        values.insert("proof".to_string(), "trivial".to_string());

        let result = template.render(&values).unwrap();
        assert_eq!(result, "theorem foo : True := by\n  trivial");
    }

    #[test]
    fn test_template_missing_placeholder() {
        let template = ProofTemplate::new("test", "theorem {name} : {prop}");

        let mut values = HashMap::new();
        values.insert("name".to_string(), "foo".to_string());

        let result = template.render(&values);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_placeholders() {
        let template = "theorem {name} : {prop} := by {proof}";
        let placeholders = extract_placeholders(template);
        assert_eq!(placeholders, vec!["name", "prop", "proof"]);
    }
}
