use super::{OutputFormat, LangCommands};
use std::path::PathBuf;
use clawdius_core::i18n::Language;
use clawdius_core::output::OutputFormat as CoreOutputFormat;

pub(super) fn handle_lang(
    action: LangCommands,
    config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    // Note: output_format is reserved for future JSON/YAML output support
    let _output_format = CoreOutputFormat::from(output_format);

    match action {
        LangCommands::List => {
            println!("Supported languages:");
            println!();
            for lang in Language::all() {
                let current = if *lang == Language::detect() {
                    " (system)"
                } else {
                    ""
                };
                println!("  {} - {}{}", lang.code(), lang.native_name(), current);
            }
            println!();
            println!("Use 'clawdius lang set <code>' to change language.");
        },
        LangCommands::Set { code } => {
            match Language::from_code(&code) {
                Some(lang) => {
                    // Update config
                    let config_path = config_path.unwrap_or_else(|| {
                        std::env::current_dir()
                            .unwrap_or_else(|_| PathBuf::from("."))
                            .join(".clawdius")
                            .join("config.toml")
                    });

                    // Read existing config or create new
                    let mut config_content = if config_path.exists() {
                        std::fs::read_to_string(&config_path)?
                    } else {
                        String::new()
                    };

                    // Update or add language setting
                    if config_content.contains("language =") {
                        // Replace existing language line
                        let re =
                            regex::Regex::new(r"^language\s*=\s*.*$").map_err(|e| anyhow::anyhow!("regex compile error: {e}"))?;
                        config_content = re
                            .replace(&config_content, &format!("language = \"{}\"", lang.code()))
                            .to_string();
                    } else {
                        use std::fmt::Write;
                        if !config_content.trim().is_empty() {
                            let _ = writeln!(config_content, "[general]");
                        }
                        let _ = writeln!(config_content, "language = \"{}\"", lang.code());
                    }

                    // Write config
                    if let Some(parent) = config_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&config_path, &config_content)?;

                    println!(
                        "✓ Language set to: {} ({})",
                        lang.native_name(),
                        lang.code()
                    );
                    println!("  Config saved to: {}", config_path.display());
                },
                None => {
                    anyhow::bail!("Unknown language code: {code}. Supported codes: en, zh, ja, ko, de, fr, es, it, pt, ru");
                },
            }
        },
        LangCommands::Show => {
            let current = Language::detect();
            println!(
                "Current language: {} ({})",
                current.native_name(),
                current.code()
            );
            println!();
            println!("Available languages:");
            for lang in Language::all() {
                let marker = if *lang == current { " *" } else { "" };
                println!("  {} - {}{}", lang.code(), lang.native_name(), marker);
            }
        },
    }

    Ok(())
}
