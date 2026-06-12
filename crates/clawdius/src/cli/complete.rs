use super::{load_config, OutputFormat};

use std::path::PathBuf;

pub(super) async fn handle_complete(
    file: String,
    line: u32,
    character: u32,
    language: Option<String>,
    provider: String,
    model: Option<String>,
    config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::completions::{
        CompletionProviderTrait, CompletionRequest, InlineCompletionProvider, LlmCompletionConfig,
    };
    use clawdius_core::llm::{create_provider, ResolvedLlmConfig};
    use clawdius_core::lsp::Position;

    let content = std::fs::read_to_string(&file)
        .map_err(|e| anyhow::anyhow!("Failed to read file {file}: {e}"))?;

    let language = language.unwrap_or_else(|| {
        std::path::Path::new(&file)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("text")
            .to_string()
    });

    let config = load_config(config_path.as_deref())?;
    let mut llm_config = ResolvedLlmConfig::from_config(&config.llm, &provider)?;
    if let Some(m) = model {
        llm_config.model = m;
    }

    // Create provider
    let llm_provider = create_provider(&llm_config)?;
    let llm_arc = std::sync::Arc::new(llm_provider);

    // Create completion provider
    let completion_config = LlmCompletionConfig::default();
    let completion_provider = InlineCompletionProvider::new(llm_arc, completion_config);

    // Create request
    let request = CompletionRequest::new(&content, Position::new(line, character), &language)
        .with_file_path(&file);

    if output_format == OutputFormat::Json {
        match completion_provider.complete(&request).await {
            Ok(response) => {
                println!("{}", serde_json::to_string_pretty(&response)?);
            },
            Err(e) => {
                println!(
                    "{}",
                    serde_json::json!({
                        "error": e.to_string()
                    })
                );
            },
        }
    } else {
        println!("🔍 Requesting completion from {provider}...");
        println!("   File: {file}:{line}");
        println!("   Language: {language}\n");

        match completion_provider.complete(&request).await {
            Ok(response) => {
                if response.text.is_empty() {
                    println!("💡 No completion available");
                } else {
                    println!(
                        "✨ Completion (confidence: {:.0}%):",
                        response.confidence * 100.0
                    );
                    println!();
                    println!("    {}", response.text.replace('\n', "\n    "));

                    if !response.alternatives.is_empty() {
                        println!("\n📚 Alternatives:");
                        for (i, alt) in response.alternatives.iter().enumerate() {
                            println!("  {}. {}", i + 1, alt.text.lines().next().unwrap_or(""));
                        }
                    }
                }
            },
            Err(e) => {
                eprintln!("❌ Completion failed: {e}");
                eprintln!("\n💡 Ensure your LLM provider is configured and accessible");
            },
        }
    }

    Ok(())
}
