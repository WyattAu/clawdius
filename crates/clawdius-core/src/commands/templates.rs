//! Command templates
//!
//! Template step definition
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateStep {
    /// Tool to use (file, shell, git)
    pub tool: String,
    /// Template content with variable substitution
    pub template: String,
    /// Description
    #[serde(default)]
    pub description: String,
}

/// Command argument definition for templates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandArgument {
    /// Argument name (without $ prefix)
    pub name: String,
    /// Description shown to user when prompting
    #[serde(default)]
    pub description: String,
    /// Whether this argument is required
    #[serde(default)]
    pub required: bool,
    /// Default value if not provided
    #[serde(default)]
    pub default: Option<String>,
    /// Validation pattern (regex)
    #[serde(default)]
    pub validation: Option<String>,
}

/// Command template
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommandTemplate {
    /// Steps
    #[serde(default)]
    pub steps: Vec<TemplateStep>,
    /// Named arguments for the template
    #[serde(default)]
    pub arguments: Vec<CommandArgument>,
    /// Whether to allow additional args
    #[serde(default)]
    pub allow_extra_args: bool,
}
