use super::OutputFormat;

use std::path::{Path, PathBuf};

pub(super) async fn handle_init(name: Option<String>) -> anyhow::Result<()> {
    let project_name = name.unwrap_or_else(|| {
        std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "my-project".to_string())
    });

    let clawdius_dir = std::env::current_dir()?.join(".clawdius");
    let modes_dir = clawdius_dir.join("modes");

    let config_path = clawdius_dir.join("config.toml");
    let default_mode_path = modes_dir.join("default.md");

    let default_config = format!(
        r#"[project]
name = "{project_name}"
version = "0.1.0"

[llm]
default_provider = "anthropic"

[llm.providers.anthropic]
model = "claude-sonnet-4-20250514"

[llm.providers.openai]
model = "gpt-4o"

[llm.providers.ollama]
model = "codellama"
base_url = "http://localhost:11434"

[completion]
max_tokens = 256
temperature = 0.3

[modes]
default = "default"
"#
    );

    let default_mode = r"# Default Coding Assistant

You are an expert software engineer acting as a coding assistant.

## Capabilities
- Write, review, and debug code
- Explain architectural decisions
- Suggest refactoring and improvements
- Generate tests and documentation

## Guidelines
- Follow the project's existing code style and conventions
- Provide concise, actionable responses
- When modifying code, show the minimal diff needed
- Ask clarifying questions when requirements are ambiguous
- Prefer standard library solutions over external dependencies
";

    if clawdius_dir.exists() {
        println!("  .clawdius/ directory already exists, skipping creation");
    } else {
        tokio::fs::create_dir_all(&clawdius_dir).await?;
        println!("  Created .clawdius/");
    }

    if config_path.exists() {
        println!("  .clawdius/config.toml already exists, skipping (use --config to specify)");
    } else {
        tokio::fs::write(&config_path, &default_config).await?;
        println!("  Created .clawdius/config.toml");
    }

    if modes_dir.exists() {
        println!("  .clawdius/modes/ directory already exists, skipping");
    } else {
        tokio::fs::create_dir_all(&modes_dir).await?;
        println!("  Created .clawdius/modes/");
    }

    if default_mode_path.exists() {
        println!("  .clawdius/modes/default.md already exists, skipping");
    } else {
        tokio::fs::write(&default_mode_path, default_mode).await?;
        println!("  Created .clawdius/modes/default.md");
    }

    println!();
    println!("Project \"{project_name}\" initialized successfully!");
    println!();
    println!("Next steps:");
    println!("  1. Set your API key: export ANTHROPIC_API_KEY=<your-key>");
    println!("  2. Start a chat:    clawdius chat");
    println!("  3. Or run setup:     clawdius setup");

    Ok(())
}

/// Interactive setup wizard for first-time users
pub(super) fn handle_setup(
    quick: bool,
    provider: Option<String>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use std::io::{self, Write};

    // Welcome screen
    if !quick {
        println!(
            r"
╔══════════════════════════════════════════════════════════════╗
║   ██████╗██╗      █████╗ ██╗   ██╗██████╗ ███████╗          ║
║  ██╔════╝██║     ██╔══██╗██║   ██║██╔══██╗██╔════╝          ║
║  ██║     ██║     ███████║██║   ██║██║  ██║█████╗            ║
║  ██║     ██║     ██╔══██║██║   ██║██║  ██║██╔══╝            ║
║  ╚██████╗███████╗██║  ██║╚██████╔╝██████╔╝███████╗          ║
║   ╚═════╝╚══════╝╚═╝  ╚═╝ ╚═════╝ ╚═════╝ ╚══════╝          ║
║                                                              ║
║   High-Assurance AI Coding Assistant                        ║
╚══════════════════════════════════════════════════════════════╝
"
        );
        println!(
            "Welcome to Clawdius Setup! This wizard will help you configure your AI assistant.\n"
        );
    }

    // Provider selection
    let selected_provider = if let Some(p) = provider {
        p
    } else {
        println!("📦 Step 1: Choose your LLM provider\n");
        println!("  1. Anthropic Claude (Recommended) - Best code generation, long context");
        println!("  2. OpenAI GPT-4 - Widely used, fast responses");
        println!("  3. Ollama (Local) - 100% private, no API costs");
        println!("  4. Zhipu AI - Chinese optimized, cost effective");
        println!();

        print!("Enter your choice (1-4) [default: 1]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let choice = input.trim().parse::<u8>().unwrap_or(1);
        match choice {
            2 => "openai".to_string(),
            3 => "ollama".to_string(),
            4 => "zai".to_string(),
            _ => "anthropic".to_string(),
        }
    };

    println!("\n✓ Selected provider: {selected_provider}\n");

    // API key configuration
    if selected_provider == "ollama" {
        println!("🔑 Step 2: Ollama Setup\n");
        println!("  Ollama runs models locally. Make sure you have:");
        println!("  1. Installed Ollama: https://ollama.ai");
        println!("  2. Started the server: ollama serve");
        println!("  3. Pulled a model: ollama pull codellama");
        println!();

        // Check if Ollama is running using a simple TCP check
        use std::net::TcpStream;
        let ollama_addr =
            std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "127.0.0.1:11434".to_string());

        // Remove http:// prefix if present
        let ollama_addr = ollama_addr
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .to_string();

        if TcpStream::connect_timeout(
            &ollama_addr.parse().unwrap_or_else(|_| {
                "127.0.0.1:11434"
                    .parse()
                    .unwrap_or_else(|_| std::net::SocketAddr::from(([127, 0, 0, 1], 11434)))
            }),
            std::time::Duration::from_secs(2),
        ).is_ok() {
            println!("  ✓ Ollama server is running at {ollama_addr}");
        } else {
            println!("  ⚠ Could not connect to Ollama at {ollama_addr}");
            println!("    Make sure Ollama is installed and running");
        }
    } else {
        println!("🔑 Step 2: Configure API key\n");

        let env_var = format!("{}_API_KEY", selected_provider.to_uppercase());
        let has_env_key = std::env::var(&env_var).is_ok();

        if has_env_key {
            println!("  ✓ Found {env_var} in environment");
        } else {
            println!("  You can provide your API key in one of these ways:");
            println!("  1. Environment variable: export {env_var}=your-key");
            println!("  2. Config file: clawdius auth set-key {selected_provider}");
            println!("  3. Keyring: clawdius auth set-key {selected_provider} (secure storage)");
            println!();

            print!("Enter your API key (or press Enter to skip): ");
            io::stdout().flush()?;

            let mut key_input = String::new();
            io::stdin().read_line(&mut key_input)?;
            let api_key = key_input.trim();

            if !api_key.is_empty() {
                // Store in keyring if available
                #[cfg(feature = "keyring")]
                {
                    use clawdius_core::config::KeyringStorage;
                    if let Err(e) =
                        KeyringStorage::global().set_api_key(&selected_provider, api_key)
                    {
                        eprintln!("  ⚠ Could not store in keyring: {e}");
                        eprintln!(
                            "  Set the environment variable instead: export {env_var}=<your-key>"
                        );
                    } else {
                        println!("  ✓ API key stored securely in keyring");
                    }
                }
                #[cfg(not(feature = "keyring"))]
                {
                    println!("  ⚠ Keyring feature not available");
                    println!("  Set the environment variable: export {env_var}=<your-key>");
                    let _ = api_key; // Suppress unused warning
                }
            }
        }
    }

    println!();

    // Settings preset
    if !quick {
        println!("⚙️  Step 3: Choose settings preset\n");
        println!("  1. Balanced - Good security with performance (Recommended)");
        println!("  2. Security - Maximum sandboxing, safest option");
        println!("  3. Performance - Faster execution, lighter sandboxing");
        println!("  4. Development - Minimal sandboxing, verbose output");
        println!();

        print!("Enter your choice (1-4) [default: 1]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let choice = input.trim().parse::<u8>().unwrap_or(1);
        let preset = match choice {
            2 => "Security",
            3 => "Performance",
            4 => "Development",
            _ => "Balanced",
        };
        println!("\n✓ Selected preset: {preset}\n");
    }

    // Quick start examples
    println!("📚 Quick Start Examples\n");
    println!("  Now that you're set up, try these commands:");
    println!();
    println!("  # Start an interactive chat:");
    println!("  $ clawdius chat");
    println!();
    println!("  # Generate code from a prompt:");
    println!("  $ clawdius generate \"Create a function that sorts a list\"");
    println!();
    println!("  # Analyze your codebase:");
    println!("  $ clawdius analyze src/");
    println!();
    println!("  # Watch for file changes:");
    println!("  $ clawdius watch . --auto-analyze");
    println!();

    // Final status
    let status = clawdius_core::onboarding::Onboarding::check_environment();
    match &status {
        clawdius_core::onboarding::OnboardingStatus::Complete => {
            println!("✅ Setup complete! Clawdius is ready to use.\n");
        },
        clawdius_core::onboarding::OnboardingStatus::MissingApiKey { provider } => {
            println!("⚠️  Setup incomplete: Missing API key for {provider}");
            println!("   Run: clawdius auth set-key {provider}\n");
        },
        clawdius_core::onboarding::OnboardingStatus::MissingConfig
        | clawdius_core::onboarding::OnboardingStatus::FirstRun => {
            println!("⚠️  Setup incomplete: Run 'clawdius init' to create a project\n");
        },
    }

    if output_format == OutputFormat::Json {
        let json_result = serde_json::json!({
            "status": "complete",
            "provider": selected_provider,
            "onboarding_status": format!("{:?}", status)
        });
        println!("{}", serde_json::to_string_pretty(&json_result)?);
    }

    Ok(())
}
