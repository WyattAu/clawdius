//! Command templates

use serde::{Deserialize, Serialize};

/// Template step definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateStep {
    /// Tool to use (file, shell, git)
    pub tool: String,
    /// Template content
    pub template: String,
    /// Description
    #[serde(default)]
    pub description: String,
}

/// Command template
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommandTemplate {
    /// Steps
    #[serde(default)]
    pub steps: Vec<TemplateStep>,
}
