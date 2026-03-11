//! Tools available to the LLM for executing actions.
//!
//! This module provides a tool execution framework that allows the LLM to interact
//! with the system through a controlled interface. Each tool has defined parameters,
//! validation, and sandboxed execution.
//!
//! # Available Tools
//!
//! - **Shell**: Execute shell commands with security restrictions
//! - **File**: Read, write, and manage files
//! - **Git**: Version control operations
//! - **Browser**: Web browser automation
//! - **Web Search**: Search the web for information
//! - **Editor**: External editor for editing prompts
//!
//! # Tool Definition
//!
//! Tools are defined with JSON Schema for parameter validation:
//!
//! ```rust
//! use clawdius_core::tools::Tool;
//! use serde_json::json;
//!
//! let tool = Tool {
//!     name: "read_file".to_string(),
//!     description: "Read contents of a file".to_string(),
//!     parameters: json!({
//!         "type": "object",
//!         "properties": {
//!             "path": {
//!                 "type": "string",
//!                 "description": "File path to read"
//!             }
//!         },
//!         "required": ["path"]
//!     }),
//! };
//! ```
//!
//! # Tool Execution
//!
//! Tools return structured results:
//!
//! ```rust
//! use clawdius_core::tools::ToolResult;
//! use serde_json::json;
//!
//! let result = ToolResult {
//!     success: true,
//!     output: "File contents...".to_string(),
//!     metadata: Some(json!({
//!         "lines": 42,
//!         "bytes": 1024
//!     })),
//! };
//! ```
//!
//! # Security
//!
//! All tools execute with security restrictions:
//! - Shell commands run in sandboxed environments
//! - File operations are restricted to working directory
//! - Network operations are monitored and logged
//!
//! # Error Handling
//!
//! Tool errors are returned in the result:
//!
//! ```rust
//! use clawdius_core::tools::ToolResult;
//!
//! let error_result = ToolResult {
//!     success: false,
//!     output: "Error: File not found".to_string(),
//!     metadata: None,
//! };
//! ```

pub mod browser;
pub mod editor;
pub mod file;
pub mod git;
pub mod shell;
pub mod web_search;

use serde::{Deserialize, Serialize};

/// Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// Parameter schema (JSON Schema)
    pub parameters: serde_json::Value,
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Success status
    pub success: bool,
    /// Output or error message
    pub output: String,
    /// Additional metadata
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}
