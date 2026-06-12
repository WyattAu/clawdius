use super::{load_config, OutputFormat};
use crate::cli::test_cmd::extract_function_from_code;

use clawdius_core::output::{OutputFormat as CoreOutputFormat, OutputFormatter, OutputOptions};
use std::path::PathBuf;

pub(super) async fn handle_doc(
    file: PathBuf,
    element: Option<String>,
    format: String,
    output: Option<PathBuf>,
    _inline: bool,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::actions::docs::{DocFormat, GenerateDocs};
    use std::fs;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let _formatter = OutputFormatter::new(options);

    let code = fs::read_to_string(&file)?;
    let language = file
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("txt")
        .to_string();

    let doc_format = match format.as_str() {
        "rustdoc" | "rust" => DocFormat::Rustdoc,
        "jsdoc" | "javascript" | "typescript" => DocFormat::JsDoc,
        "pydoc" | "python" => DocFormat::PythonDocstring,
        "markdown" | "md" => DocFormat::Markdown,
        _ => match language.as_str() {
            "ts" | "js" => DocFormat::JsDoc,
            "py" => DocFormat::PythonDocstring,
            _ => DocFormat::Markdown,
        },
    };

    if output_format == OutputFormat::Text {
        println!("📝 Generating documentation for: {}", file.display());
        println!("   Language: {language}");
        println!("   Format: {doc_format:?}");
        if let Some(ref elem) = element {
            println!("   Element: {elem}");
        }
        println!();
    }

    // Create LLM client
    let config = load_config(None)?;
    let llm_config = clawdius_core::llm::ResolvedLlmConfig::from_config(&config.llm, "anthropic")?;
    let llm_client = std::sync::Arc::new(clawdius_core::llm::create_provider(&llm_config)?);

    let doc_generator = GenerateDocs::new(llm_client);

    // Generate documentation based on element type
    let generated_docs = if let Some(element_name) = &element {
        // Try to parse as function first, then struct
        match async {
            // Try function
            if let Ok(func) = extract_function_from_code(&code, element_name, &language) {
                let docs = doc_generator
                    .generate_for_function(&func.name, &func.signature, &func.body, &language)
                    .await?;
                return Ok::<_, anyhow::Error>(docs);
            }

            // Fallback to basic documentation
            anyhow::bail!("Could not parse element: {element_name}")
        }
        .await
        {
            Ok(docs) => docs,
            Err(e) => {
                if output_format == OutputFormat::Text {
                    println!("⚠️  Could not generate LLM docs: {e}");
                    println!("   Falling back to basic documentation template");
                }
                return Err(e);
            },
        }
    } else {
        // Generate module-level documentation
        let exports = extract_exports(&code, &language);
        match doc_generator
            .generate_for_module(&file.display().to_string(), &code, &exports)
            .await
        {
            Ok(docs) => docs,
            Err(e) => {
                if output_format == OutputFormat::Text {
                    println!("⚠️  Could not generate module docs: {e}");
                }
                return Err(e.into());
            },
        }
    };

    // Format the documentation
    let formatted_docs = doc_generator.format_docs(&generated_docs, &language);

    // Output the documentation
    if let Some(output_path) = &output {
        fs::write(output_path, &formatted_docs)?;
        if output_format == OutputFormat::Text {
            println!("✅ Documentation written to: {}", output_path.display());
        }
    } else {
        println!("{formatted_docs}");
    }

    Ok(())
}

fn extract_exports(code: &str, language: &str) -> Vec<String> {
    let mut exports = Vec::new();

    match language {
        "rs" => {
            // Look for pub fn, pub struct, pub enum
            let fn_re = regex::Regex::new(r"pub\s+(?:async\s+)?fn\s+(\w+)").ok();
            let struct_re = regex::Regex::new(r"pub\s+struct\s+(\w+)").ok();
            let enum_re = regex::Regex::new(r"pub\s+enum\s+(\w+)").ok();

            if let Some(re) = fn_re {
                for cap in re.captures_iter(code) {
                    exports.push(format!("fn {}", &cap[1]));
                }
            }
            if let Some(re) = struct_re {
                for cap in re.captures_iter(code) {
                    exports.push(format!("struct {}", &cap[1]));
                }
            }
            if let Some(re) = enum_re {
                for cap in re.captures_iter(code) {
                    exports.push(format!("enum {}", &cap[1]));
                }
            }
        },
        "ts" | "js" => {
            // Look for export function, export class
            let fn_re = regex::Regex::new(r"export\s+(?:async\s+)?function\s+(\w+)").ok();
            let class_re = regex::Regex::new(r"export\s+class\s+(\w+)").ok();

            if let Some(re) = fn_re {
                for cap in re.captures_iter(code) {
                    exports.push(format!("function {}", &cap[1]));
                }
            }
            if let Some(re) = class_re {
                for cap in re.captures_iter(code) {
                    exports.push(format!("class {}", &cap[1]));
                }
            }
        },
        "py" => {
            // Look for def and class
            let def_re = regex::Regex::new(r"^def\s+(\w+)").ok();
            let class_re = regex::Regex::new(r"^class\s+(\w+)").ok();

            if let Some(re) = def_re {
                for cap in re.captures_iter(code) {
                    exports.push(format!("def {}", &cap[1]));
                }
            }
            if let Some(re) = class_re {
                for cap in re.captures_iter(code) {
                    exports.push(format!("class {}", &cap[1]));
                }
            }
        },
        _ => {},
    }

    exports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_exports_rust_finds_fn_struct_enum() {
        let code = "pub fn hello() {}\npub struct Foo {}\npub enum Bar { A, B }";
        let exports = extract_exports(code, "rs");
        assert!(
            exports.contains(&"fn hello".to_string()),
            "should find pub fn hello"
        );
        assert!(
            exports.contains(&"struct Foo".to_string()),
            "should find pub struct Foo"
        );
        assert!(
            exports.contains(&"enum Bar".to_string()),
            "should find pub enum Bar"
        );
        assert_eq!(exports.len(), 3);
    }

    #[test]
    fn extract_exports_typescript_finds_function_and_class() {
        let code = "export function greet() {}\nexport class MyClass {}";
        let exports = extract_exports(code, "ts");
        assert!(
            exports.contains(&"function greet".to_string()),
            "should find export function greet"
        );
        assert!(
            exports.contains(&"class MyClass".to_string()),
            "should find export class MyClass"
        );
        assert_eq!(exports.len(), 2);
    }

    #[test]
    fn extract_exports_python_finds_def_and_class() {
        let exports_def = extract_exports("def foo():\n    pass", "py");
        assert!(
            exports_def.contains(&"def foo".to_string()),
            "should find def foo"
        );

        let exports_class = extract_exports("class Bar:\n    pass", "py");
        assert!(
            exports_class.contains(&"class Bar".to_string()),
            "should find class Bar"
        );
    }

    #[test]
    fn extract_exports_unknown_language_returns_empty() {
        let code = "some code here";
        let exports = extract_exports(code, "ruby");
        assert!(
            exports.is_empty(),
            "unknown language should return empty vec"
        );
    }

    #[test]
    fn extract_exports_empty_code_returns_empty() {
        let exports = extract_exports("", "rs");
        assert!(exports.is_empty(), "empty code should return empty vec");
    }

    #[test]
    fn extract_exports_rust_async_fn_found() {
        let code = "pub async fn fetch() {}";
        let exports = extract_exports(code, "rs");
        assert!(
            exports.contains(&"fn fetch".to_string()),
            "should find pub async fn fetch"
        );
        assert_eq!(exports.len(), 1);
    }

    #[test]
    fn extract_exports_rust_no_pub_items_returns_empty() {
        let code = "fn private() {}\nstruct Internal {}";
        let exports = extract_exports(code, "rs");
        assert!(exports.is_empty(), "non-pub items should not be exported");
    }
}
