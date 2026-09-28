//! Custom commands system

// Unwrap purge batch 3: commands module — production code must not
// unwrap/expect; propagate, document a true invariant with an INVARIANT
// comment, or restructure. Lints inherit into all child modules and tests.
#![deny(clippy::unwrap_used, clippy::expect_used)]
// Test builds keep unwrap/expect for brevity (fleet convention, see lib.rs).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

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
