use super::{OutputFormat, SkillAction};

use std::path::PathBuf;

pub(super) async fn handle_skill(action: SkillAction, output_format: OutputFormat) -> anyhow::Result<()> {
    use clawdius_core::llm::providers::LlmClient;
    use clawdius_core::skills::{SkillContext, SkillRegistry};
    use std::sync::Arc;

    match action {
        SkillAction::List => {
            let registry = SkillRegistry::new();
            registry.register_builtin_skills().await;
            let builtins = registry.list().await;

            let home_dir = std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .or_else(|| std::env::var_os("USERPROFILE").map(std::path::PathBuf::from));

            let mut user_skills: Vec<serde_json::Value> = Vec::new();
            if let Some(home) = home_dir {
                let skills_dir = home.join(".clawdius").join("skills");
                if let Ok(loaded) = registry.load_skills_from_dir(&skills_dir).await {
                    if !loaded.is_empty() {
                        let all_skills = registry.list().await;
                        let builtin_names: std::collections::HashSet<&str> =
                            builtins.iter().map(|s| s.name.as_str()).collect();
                        for skill in &all_skills {
                            if !builtin_names.contains(skill.name.as_str()) {
                                user_skills.push(serde_json::json!({
                                    "name": skill.name,
                                    "description": skill.description,
                                }));
                            }
                        }
                    }
                }
            }

            match output_format {
                OutputFormat::Json => {
                    let builtin_json: Vec<serde_json::Value> = builtins
                        .iter()
                        .map(|s| {
                            serde_json::json!({
                                "name": s.name,
                                "description": s.description,
                                "version": s.version,
                                "source": "builtin",
                            })
                        })
                        .collect();

                    println!(
                        "{}",
                        serde_json::json!({
                            "builtin_skills": builtin_json,
                            "user_skills": user_skills,
                        })
                    );
                },
                _ => {
                    if builtins.is_empty() && user_skills.is_empty() {
                        println!("No skills found.");
                        println!("Built-in skills can be used directly.");
                        println!(
                            "Add markdown skill files to ~/.clawdius/skills/ for custom skills."
                        );
                    } else {
                        if !builtins.is_empty() {
                            println!("📚 Built-in skills:");
                            for skill in &builtins {
                                println!("   {} - {}", skill.name, skill.description);
                            }
                        }
                        if !user_skills.is_empty() {
                            println!();
                            println!("📂 User skills (~/.clawdius/skills/):");
                            for skill in &user_skills {
                                let name =
                                    skill.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                                let desc = skill
                                    .get("description")
                                    .and_then(|d| d.as_str())
                                    .unwrap_or("");
                                println!("   {name} - {desc}");
                            }
                        }
                    }
                },
            }
        },
        SkillAction::Run { name, arguments } => {
            let config = clawdius_core::config::Config::load_or_default();

            let provider_name = config
                .llm
                .default_provider
                .as_deref()
                .unwrap_or("anthropic");

            let optional_llm: Option<Arc<dyn LlmClient>> =
                clawdius_core::llm::LlmConfig::from_config(&config.llm, provider_name)
                    .ok()
                    .and_then(|llm_cfg| clawdius_core::llm::create_provider(&llm_cfg).ok())
                    .map(|p| Arc::new(p) as Arc<dyn LlmClient>);

            let registry = SkillRegistry::new();
            registry.register_builtin_skills().await;

            let home_dir = std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .or_else(|| std::env::var_os("USERPROFILE").map(std::path::PathBuf::from));

            if let Some(home) = home_dir {
                let skills_dir = home.join(".clawdius").join("skills");
                let _ = registry.load_skills_from_dir(&skills_dir).await;
            }

            let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

            let mut ctx = SkillContext::new(project_root);
            if let Some(llm) = optional_llm {
                ctx = ctx.with_llm(llm);
            }

            for arg in arguments.split_whitespace() {
                if let Some((key, value)) = arg.split_once('=') {
                    ctx.add_argument(key, value);
                }
            }

            let result = registry.execute(&name, ctx).await;

            match result {
                Ok(skill_result) => if output_format == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "success": skill_result.success,
                            "output": skill_result.output,
                            "modified_files": skill_result.modified_files,
                            "duration_ms": skill_result.duration_ms,
                        })
                    );
                } else {
                    if skill_result.success {
                        println!("✅ Skill '{name}' completed successfully");
                    } else {
                        println!("❌ Skill '{name}' failed");
                    }
                    if !skill_result.output.is_empty() {
                        println!();
                        println!("{}", skill_result.output);
                    }
                    if !skill_result.modified_files.is_empty() {
                        println!();
                        println!("Files modified:");
                        for f in &skill_result.modified_files {
                            println!("  {f}");
                        }
                    }
                },
                Err(e) => match output_format {
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::json!({
                                "success": false,
                                "error": e.to_string(),
                                "skill": name,
                            })
                        );
                    },
                    _ => {
                        eprintln!("Failed to execute skill '{name}': {e}");
                    },
                },
            }
        },
    }

    Ok(())
}
