//! Test generation actions.

use super::{ActionContext, ActionEdit, ActionKind, Applicability, CodeAction, Range, TextEdit};
use crate::llm::{ChatMessage, ChatRole, LlmClient};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub signature: String,
    pub body: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub name: String,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedTests {
    pub test_file_path: String,
    pub test_cases: Vec<TestCase>,
    pub coverage_hints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub name: String,
    pub code: String,
    pub description: String,
}

pub struct GenerateTests {
    llm: Arc<dyn LlmClient>,
}

impl GenerateTests {
    pub fn new(llm: Arc<dyn LlmClient>) -> Self {
        Self { llm }
    }

    pub async fn generate_for_function(&self, func: &Function) -> Result<GeneratedTests> {
        let prompt = format!(
            r#"Generate comprehensive unit tests for the following function:

Function signature: {}
Function body:
{}

Please generate test cases that cover:
1. Normal/happy path scenarios
2. Edge cases (empty inputs, boundary values)
3. Error cases (invalid inputs)
4. Type-specific edge cases

For each test, provide:
- Test name (descriptive)
- Test code (in the same language)
- Brief description of what it tests

Format your response as JSON:
{{
  "test_cases": [
    {{
      "name": "test_<descriptive_name>",
      "code": "<test code>",
      "description": "<what this tests>"
    }}
  ],
  "coverage_hints": ["<hint1>", "<hint2>"]
}}"#,
            func.signature, func.body
        );

        let messages = vec![ChatMessage {
            role: ChatRole::User,
            content: prompt,
        }];

        let response = self.llm.chat(messages).await?;
        let tests: GeneratedTests = parse_llm_test_response(&response, &func.name)?;

        Ok(tests)
    }

    pub async fn generate_for_module(&self, module: &Module) -> Result<GeneratedTests> {
        let mut all_tests = GeneratedTests {
            test_file_path: format!("{}_test", module.name),
            test_cases: Vec::new(),
            coverage_hints: Vec::new(),
        };

        for func in &module.functions {
            let tests = self.generate_for_function(func).await?;
            all_tests.test_cases.extend(tests.test_cases);
            all_tests.coverage_hints.extend(tests.coverage_hints);
        }

        all_tests.coverage_hints.dedup();

        Ok(all_tests)
    }

    pub fn parse_function_from_selection(selection: &str, language: &str) -> Result<Function> {
        let (name, signature, parameters, return_type) = match language {
            "rust" => parse_rust_function(selection)?,
            "typescript" | "javascript" => parse_typescript_function(selection)?,
            "python" => parse_python_function(selection)?,
            _ => {
                return Err(crate::Error::UnsupportedLanguage(format!(
                    "Test generation not supported for: {}",
                    language
                )))
            }
        };

        Ok(Function {
            name,
            signature,
            body: selection.to_string(),
            parameters,
            return_type,
        })
    }
}

impl CodeAction for GenerateTests {
    fn id(&self) -> &str {
        "source.generate.tests"
    }

    fn title(&self) -> &str {
        "Generate tests"
    }

    fn applicability(&self, context: &ActionContext) -> Applicability {
        if let Some(symbol) = &context.symbol_at_position {
            if matches!(
                symbol.kind,
                super::SymbolKind::Function | super::SymbolKind::Method
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
            crate::Error::InvalidInput("Selection required for test generation".to_string())
        })?;

        let test_code = generate_basic_test(selection, &context.language);

        let test_file_name = format!(
            "{}_test.{}",
            context
                .symbol_at_position
                .as_ref()
                .map(|s| s.name.to_lowercase())
                .unwrap_or_else(|| "generated".to_string()),
            get_file_extension(&context.language)
        );

        Ok(ActionEdit {
            edits: vec![TextEdit {
                range: Range {
                    start: super::Position { line: 0, column: 0 },
                    end: super::Position { line: 0, column: 0 },
                },
                new_text: format!("// Generated tests\n{}\n", test_code),
            }],
            title: format!("Generate tests (suggestion: save to {})", test_file_name),
            kind: ActionKind::Source,
        })
    }
}

fn parse_llm_test_response(response: &str, func_name: &str) -> Result<GeneratedTests> {
    let json_start = response
        .find('{')
        .ok_or_else(|| crate::Error::ParseError("No JSON found in LLM response".to_string()))?;
    let json_end = response
        .rfind('}')
        .ok_or_else(|| crate::Error::ParseError("No closing brace in LLM response".to_string()))?;

    let json_str = &response[json_start..=json_end];

    let mut tests: GeneratedTests = serde_json::from_str(json_str)
        .map_err(|e| crate::Error::ParseError(format!("Failed to parse test JSON: {}", e)))?;

    tests.test_file_path = format!("{}_test", func_name);

    Ok(tests)
}

fn parse_rust_function(code: &str) -> Result<(String, String, Vec<Parameter>, Option<String>)> {
    let fn_pattern = r"fn\s+(\w+)\s*(?:<[^>]+>)?\s*\(([^)]*)\)(?:\s*->\s*([^{]+))?\s*\{";
    let re = regex::Regex::new(fn_pattern)
        .map_err(|e| crate::Error::ParseError(format!("Failed to compile regex: {}", e)))?;

    let caps = re
        .captures(code)
        .ok_or_else(|| crate::Error::ParseError("No function found in selection".to_string()))?;

    let name = caps[1].to_string();
    let params_str = &caps[2];
    let return_type = caps.get(3).map(|m| m.as_str().trim().to_string());

    let parameters = parse_rust_parameters(params_str)?;

    let signature = format!(
        "fn {}({}){}",
        name,
        params_str,
        return_type
            .as_ref()
            .map(|t| format!(" -> {}", t))
            .unwrap_or_default()
    );

    Ok((name, signature, parameters, return_type))
}

fn parse_rust_parameters(params_str: &str) -> Result<Vec<Parameter>> {
    if params_str.trim().is_empty() {
        return Ok(Vec::new());
    }

    let params: Vec<Parameter> = params_str
        .split(',')
        .filter_map(|param| {
            let param = param.trim();
            if param.is_empty() {
                return None;
            }

            let parts: Vec<&str> = param.split(':').collect();
            if parts.len() == 2 {
                Some(Parameter {
                    name: parts[0].trim().to_string(),
                    ty: parts[1].trim().to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(params)
}

fn parse_typescript_function(
    code: &str,
) -> Result<(String, String, Vec<Parameter>, Option<String>)> {
    let fn_pattern =
        r"(?:async\s+)?function\s+(\w+)\s*(?:<[^>]+>)?\s*\(([^)]*)\)(?:\s*:\s*([^{]+))?\s*\{";
    let re = regex::Regex::new(fn_pattern)
        .map_err(|e| crate::Error::ParseError(format!("Failed to compile regex: {}", e)))?;

    let caps = re
        .captures(code)
        .ok_or_else(|| crate::Error::ParseError("No function found in selection".to_string()))?;

    let name = caps[1].to_string();
    let params_str = &caps[2];
    let return_type = caps.get(3).map(|m| m.as_str().trim().to_string());

    let parameters = parse_typescript_parameters(params_str)?;

    let signature = format!(
        "function {}({}){}",
        name,
        params_str,
        return_type
            .as_ref()
            .map(|t| format!(": {}", t))
            .unwrap_or_default()
    );

    Ok((name, signature, parameters, return_type))
}

fn parse_typescript_parameters(params_str: &str) -> Result<Vec<Parameter>> {
    if params_str.trim().is_empty() {
        return Ok(Vec::new());
    }

    let params: Vec<Parameter> = params_str
        .split(',')
        .filter_map(|param| {
            let param = param.trim();
            if param.is_empty() {
                return None;
            }

            let parts: Vec<&str> = param.split(':').collect();
            if parts.len() >= 1 {
                let name = parts[0].trim().replace("?", "");
                let ty = parts
                    .get(1)
                    .map(|t| t.trim().to_string())
                    .unwrap_or_default();
                Some(Parameter { name, ty })
            } else {
                None
            }
        })
        .collect();

    Ok(params)
}

fn parse_python_function(code: &str) -> Result<(String, String, Vec<Parameter>, Option<String>)> {
    let fn_pattern = r"def\s+(\w+)\s*\(([^)]*)\)(?:\s*->\s*([^\:]+))?\s*:";
    let re = regex::Regex::new(fn_pattern)
        .map_err(|e| crate::Error::ParseError(format!("Failed to compile regex: {}", e)))?;

    let caps = re
        .captures(code)
        .ok_or_else(|| crate::Error::ParseError("No function found in selection".to_string()))?;

    let name = caps[1].to_string();
    let params_str = &caps[2];
    let return_type = caps.get(3).map(|m| m.as_str().trim().to_string());

    let parameters = parse_python_parameters(params_str)?;

    let signature = format!(
        "def {}({}){}",
        name,
        params_str,
        return_type
            .as_ref()
            .map(|t| format!(" -> {}", t))
            .unwrap_or_default()
    );

    Ok((name, signature, parameters, return_type))
}

fn parse_python_parameters(params_str: &str) -> Result<Vec<Parameter>> {
    if params_str.trim().is_empty() {
        return Ok(Vec::new());
    }

    let params: Vec<Parameter> = params_str
        .split(',')
        .filter_map(|param| {
            let param = param.trim();
            if param.is_empty() || param.starts_with('*') || param.starts_with("self") {
                return None;
            }

            let parts: Vec<&str> = if param.contains(':') {
                param.split(':').collect()
            } else if param.contains('=') {
                param.split('=').collect()
            } else {
                vec![param]
            };

            if !parts.is_empty() {
                Some(Parameter {
                    name: parts[0].trim().to_string(),
                    ty: parts
                        .get(1)
                        .map(|t| t.trim().to_string())
                        .unwrap_or_default(),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(params)
}

fn generate_basic_test(_selection: &str, language: &str) -> String {
    match language {
        "rust" => format!(
            r#"#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_normal_case() {{
        // TODO: Add test implementation
    }}

    #[test]
    fn test_edge_case() {{
        // TODO: Test edge cases
    }}

    #[test]
    fn test_error_case() {{
        // TODO: Test error scenarios
    }}
}}"#
        ),
        "typescript" | "javascript" => format!(
            r#"describe('function tests', () => {{
    test('normal case', () => {{
        // TODO: Add test implementation
    }});

    test('edge case', () => {{
        // TODO: Test edge cases
    }});

    test('error case', () => {{
        // TODO: Test error scenarios
    }});
}});"#
        ),
        "python" => format!(
            r#"import unittest

class TestFunction(unittest.TestCase):
    def test_normal_case(self):
        # TODO: Add test implementation
        pass

    def test_edge_case(self):
        # TODO: Test edge cases
        pass

    def test_error_case(self):
        # TODO: Test error scenarios
        pass

if __name__ == '__main__':
    unittest.main()"#
        ),
        _ => "// Test generation not supported for this language".to_string(),
    }
}

fn get_file_extension(language: &str) -> &str {
    match language {
        "rust" => "rs",
        "typescript" => "ts",
        "javascript" => "js",
        "python" => "py",
        _ => "txt",
    }
}
