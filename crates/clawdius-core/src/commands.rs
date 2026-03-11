//! Custom commands system

mod executor;
mod parser;
mod templates;

pub use executor::{CommandExecutor, CommandResult};
pub use parser::CommandParser;
pub use templates::{CommandTemplate, TemplateStep};

use serde::{Deserialize, Serialize};

/// Command argument definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandArgument {
    /// Argument name
    pub name: String,
    /// Whether the argument is required
    #[serde(default)]
    pub required: bool,
    /// Default value for optional arguments
    #[serde(default)]
    pub default: Option<String>,
    /// Description of the argument
    #[serde(default)]
    pub description: String,
}

/// A custom command definition
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomCommand {
    /// Command ID
    pub id: String,
    /// Command name
    pub name: String,
    /// Description
    pub description: String,
    /// Template with steps
    pub template: CommandTemplate,
    /// Arguments
    #[serde(default)]
    pub arguments: Vec<CommandArgument>,
}
