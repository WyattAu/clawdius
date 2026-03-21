//! Prompt Templates for Enterprise Teams
//!
//! Pre-defined prompt templates for common coding tasks.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Prompt template category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemplateCategory {
    CodeGeneration,
    CodeReview,
    Refactoring,
    Documentation,
    Testing,
    Debugging,
    Security,
    Performance,
    Custom,
}

impl Default for TemplateCategory {
    fn default() -> Self {
        Self::Custom
    }
}

/// Template variable definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVariable {
    pub name: String,
    pub description: String,
    pub default_value: Option<String>,
    pub required: bool,
    pub validation_pattern: Option<String>,
    pub examples: Vec<String>,
}

impl TemplateVariable {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            default_value: None,
            required: false,
            validation_pattern: None,
            examples: Vec::new(),
        }
    }

    pub fn with_default(mut self, value: impl Into<String>) -> Self {
        self.default_value = Some(value.into());
        self
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn with_validation(mut self, pattern: impl Into<String>) -> Self {
        self.validation_pattern = Some(pattern.into());
        self
    }

    pub fn with_example(mut self, example: impl Into<String>) -> Self {
        self.examples.push(example.into());
        self
    }
}

/// Prompt template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub category: TemplateCategory,
    pub content: String,
    pub variables: Vec<TemplateVariable>,
    pub system_prompt: Option<String>,
    pub supported_languages: Vec<String>,
    pub tags: Vec<String>,
    pub is_builtin: bool,
    pub team_id: Option<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl PromptTemplate {
    pub fn new(
        name: impl Into<String>,
        category: TemplateCategory,
        content: impl Into<String>,
        created_by: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            description: None,
            category,
            content: content.into(),
            variables: Vec::new(),
            system_prompt: None,
            supported_languages: Vec::new(),
            tags: Vec::new(),
            is_builtin: false,
            team_id: None,
            created_by: created_by.into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    pub fn with_variable(mut self, variable: TemplateVariable) -> Self {
        self.variables.push(variable);
        self
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn with_languages(mut self, languages: Vec<String>) -> Self {
        self.supported_languages = languages;
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn for_team(mut self, team_id: impl Into<String>) -> Self {
        self.team_id = Some(team_id.into());
        self
    }

    pub fn builtin(mut self) -> Self {
        self.is_builtin = true;
        self
    }

    pub fn render(&self, variables: &HashMap<String, String>) -> Result<String, TemplateError> {
        let mut result = self.content.clone();

        // Validate required variables
        for var in &self.variables {
            if var.required && !variables.contains_key(&var.name) && var.default_value.is_none() {
                return Err(TemplateError::MissingVariable(var.name.clone()));
            }
        }

        // Replace variables
        for var in &self.variables {
            let placeholder = format!("{{{{{}}}}}", var.name);
            let value = variables
                .get(&var.name)
                .cloned()
                .or_else(|| var.default_value.clone())
                .unwrap_or_default();

            // Validate against pattern if present
            if let Some(pattern) = &var.validation_pattern {
                let regex = regex::Regex::new(pattern)
                    .map_err(|e| TemplateError::InvalidPattern(e.to_string()))?;
                if !value.is_empty() && !regex.is_match(&value) {
                    return Err(TemplateError::ValidationFailed(var.name.clone(), value));
                }
            }

            result = result.replace(&placeholder, &value);
        }

        Ok(result)
    }

    /// Extract variable names from content
    pub fn extract_variables(&self) -> Vec<String> {
        let re = regex::Regex::new(r"\{\{(\w+)\}\}").unwrap();
        re.captures_iter(&self.content)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect()
    }

    /// Check if template supports a language
    pub fn supports_language(&self, language: &str) -> bool {
        self.supported_languages.is_empty()
            || self
                .supported_languages
                .iter()
                .any(|l| l.eq_ignore_ascii_case(language))
    }
}

/// Template error
#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    #[error("Missing required variable: {0}")]
    MissingVariable(String),
    #[error("Invalid validation pattern: {0}")]
    InvalidPattern(String),
    #[error("Validation failed for variable '{0}': {1}")]
    ValidationFailed(String, String),
    #[error("Template not found: {0}")]
    NotFound(String),
}

/// Builtin templates manager
pub struct BuiltinTemplates {
    templates: HashMap<String, PromptTemplate>,
}

impl BuiltinTemplates {
    /// Create a new builtin templates manager with default templates
    pub fn new() -> Self {
        let mut templates = HashMap::new();

        // Code Review Template
        let code_review = PromptTemplate::new(
            "Code Review",
            TemplateCategory::CodeReview,
            "Review the following {{language}} code:\n\n```{{language}}\n{{code}}\n```\n\nFocus on:\n- Code quality and readability\n- Potential bugs or errors\n- Performance considerations\n- Security vulnerabilities\n- Best practices adherence",
            "system",
        )
        .with_variable(
            TemplateVariable::new("language", "Programming language")
                .required()
                .with_example("Rust")
                .with_example("Python"),
        )
        .with_variable(
            TemplateVariable::new("code", "The code to review")
                .required(),
        )
        .with_system_prompt("You are an expert code reviewer. Provide thorough, actionable feedback.")
        .with_languages(vec!["Rust".to_string(), "Python".to_string(), "JavaScript".to_string()])
        .with_tags(vec!["review".to_string(), "quality".to_string()])
        .builtin();

        templates.insert(code_review.id.clone(), code_review);

        // Test Generation Template
        let test_gen = PromptTemplate::new(
            "Generate Tests",
            TemplateCategory::Testing,
            "Generate comprehensive tests for the following {{language}} code:\n\n```{{language}}\n{{code}}\n```\n\n{{requirements}}",
            "system",
        )
        .with_variable(
            TemplateVariable::new("language", "Programming language")
                .required(),
        )
        .with_variable(
            TemplateVariable::new("code", "The code to test")
                .required(),
        )
        .with_variable(
            TemplateVariable::new("requirements", "Specific testing requirements")
                .with_default("Include unit tests, edge cases, and integration tests"),
        )
        .with_system_prompt("You are an expert test engineer. Generate comprehensive, maintainable tests.")
        .builtin();

        templates.insert(test_gen.id.clone(), test_gen);

        // Documentation Template
        let docs = PromptTemplate::new(
            "Generate Documentation",
            TemplateCategory::Documentation,
            "Generate {{doc_type}} documentation for:\n\n```{{language}}\n{{code}}\n```\n\n{{style_guide}}",
            "system",
        )
        .with_variable(
            TemplateVariable::new("language", "Programming language")
                .required(),
        )
        .with_variable(
            TemplateVariable::new("code", "The code to document")
                .required(),
        )
        .with_variable(
            TemplateVariable::new("doc_type", "Type of documentation")
                .with_default("API reference")
                .with_example("API reference")
                .with_example("Inline comments")
                .with_example("README"),
        )
        .with_variable(
            TemplateVariable::new("style_guide", "Documentation style preferences")
                .with_default("Clear, concise, with examples"),
        )
        .with_system_prompt("You are a technical writer. Generate clear, comprehensive documentation.")
        .builtin();

        templates.insert(docs.id.clone(), docs);

        // Refactoring Template
        let refactor = PromptTemplate::new(
            "Refactor Code",
            TemplateCategory::Refactoring,
            "Refactor the following {{language}} code to improve {{focus}}:\n\n```{{language}}\n{{code}}\n```\n\nConstraints:\n{{constraints}}",
            "system",
        )
        .with_variable(
            TemplateVariable::new("language", "Programming language")
                .required(),
        )
        .with_variable(
            TemplateVariable::new("code", "The code to refactor")
                .required(),
        )
        .with_variable(
            TemplateVariable::new("focus", "Primary refactoring focus")
                .with_default("readability and maintainability")
                .with_example("readability")
                .with_example("performance")
                .with_example("memory efficiency"),
        )
        .with_variable(
            TemplateVariable::new("constraints", "Refactoring constraints or requirements")
                .with_default("Maintain existing behavior and API compatibility"),
        )
        .with_system_prompt("You are a software architect. Provide clean, idiomatic refactoring while preserving functionality.")
        .builtin();

        templates.insert(refactor.id.clone(), refactor);

        // Security Review Template
        let security = PromptTemplate::new(
            "Security Review",
            TemplateCategory::Security,
            "Perform a security review of:\n\n```{{language}}\n{{code}}\n```\n\nFocus on:\n- Input validation\n- Authentication/authorization\n- Injection vulnerabilities\n- Data exposure\n- Cryptographic issues",
            "system",
        )
        .with_variable(
            TemplateVariable::new("language", "Programming language")
                .required(),
        )
        .with_variable(
            TemplateVariable::new("code", "The code to review")
                .required(),
        )
        .with_system_prompt("You are a security expert. Identify vulnerabilities and provide remediation guidance.")
        .with_tags(vec!["security".to_string(), "review".to_string()])
        .builtin();

        templates.insert(security.id.clone(), security);

        Self { templates }
    }

    /// Get a builtin template by name
    pub fn get(&self, name: &str) -> Option<&PromptTemplate> {
        self.templates.values().find(|t| t.name == name)
    }

    /// Get a builtin template by ID
    pub fn get_by_id(&self, id: &str) -> Option<&PromptTemplate> {
        self.templates.get(id)
    }

    /// List all builtin templates
    pub fn list(&self) -> Vec<&PromptTemplate> {
        self.templates.values().collect()
    }

    /// List templates by category
    pub fn list_by_category(&self, category: TemplateCategory) -> Vec<&PromptTemplate> {
        self.templates
            .values()
            .filter(|t| t.category == category)
            .collect()
    }

    /// Get template names
    pub fn names(&self) -> Vec<String> {
        self.templates.values().map(|t| t.name.clone()).collect()
    }
}

impl Default for BuiltinTemplates {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_render() {
        let template = PromptTemplate::new(
            "Test Template",
            TemplateCategory::CodeGeneration,
            "Hello, {{name}}! Your task is: {{task}}",
            "test-user",
        )
        .with_variable(TemplateVariable::new("name", "User name").required())
        .with_variable(
            TemplateVariable::new("task", "Task description").with_default("code review"),
        );

        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Alice".to_string());

        let result = template.render(&vars).unwrap();
        assert_eq!(result, "Hello, Alice! Your task is: code review");
    }

    #[test]
    fn test_template_missing_required() {
        let template = PromptTemplate::new(
            "Test Template",
            TemplateCategory::CodeGeneration,
            "Hello, {{name}}!",
            "test-user",
        )
        .with_variable(TemplateVariable::new("name", "User name").required());

        let vars = HashMap::new();
        let result = template.render(&vars);
        assert!(matches!(result, Err(TemplateError::MissingVariable(_))));
    }
}
