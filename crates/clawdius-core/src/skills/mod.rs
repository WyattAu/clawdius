//! Skills and Commands System
//!
//! This module implements a reusable skills/commands system inspired by Claude Code's skills
//! and `OpenClaw`'s `ClawHub`. Skills are reusable workflows that can be invoked with natural language.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Skills Registry                           │
//! │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │
//! │  │ Skill 1  │  │ Skill 2  │  │ Skill 3  │  │ Skill N  │   │
//! │  │ Review   │  │ Test     │  │ Refactor │  │ Custom   │   │
//! │  └──────────┘  └──────────┘  └──────────┘  └──────────┘   │
//! │       │             │             │             │          │
//! │       └─────────────┴─────────────┴─────────────┘          │
//! │                          │                                  │
//! │                   Skill Executor                            │
//! │                   (with context)                            │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use clawdius_core::skills::{SkillRegistry, Skill, SkillContext};
//!
//! let mut registry = SkillRegistry::new();
//! registry.register_builtin_skills();
//!
//! let skill = registry.find("review");
//! let result = skill.execute(context).await?;
//! ```

use crate::llm::providers::LlmClient;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

pub mod markdown_skill;

/// Skills system errors
#[derive(Debug, Error)]
pub enum SkillError {
    /// Skill not found
    #[error("Skill not found: {0}")]
    NotFound(String),
    /// Execution failed
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    /// Invalid arguments
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
    /// Parse error
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Result type for skill operations
pub type Result<T> = std::result::Result<T, SkillError>;

/// Skill metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    /// Skill name (unique identifier)
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Skill version
    pub version: String,
    /// Author
    pub author: Option<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Required arguments
    pub arguments: Vec<SkillArgument>,
    /// Examples of usage
    pub examples: Vec<String>,
}

/// Skill argument definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillArgument {
    /// Argument name
    pub name: String,
    /// Argument description
    pub description: String,
    /// Whether required
    pub required: bool,
    /// Default value
    pub default: Option<String>,
    /// Possible values (for enums)
    pub options: Option<Vec<String>>,
}

/// Skill execution context
#[derive(Clone)]
pub struct SkillContext {
    /// Project root path
    pub project_root: PathBuf,
    /// Current file being edited (if any)
    pub current_file: Option<PathBuf>,
    /// Selected text (if any)
    pub selection: Option<String>,
    /// Arguments passed to the skill
    pub arguments: HashMap<String, String>,
    /// Additional context
    pub extra: HashMap<String, serde_json::Value>,
    /// Optional LLM client for skills that need LLM execution
    pub llm: Option<Arc<dyn LlmClient>>,
}

impl std::fmt::Debug for SkillContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillContext")
            .field("project_root", &self.project_root)
            .field("current_file", &self.current_file)
            .field("selection", &self.selection.is_some())
            .field("arguments", &self.arguments)
            .field("extra", &self.extra)
            .field("has_llm", &self.llm.is_some())
            .finish()
    }
}

impl SkillContext {
    /// Create a new skill context
    #[must_use]
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            current_file: None,
            selection: None,
            arguments: HashMap::new(),
            extra: HashMap::new(),
            llm: None,
        }
    }

    /// Set the LLM client for this context
    #[must_use]
    pub fn with_llm(mut self, llm: Arc<dyn LlmClient>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Set the current file
    #[must_use]
    pub fn with_file(mut self, path: PathBuf) -> Self {
        self.current_file = Some(path);
        self
    }

    /// Set the selection
    #[must_use]
    pub fn with_selection(mut self, text: String) -> Self {
        self.selection = Some(text);
        self
    }

    /// Add an argument
    pub fn add_argument(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.arguments.insert(name.into(), value.into());
    }

    /// Get an argument
    #[must_use]
    pub fn get_argument(&self, name: &str) -> Option<&String> {
        self.arguments.get(name)
    }

    /// Get a required argument or error
    pub fn require_argument(&self, name: &str) -> Result<&String> {
        self.arguments.get(name).ok_or_else(|| {
            SkillError::InvalidArguments(format!("Missing required argument: {name}"))
        })
    }
}

/// Skill execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResult {
    /// Whether the skill succeeded
    pub success: bool,
    /// Output message
    pub output: String,
    /// Any files that were modified
    pub modified_files: Vec<String>,
    /// Any additional data
    pub data: HashMap<String, serde_json::Value>,
    /// Execution timestamp
    pub timestamp: DateTime<Utc>,
    /// Execution duration (ms)
    pub duration_ms: u64,
}

impl SkillResult {
    /// Create a successful result
    #[must_use]
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            modified_files: Vec::new(),
            data: HashMap::new(),
            timestamp: Utc::now(),
            duration_ms: 0,
        }
    }

    /// Create a failed result
    #[must_use]
    pub fn failure(output: impl Into<String>) -> Self {
        Self {
            success: false,
            output: output.into(),
            modified_files: Vec::new(),
            data: HashMap::new(),
            timestamp: Utc::now(),
            duration_ms: 0,
        }
    }

    /// Add a modified file
    pub fn add_modified_file(&mut self, path: impl Into<String>) {
        self.modified_files.push(path.into());
    }
}

/// Skill trait for implementing custom skills
#[async_trait::async_trait]
pub trait Skill: Send + Sync {
    /// Get skill metadata
    fn meta(&self) -> &SkillMeta;

    /// Execute the skill
    async fn execute(&self, context: SkillContext) -> Result<SkillResult>;

    /// Validate arguments before execution
    fn validate_arguments(&self, context: &SkillContext) -> Result<()> {
        for arg in &self.meta().arguments {
            if arg.required && context.get_argument(&arg.name).is_none() && arg.default.is_none() {
                return Err(SkillError::InvalidArguments(format!(
                    "Missing required argument: {}",
                    arg.name
                )));
            }
        }
        Ok(())
    }
}

/// Code review skill
pub struct CodeReviewSkill {
    meta: SkillMeta,
}

impl CodeReviewSkill {
    /// Create a new code review skill
    #[must_use]
    pub fn new() -> Self {
        Self {
            meta: SkillMeta {
                name: "review".into(),
                description: "Perform a comprehensive code review".into(),
                version: "1.0.0".into(),
                author: Some("Clawdius Team".into()),
                tags: vec!["review".into(), "quality".into()],
                arguments: vec![SkillArgument {
                    name: "focus".into(),
                    description: "Review focus area".into(),
                    required: false,
                    default: Some("general".into()),
                    options: Some(vec![
                        "general".into(),
                        "security".into(),
                        "performance".into(),
                        "style".into(),
                    ]),
                }],
                examples: vec!["/review".into(), "/review focus=security".into()],
            },
        }
    }
}

impl Default for CodeReviewSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Skill for CodeReviewSkill {
    fn meta(&self) -> &SkillMeta {
        &self.meta
    }

    async fn execute(&self, context: SkillContext) -> Result<SkillResult> {
        let focus = context
            .get_argument("focus")
            .map_or("general", String::as_str);

        let selection = context
            .selection
            .as_deref()
            .ok_or_else(|| SkillError::InvalidArguments("No code selected for review".into()))?;

        // In a real implementation, this would call the LLM
        let output = format!(
            "Code Review (focus: {})\n\n\
             Analyzed {} characters of code.\n\n\
             Suggestions:\n\
             1. Consider adding documentation\n\
             2. Review error handling\n\
             3. Check for potential edge cases\n",
            focus,
            selection.len()
        );

        Ok(SkillResult::success(output))
    }
}

/// Test generation skill
pub struct GenerateTestsSkill {
    meta: SkillMeta,
}

impl GenerateTestsSkill {
    /// Create a new test generation skill
    #[must_use]
    pub fn new() -> Self {
        Self {
            meta: SkillMeta {
                name: "test".into(),
                description: "Generate unit tests for the selected code".into(),
                version: "1.0.0".into(),
                author: Some("Clawdius Team".into()),
                tags: vec!["test".into(), "generation".into()],
                arguments: vec![SkillArgument {
                    name: "framework".into(),
                    description: "Testing framework to use".into(),
                    required: false,
                    default: Some("auto".into()),
                    options: Some(vec![
                        "auto".into(),
                        "pytest".into(),
                        "jest".into(),
                        "cargo-test".into(),
                        "junit".into(),
                    ]),
                }],
                examples: vec!["/test".into(), "/test framework=pytest".into()],
            },
        }
    }
}

impl Default for GenerateTestsSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Skill for GenerateTestsSkill {
    fn meta(&self) -> &SkillMeta {
        &self.meta
    }

    async fn execute(&self, context: SkillContext) -> Result<SkillResult> {
        let framework = context
            .get_argument("framework")
            .map_or("auto", String::as_str);

        let selection = context.selection.as_deref().ok_or_else(|| {
            SkillError::InvalidArguments("No code selected for test generation".into())
        })?;

        // In a real implementation, this would call the LLM
        let output = format!(
            "Generated Tests (framework: {})\n\n\
             // TODO: Generated test cases would appear here\n\
             // Based on {} characters of code\n",
            framework,
            selection.len()
        );

        Ok(SkillResult::success(output))
    }
}

/// Refactor skill
pub struct RefactorSkill {
    meta: SkillMeta,
}

impl RefactorSkill {
    /// Create a new refactor skill
    #[must_use]
    pub fn new() -> Self {
        Self {
            meta: SkillMeta {
                name: "refactor".into(),
                description: "Suggest refactoring improvements".into(),
                version: "1.0.0".into(),
                author: Some("Clawdius Team".into()),
                tags: vec!["refactor".into(), "improvement".into()],
                arguments: vec![SkillArgument {
                    name: "goal".into(),
                    description: "Refactoring goal".into(),
                    required: false,
                    default: Some("readability".into()),
                    options: Some(vec![
                        "readability".into(),
                        "performance".into(),
                        "maintainability".into(),
                        "simplicity".into(),
                    ]),
                }],
                examples: vec!["/refactor".into(), "/refactor goal=performance".into()],
            },
        }
    }
}

impl Default for RefactorSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Skill for RefactorSkill {
    fn meta(&self) -> &SkillMeta {
        &self.meta
    }

    async fn execute(&self, context: SkillContext) -> Result<SkillResult> {
        let goal = context
            .get_argument("goal")
            .map_or("readability", String::as_str);

        let selection = context.selection.as_deref().ok_or_else(|| {
            SkillError::InvalidArguments("No code selected for refactoring".into())
        })?;

        // In a real implementation, this would call the LLM
        let output = format!(
            "Refactoring Suggestions (goal: {})\n\n\
             Analyzed {} characters of code.\n\n\
             Suggestions:\n\
             1. Extract complex logic into helper functions\n\
             2. Improve variable naming\n\
             3. Reduce function complexity\n",
            goal,
            selection.len()
        );

        Ok(SkillResult::success(output))
    }
}

/// Explain code skill
pub struct ExplainSkill {
    meta: SkillMeta,
}

impl ExplainSkill {
    /// Create a new explain skill
    #[must_use]
    pub fn new() -> Self {
        Self {
            meta: SkillMeta {
                name: "explain".into(),
                description: "Explain the selected code".into(),
                version: "1.0.0".into(),
                author: Some("Clawdius Team".into()),
                tags: vec!["explain".into(), "documentation".into()],
                arguments: vec![SkillArgument {
                    name: "level".into(),
                    description: "Explanation detail level".into(),
                    required: false,
                    default: Some("intermediate".into()),
                    options: Some(vec![
                        "beginner".into(),
                        "intermediate".into(),
                        "advanced".into(),
                    ]),
                }],
                examples: vec!["/explain".into(), "/explain level=beginner".into()],
            },
        }
    }
}

impl Default for ExplainSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Skill for ExplainSkill {
    fn meta(&self) -> &SkillMeta {
        &self.meta
    }

    async fn execute(&self, context: SkillContext) -> Result<SkillResult> {
        let level = context
            .get_argument("level")
            .map_or("intermediate", String::as_str);

        let selection = context.selection.as_deref().ok_or_else(|| {
            SkillError::InvalidArguments("No code selected for explanation".into())
        })?;

        // In a real implementation, this would call the LLM
        let output = format!(
            "Code Explanation (level: {})\n\n\
             This code performs the following operations:\n\
             [Explanation would be generated here based on {} characters of code]\n",
            level,
            selection.len()
        );

        Ok(SkillResult::success(output))
    }
}

/// Skills registry
pub struct SkillRegistry {
    skills: Arc<RwLock<HashMap<String, Arc<dyn Skill>>>>,
}

impl SkillRegistry {
    /// Create a new skill registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            skills: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a skill
    pub async fn register(&self, skill: Arc<dyn Skill>) {
        let mut skills = self.skills.write().await;
        skills.insert(skill.meta().name.clone(), skill);
    }

    /// Register all built-in skills
    pub async fn register_builtin_skills(&self) {
        self.register(Arc::new(CodeReviewSkill::new())).await;
        self.register(Arc::new(GenerateTestsSkill::new())).await;
        self.register(Arc::new(RefactorSkill::new())).await;
        self.register(Arc::new(ExplainSkill::new())).await;
    }

    /// Find a skill by name
    pub async fn find(&self, name: &str) -> Option<Arc<dyn Skill>> {
        let skills = self.skills.read().await;
        skills.get(name).cloned()
    }

    /// List all available skills
    pub async fn list(&self) -> Vec<SkillMeta> {
        let skills = self.skills.read().await;
        skills.values().map(|s| s.meta().clone()).collect()
    }

    /// Execute a skill by name
    pub async fn execute(&self, name: &str, context: SkillContext) -> Result<SkillResult> {
        let skill = self
            .find(name)
            .await
            .ok_or_else(|| SkillError::NotFound(name.into()))?;

        skill.validate_arguments(&context)?;
        skill.execute(context).await
    }

    /// Parse a skill command (e.g., "/review focus=security")
    pub fn parse_command(&self, command: &str) -> Result<(String, HashMap<String, String>)> {
        let command = command.trim();

        // Remove leading slash
        let command = command
            .strip_prefix('/')
            .ok_or_else(|| SkillError::ParseError("Command must start with /".into()))?;

        // Split into skill name and arguments
        let parts: Vec<&str> = command.split_whitespace().collect();

        if parts.is_empty() {
            return Err(SkillError::ParseError("Empty command".into()));
        }

        let skill_name = parts[0].to_string();
        let mut arguments = HashMap::new();

        // Parse arguments (key=value format)
        for part in &parts[1..] {
            if let Some((key, value)) = part.split_once('=') {
                arguments.insert(key.to_string(), value.to_string());
            }
        }

        Ok((skill_name, arguments))
    }

    /// Load all markdown skill files from a directory and register them.
    /// Returns a list of skill names that were successfully loaded.
    pub async fn load_skills_from_dir(&self, dir: &Path) -> Result<Vec<String>> {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(Vec::new()),
        };

        let mut loaded = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "md") {
                match crate::skills::markdown_skill::MarkdownSkill::from_file(&path) {
                    Ok(skill) => {
                        let name = skill.meta().name.clone();
                        self.register(Arc::new(skill)).await;
                        loaded.push(name);
                    },
                    Err(e) => {
                        tracing::warn!("Failed to load skill from {}: {e}", path.display());
                    },
                }
            }
        }
        Ok(loaded)
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command() {
        let registry = SkillRegistry::new();

        let (name, args) = registry.parse_command("/review focus=security").unwrap();
        assert_eq!(name, "review");
        assert_eq!(args.get("focus"), Some(&"security".to_string()));

        let (name, args) = registry.parse_command("/test").unwrap();
        assert_eq!(name, "test");
        assert!(args.is_empty());
    }

    #[test]
    fn test_skill_context() {
        let ctx =
            SkillContext::new(PathBuf::from("/project")).with_selection("fn main() {}".into());

        assert!(ctx.selection.is_some());
    }

    #[tokio::test]
    async fn test_skill_registry() {
        let registry = SkillRegistry::new();
        registry.register_builtin_skills().await;

        let skills = registry.list().await;
        assert!(!skills.is_empty());

        let review = registry.find("review").await;
        assert!(review.is_some());
    }

    #[tokio::test]
    async fn test_execute_skill() {
        let registry = SkillRegistry::new();
        registry.register_builtin_skills().await;

        let ctx =
            SkillContext::new(PathBuf::from("/project")).with_selection("fn main() {}".into());

        let result = registry.execute("review", ctx).await.unwrap();
        assert!(result.success);
    }
}
