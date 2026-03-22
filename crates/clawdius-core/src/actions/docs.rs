//! Documentation generation actions.
//!
//! This module provides LLM-powered documentation generation for functions,
//! modules, structs, and other code constructs.

use super::{ActionContext, ActionEdit, ActionKind, Applicability, CodeAction, Range, TextEdit};
use crate::llm::{ChatMessage, ChatRole, LlmClient};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A documented code element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentedElement {
    /// The element name (function, struct, etc.)
    pub name: String,
    /// The element type (function, struct, enum, etc.)
    pub element_type: ElementType,
    /// Generated documentation
    pub documentation: String,
    /// Code examples
    pub examples: Vec<CodeExample>,
}

/// Type of code element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementType {
    Function,
    Struct,
    Enum,
    Trait,
    Module,
    Constant,
    TypeAlias,
    Macro,
}

/// A code example for documentation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExample {
    /// Example title
    pub title: String,
    /// Example code
    pub code: String,
    /// Expected output (if applicable)
    pub output: Option<String>,
}

/// Generated documentation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedDocs {
    /// Element being documented
    pub element: DocumentedElement,
    /// Documentation format used
    pub format: DocFormat,
    /// Whether to include inline comments
    pub include_inline: bool,
}

/// Documentation format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocFormat {
    /// Rust doc comments (///)
    Rustdoc,
    /// JSDoc format
    JsDoc,
    /// Python docstrings
    PythonDocstring,
    /// Markdown
    Markdown,
}

/// Documentation generator powered by LLM.
pub struct GenerateDocs {
    llm: Arc<dyn LlmClient>,
}

impl GenerateDocs {
    /// Creates a new documentation generator.
    #[must_use]
    pub fn new(llm: Arc<dyn LlmClient>) -> Self {
        Self { llm }
    }

    /// Generate documentation for a function.
    ///
    /// # Errors
    ///
    /// Returns an error if the LLM request fails or the response cannot be parsed.
    pub async fn generate_for_function(
        &self,
        name: &str,
        signature: &str,
        body: &str,
        _language: &str,
    ) -> Result<GeneratedDocs> {
        let prompt = format!(
            r#"Generate comprehensive documentation for the following function:

Function name: {}
Signature: {}
Implementation:
{}

Please generate documentation that includes:
1. A brief description of what the function does
2. Parameter descriptions (name, type, purpose)
3. Return value description
4. Any error conditions or panics
5. Example usage
6. Any important notes or caveats

Format your response as JSON:
{{
  "element": {{
    "name": "<function_name>",
    "element_type": "function",
    "documentation": "<main documentation text>",
    "examples": [
      {{
        "title": "<example_title>",
        "code": "<example_code>",
        "output": "<expected_output_or_null>"
      }}
    ]
  }},
  "format": "<format_type>",
  "include_inline": true
}}

Use the appropriate documentation format for the language:
- Rust: rustdoc
- TypeScript/JavaScript: jsdoc
- Python: python_docstring"#,
            name, signature, body
        );

        let messages = vec![ChatMessage {
            role: ChatRole::User,
            content: prompt,
        }];

        let response = self.llm.chat(messages).await?;
        let docs: GeneratedDocs = parse_llm_doc_response(&response, name, ElementType::Function)?;

        Ok(docs)
    }

    /// Generate documentation for a struct.
    ///
    /// # Errors
    ///
    /// Returns an error if the LLM request fails or the response cannot be parsed.
    pub async fn generate_for_struct(
        &self,
        name: &str,
        definition: &str,
        _language: &str,
    ) -> Result<GeneratedDocs> {
        let prompt = format!(
            r#"Generate comprehensive documentation for the following struct/ class:

Name: {}
Definition:
{}

Please generate documentation that includes:
1. A brief description of the struct/class purpose
2. Field/property descriptions
3. Usage examples
4. Any invariants or constraints
5. Thread safety notes (if applicable)

Format your response as JSON:
{{
  "element": {{
    "name": "<struct_name>",
    "element_type": "struct",
    "documentation": "<main documentation text>",
    "examples": [
      {{
        "title": "<example_title>",
        "code": "<example_code>",
        "output": "<expected_output_or_null>"
      }}
    ]
  }},
  "format": "<format_type>",
  "include_inline": true
}}"#,
            name, definition
        );

        let messages = vec![ChatMessage {
            role: ChatRole::User,
            content: prompt,
        }];

        let response = self.llm.chat(messages).await?;
        let docs: GeneratedDocs = parse_llm_doc_response(&response, name, ElementType::Struct)?;

        Ok(docs)
    }

    /// Generate documentation for a module.
    ///
    /// # Errors
    ///
    /// Returns an error if the LLM request fails or the response cannot be parsed.
    pub async fn generate_for_module(
        &self,
        name: &str,
        description: &str,
        exports: &[String],
    ) -> Result<GeneratedDocs> {
        let prompt = format!(
            r#"Generate comprehensive module-level documentation:

Module name: {}
Description: {}
Exported items: {}

Please generate documentation that includes:
1. Module purpose and overview
2. Main features/capabilities
3. Usage examples
4. Important types and functions
5. Any design decisions or constraints

Format your response as JSON:
{{
  "element": {{
    "name": "<module_name>",
    "element_type": "module",
    "documentation": "<main documentation text>",
    "examples": [
      {{
        "title": "<example_title>",
        "code": "<example_code>",
        "output": "<expected_output_or_null>"
      }}
    ]
  }},
  "format": "markdown",
  "include_inline": false
}}"#,
            name,
            description,
            exports.join(", ")
        );

        let messages = vec![ChatMessage {
            role: ChatRole::User,
            content: prompt,
        }];

        let response = self.llm.chat(messages).await?;
        let docs: GeneratedDocs = parse_llm_doc_response(&response, name, ElementType::Module)?;

        Ok(docs)
    }

    /// Format documentation for a specific language.
    #[must_use]
    pub fn format_docs(&self, docs: &GeneratedDocs, language: &str) -> String {
        let format = match language {
            "rust" => DocFormat::Rustdoc,
            "typescript" | "javascript" => DocFormat::JsDoc,
            "python" => DocFormat::PythonDocstring,
            _ => DocFormat::Markdown,
        };

        match format {
            DocFormat::Rustdoc => format_rustdoc(docs),
            DocFormat::JsDoc => format_jsdoc(docs),
            DocFormat::PythonDocstring => format_python_docstring(docs),
            DocFormat::Markdown => format_markdown(docs),
        }
    }
}

impl CodeAction for GenerateDocs {
    fn id(&self) -> &'static str {
        "source.generate.docs"
    }

    fn title(&self) -> &'static str {
        "Generate documentation"
    }

    fn applicability(&self, context: &ActionContext) -> Applicability {
        if let Some(symbol) = &context.symbol_at_position {
            if matches!(
                symbol.kind,
                super::SymbolKind::Function
                    | super::SymbolKind::Method
                    | super::SymbolKind::Struct
                    | super::SymbolKind::Class
                    | super::SymbolKind::Module
            ) {
                Applicability::WhenSelected
            } else {
                Applicability::Never
            }
        } else {
            Applicability::Never
        }
    }

    fn execute(&self, context: &ActionContext) -> Result<ActionEdit> {
        let selection = context.selection.as_ref().ok_or_else(|| {
            crate::Error::InvalidInput(
                "Selection required for documentation generation".to_string(),
            )
        })?;

        let doc_comment = generate_basic_doc(selection, &context.language);

        Ok(ActionEdit {
            edits: vec![TextEdit {
                range: Range {
                    start: super::Position { line: 0, column: 0 },
                    end: super::Position { line: 0, column: 0 },
                },
                new_text: doc_comment,
            }],
            title: "Generate documentation".to_string(),
            kind: ActionKind::Source,
        })
    }
}

fn parse_llm_doc_response(
    response: &str,
    name: &str,
    element_type: ElementType,
) -> Result<GeneratedDocs> {
    let json_start = response
        .find('{')
        .ok_or_else(|| crate::Error::ParseError("No JSON found in LLM response".to_string()))?;
    let json_end = response
        .rfind('}')
        .ok_or_else(|| crate::Error::ParseError("No closing brace in LLM response".to_string()))?;

    let json_str = &response[json_start..=json_end];

    let mut docs: GeneratedDocs = serde_json::from_str(json_str)
        .map_err(|e| crate::Error::ParseError(format!("Failed to parse doc JSON: {e}")))?;

    // Ensure the element name is set correctly
    docs.element.name = name.to_string();
    docs.element.element_type = element_type;

    Ok(docs)
}

fn format_rustdoc(docs: &GeneratedDocs) -> String {
    let mut result = String::new();

    // Main documentation
    for line in docs.element.documentation.lines() {
        result.push_str("/// ");
        result.push_str(line);
        result.push('\n');
    }

    // Examples
    for example in &docs.element.examples {
        result.push_str("///\n");
        result.push_str("/// # Examples\n");
        result.push_str("///\n");
        result.push_str(&format!("/// ```\n"));

        for line in example.code.lines() {
            result.push_str("/// ");
            result.push_str(line);
            result.push('\n');
        }

        result.push_str("/// ```\n");

        if let Some(output) = &example.output {
            result.push_str("///\n");
            result.push_str("/// Output:\n");
            for line in output.lines() {
                result.push_str("/// ");
                result.push_str(line);
                result.push('\n');
            }
        }
    }

    result
}

fn format_jsdoc(docs: &GeneratedDocs) -> String {
    let mut result = String::new();
    result.push_str("/**\n");

    // Main description
    for line in docs.element.documentation.lines() {
        result.push_str(" * ");
        result.push_str(line);
        result.push('\n');
    }

    // Examples
    if !docs.element.examples.is_empty() {
        result.push_str(" *\n");
        result.push_str(" * @example\n");

        for example in &docs.element.examples {
            result.push_str(" * // ");
            result.push_str(&example.title);
            result.push('\n');

            for line in example.code.lines() {
                result.push_str(" * ");
                result.push_str(line);
                result.push('\n');
            }
        }
    }

    result.push_str(" */\n");
    result
}

fn format_python_docstring(docs: &GeneratedDocs) -> String {
    let mut result = String::new();
    result.push_str("\"\"\"\n");

    // Main description
    result.push_str(&docs.element.documentation);
    result.push('\n');

    // Examples
    if !docs.element.examples.is_empty() {
        result.push_str("\nExamples:\n");

        for example in &docs.element.examples {
            result.push_str(&format!("\n    {}:\n", example.title));

            for line in example.code.lines() {
                result.push_str("    >>> ");
                result.push_str(line);
                result.push('\n');
            }

            if let Some(output) = &example.output {
                for line in output.lines() {
                    result.push_str("    ");
                    result.push_str(line);
                    result.push('\n');
                }
            }
        }
    }

    result.push_str("\"\"\"\n");
    result
}

fn format_markdown(docs: &GeneratedDocs) -> String {
    let mut result = String::new();

    result.push_str(&format!("# {}\n\n", docs.element.name));
    result.push_str(&docs.element.documentation);
    result.push_str("\n\n");

    if !docs.element.examples.is_empty() {
        result.push_str("## Examples\n\n");

        for example in &docs.element.examples {
            result.push_str(&format!("### {}\n\n", example.title));
            result.push_str("```\n");
            result.push_str(&example.code);
            result.push_str("\n```\n\n");

            if let Some(output) = &example.output {
                result.push_str("**Output:**\n```\n");
                result.push_str(output);
                result.push_str("\n```\n\n");
            }
        }
    }

    result
}

fn generate_basic_doc(selection: &str, language: &str) -> String {
    match language {
        "rust" => format!(
            "/// TODO: Add documentation\n///\n/// # Arguments\n///\n/// * `arg` - Description\n///\n/// # Returns\n///\n/// Description of return value\n///\n/// # Examples\n///\n/// ```\n/// // {}\n/// ```\n",
            selection.lines().next().unwrap_or("").trim()
        ),
        "typescript" | "javascript" => format!(
            "/**\n * TODO: Add documentation\n * @param arg - Description\n * @returns Description of return value\n * @example\n * // {}\n */\n",
            selection.lines().next().unwrap_or("").trim()
        ),
        "python" => format!(
            "\"\"\"\nTODO: Add documentation\n\nArgs:\n    arg: Description\n\nReturns:\n    Description of return value\n\nExample:\n    {}\n\"\"\"\n",
            selection.lines().next().unwrap_or("").trim()
        ),
        _ => "// TODO: Add documentation\n".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_rustdoc() {
        let docs = GeneratedDocs {
            element: DocumentedElement {
                name: "test_function".to_string(),
                element_type: ElementType::Function,
                documentation: "This is a test function.".to_string(),
                examples: vec![CodeExample {
                    title: "Basic usage".to_string(),
                    code: "let result = test_function();".to_string(),
                    output: Some("42".to_string()),
                }],
            },
            format: DocFormat::Rustdoc,
            include_inline: true,
        };

        let formatted = format_rustdoc(&docs);
        assert!(formatted.contains("/// This is a test function"));
        assert!(formatted.contains("/// # Examples"));
        assert!(formatted.contains("let result = test_function();"));
    }

    #[test]
    fn test_format_jsdoc() {
        let docs = GeneratedDocs {
            element: DocumentedElement {
                name: "testFunction".to_string(),
                element_type: ElementType::Function,
                documentation: "This is a test function.".to_string(),
                examples: vec![CodeExample {
                    title: "Basic usage".to_string(),
                    code: "const result = testFunction();".to_string(),
                    output: None,
                }],
            },
            format: DocFormat::JsDoc,
            include_inline: true,
        };

        let formatted = format_jsdoc(&docs);
        assert!(formatted.starts_with("/**"));
        assert!(formatted.contains(" * This is a test function"));
        assert!(formatted.contains("@example"));
    }

    #[test]
    fn test_format_python_docstring() {
        let docs = GeneratedDocs {
            element: DocumentedElement {
                name: "test_function".to_string(),
                element_type: ElementType::Function,
                documentation: "This is a test function.".to_string(),
                examples: vec![CodeExample {
                    title: "Basic usage".to_string(),
                    code: "result = test_function()".to_string(),
                    output: Some("42".to_string()),
                }],
            },
            format: DocFormat::PythonDocstring,
            include_inline: true,
        };

        let formatted = format_python_docstring(&docs);
        assert!(formatted.starts_with("\"\"\""));
        assert!(formatted.contains("This is a test function"));
        assert!(formatted.contains("Examples:"));
        assert!(formatted.contains(">>>"));
    }

    #[test]
    fn test_format_markdown() {
        let docs = GeneratedDocs {
            element: DocumentedElement {
                name: "test_function".to_string(),
                element_type: ElementType::Function,
                documentation: "This is a test function.".to_string(),
                examples: vec![CodeExample {
                    title: "Basic usage".to_string(),
                    code: "result = test_function()".to_string(),
                    output: None,
                }],
            },
            format: DocFormat::Markdown,
            include_inline: false,
        };

        let formatted = format_markdown(&docs);
        assert!(formatted.starts_with("# test_function"));
        assert!(formatted.contains("This is a test function"));
        assert!(formatted.contains("## Examples"));
    }

    #[test]
    fn test_element_type_serialization() {
        let types = vec![
            ElementType::Function,
            ElementType::Struct,
            ElementType::Enum,
            ElementType::Trait,
            ElementType::Module,
        ];

        for et in types {
            let json = serde_json::to_string(&et).unwrap();
            let parsed: ElementType = serde_json::from_str(&json).unwrap();
            assert_eq!(et, parsed);
        }
    }
}
