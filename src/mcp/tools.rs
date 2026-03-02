//! MCP Tool implementations for Graph-RAG

use crate::ast_store::{AstQuery, NodeType};
use crate::mcp::types::{McpError, ToolDefinition, ToolRequest, ToolResponse};

/// Trait for MCP tools
pub trait McpTool: Send + Sync {
    /// Get tool name
    fn name(&self) -> &str;

    /// Get tool description
    fn description(&self) -> &str;

    /// Get tool schema
    fn schema(&self) -> serde_json::Value;

    /// Execute the tool
    fn execute(&self, request: ToolRequest) -> Result<ToolResponse, McpError>;
}

/// Search AST nodes tool
#[derive(Debug)]
pub struct SearchAstTool {
    definition: ToolDefinition,
}

impl SearchAstTool {
    /// Create a new search AST tool
    #[must_use]
    pub fn new() -> Self {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "node_type": {
                    "type": "string",
                    "enum": ["function", "struct", "enum", "trait", "impl", "module"],
                    "description": "Type of AST node to search for"
                },
                "name_pattern": {
                    "type": "string",
                    "description": "Name pattern (supports * wildcard)"
                },
                "file_path": {
                    "type": "string",
                    "description": "Filter by file path"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results"
                }
            }
        });

        Self {
            definition: ToolDefinition::new(
                "search_ast",
                "Search the AST index for code structures",
                schema,
            ),
        }
    }
}

impl Default for SearchAstTool {
    fn default() -> Self {
        Self::new()
    }
}

impl McpTool for SearchAstTool {
    fn name(&self) -> &str {
        "search_ast"
    }

    fn description(&self) -> &str {
        "Search the AST index for code structures like functions, structs, traits, etc."
    }

    fn schema(&self) -> serde_json::Value {
        self.definition.input_schema.clone()
    }

    fn execute(&self, request: ToolRequest) -> Result<ToolResponse, McpError> {
        let node_type: Option<NodeType> = request
            .arguments
            .get("node_type")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok());

        let name_pattern = request
            .arguments
            .get("name_pattern")
            .and_then(|v| v.as_str())
            .map(String::from);

        let limit = request
            .arguments
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);

        let _query = AstQuery {
            node_type,
            name_pattern,
            file_path: None,
            language: None,
            limit,
        };

        Ok(ToolResponse::text(
            serde_json::json!({
                "query_received": true,
                "note": "Tool needs GraphRag reference for actual execution"
            })
            .to_string(),
        ))
    }
}

/// Get call graph tool
#[derive(Debug)]
pub struct GetCallGraphTool {
    definition: ToolDefinition,
}

impl GetCallGraphTool {
    /// Create a new call graph tool
    #[must_use]
    pub fn new() -> Self {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "function_id": {
                    "type": "string",
                    "description": "Node ID of the function"
                }
            },
            "required": ["function_id"]
        });

        Self {
            definition: ToolDefinition::new(
                "get_call_graph",
                "Get the call graph for a function",
                schema,
            ),
        }
    }
}

impl Default for GetCallGraphTool {
    fn default() -> Self {
        Self::new()
    }
}

impl McpTool for GetCallGraphTool {
    fn name(&self) -> &str {
        "get_call_graph"
    }

    fn description(&self) -> &str {
        "Get the call graph showing all functions called by a given function"
    }

    fn schema(&self) -> serde_json::Value {
        self.definition.input_schema.clone()
    }

    fn execute(&self, request: ToolRequest) -> Result<ToolResponse, McpError> {
        let function_id = request
            .arguments
            .get("function_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidArguments("Missing function_id".into()))?;

        Ok(ToolResponse::text(
            serde_json::json!({
                "function_id": function_id,
                "note": "Tool needs GraphRag reference for actual execution"
            })
            .to_string(),
        ))
    }
}

/// Semantic search tool
#[derive(Debug)]
pub struct SemanticSearchTool {
    definition: ToolDefinition,
}

impl SemanticSearchTool {
    /// Create a new semantic search tool
    #[must_use]
    pub fn new() -> Self {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Natural language query"
                },
                "k": {
                    "type": "integer",
                    "description": "Number of results to return",
                    "default": 5
                }
            },
            "required": ["query"]
        });

        Self {
            definition: ToolDefinition::new(
                "semantic_search",
                "Search code semantically using natural language",
                schema,
            ),
        }
    }
}

impl Default for SemanticSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl McpTool for SemanticSearchTool {
    fn name(&self) -> &str {
        "semantic_search"
    }

    fn description(&self) -> &str {
        "Search code using natural language queries via semantic similarity"
    }

    fn schema(&self) -> serde_json::Value {
        self.definition.input_schema.clone()
    }

    fn execute(&self, request: ToolRequest) -> Result<ToolResponse, McpError> {
        let query = request
            .arguments
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidArguments("Missing query".into()))?;

        let k = request
            .arguments
            .get("k")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;

        Ok(ToolResponse::text(
            serde_json::json!({
                "query": query,
                "k": k,
                "note": "Tool needs GraphRag reference for actual execution"
            })
            .to_string(),
        ))
    }
}

/// Find impact tool
#[derive(Debug)]
pub struct FindImpactTool {
    definition: ToolDefinition,
}

impl FindImpactTool {
    /// Create a new find impact tool
    #[must_use]
    pub fn new() -> Self {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "node_id": {
                    "type": "string",
                    "description": "Node ID to analyze impact for"
                }
            },
            "required": ["node_id"]
        });

        Self {
            definition: ToolDefinition::new(
                "find_impact",
                "Find all nodes that would be impacted by changes to a node",
                schema,
            ),
        }
    }
}

impl Default for FindImpactTool {
    fn default() -> Self {
        Self::new()
    }
}

impl McpTool for FindImpactTool {
    fn name(&self) -> &str {
        "find_impact"
    }

    fn description(&self) -> &str {
        "Find all code that would be impacted by changes to a given node"
    }

    fn schema(&self) -> serde_json::Value {
        self.definition.input_schema.clone()
    }

    fn execute(&self, request: ToolRequest) -> Result<ToolResponse, McpError> {
        let node_id = request
            .arguments
            .get("node_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidArguments("Missing node_id".into()))?;

        Ok(ToolResponse::text(
            serde_json::json!({
                "node_id": node_id,
                "note": "Tool needs GraphRag reference for actual execution"
            })
            .to_string(),
        ))
    }
}

/// Get all built-in tools
#[must_use]
pub fn builtin_tools() -> Vec<Box<dyn McpTool>> {
    vec![
        Box::new(SearchAstTool::new()),
        Box::new(GetCallGraphTool::new()),
        Box::new(SemanticSearchTool::new()),
        Box::new(FindImpactTool::new()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_search_ast_tool() {
        let tool = SearchAstTool::new();
        assert_eq!(tool.name(), "search_ast");

        let request = ToolRequest {
            name: "search_ast".into(),
            arguments: HashMap::new(),
        };

        let response = tool.execute(request);
        assert!(response.is_ok());
    }

    #[test]
    fn test_semantic_search_tool() {
        let tool = SemanticSearchTool::new();
        assert_eq!(tool.name(), "semantic_search");

        let mut args = HashMap::new();
        args.insert(
            "query".into(),
            serde_json::json!("find authentication code"),
        );

        let request = ToolRequest {
            name: "semantic_search".into(),
            arguments: args,
        };

        let response = tool.execute(request);
        assert!(response.is_ok());
    }

    #[test]
    fn test_builtin_tools() {
        let tools = builtin_tools();
        assert_eq!(tools.len(), 4);
    }
}
