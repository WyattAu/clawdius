use super::{OutputFormat, ModelsCommands};

#[allow(clippy::cast_precision_loss)]
pub(super) async fn handle_models(
    action: ModelsCommands,
    host: &str,
    port: u16,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::llm::providers::local::LocalLlmProvider;

    let base_url = format!("http://{host}:{port}");
    let provider = LocalLlmProvider::new(base_url, "default".to_string());

    match action {
        ModelsCommands::List => match provider.list_models().await {
            Ok(models) => {
                if models.is_empty() {
                    if output_format == OutputFormat::Json {
                        println!("[]");
                    } else {
                        println!("No models found. Pull a model with:");
                        println!("  clawdius models pull llama3.2");
                    }
                } else if output_format == OutputFormat::Json {
                    println!("{}", serde_json::to_string_pretty(&models)?);
                } else {
                    println!("Available models:\n");
                    for model in &models {
                        let size = model
                            .size
                            .map(|s| format!("{:.2} GB", s as f64 / 1_073_741_824.0))
                            .unwrap_or_default();
                        println!("  🦙 {} {}", model.name, size);
                    }
                    println!("\nTotal: {} model(s)", models.len());
                }
            },
            Err(e) => {
                if output_format == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "error": e.to_string(),
                            "hint": "Ensure Ollama is running"
                        })
                    );
                } else {
                    eprintln!("❌ Error: {e}");
                    eprintln!("\n💡 Ensure Ollama is running:");
                    eprintln!("   ollama serve");
                }
                return Err(anyhow::anyhow!("{e}"));
            },
        },

        ModelsCommands::Pull { model } => {
            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "pulling",
                        "model": model
                    })
                );
            } else {
                println!("📦 Pulling model: {model}");
                println!("   This may take a while...\n");
            }

            match provider.pull_model(&model).await {
                Ok(()) => if output_format == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "success",
                            "model": model
                        })
                    );
                } else {
                    println!("✅ Model pulled successfully: {model}");
                    println!("\nUse it with:");
                    println!("  clawdius chat -P ollama --model {model}");
                },
                Err(e) => {
                    match output_format {
                        OutputFormat::Json => {
                            println!(
                                "{}",
                                serde_json::json!({
                                    "error": e.to_string(),
                                    "model": model
                                })
                            );
                        },
                        _ => {
                            eprintln!("❌ Failed to pull model: {e}");
                        },
                    }
                    return Err(anyhow::anyhow!("{e}"));
                },
            }
        },

        ModelsCommands::Health => if matches!(provider.health_check().await, Ok(true)) { if output_format == OutputFormat::Json {
            println!(
                "{}",
                serde_json::json!({
                    "status": "healthy",
                    "host": host,
                    "port": port
                })
            );
        } else {
            println!("✅ Ollama server is healthy");
            println!("   Host: {host}:{port}");
        } } else {
            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "unhealthy",
                        "host": host,
                        "port": port
                    })
                );
            } else {
                eprintln!("❌ Ollama server is not responding");
                eprintln!("\n💡 Start Ollama with:");
                eprintln!("   ollama serve");
            }
            return Err(anyhow::anyhow!("Ollama server not responding"));
        },

        ModelsCommands::Current => {
            // This would require loading from config
            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "model": "llama3.2",
                        "provider": "ollama",
                        "note": "Configure in clawdius.toml"
                    })
                );
            } else {
                println!("Current model configuration:");
                println!("  Provider: ollama (default)");
                println!("  Model: llama3.2 (default)");
                println!("\n💡 Configure in clawdius.toml:");
                println!("   [llm.ollama]");
                println!("   model = \"mistral\"");
                println!("   base_url = \"http://localhost:11434\"");
            }
        },
    }

    Ok(())
}
