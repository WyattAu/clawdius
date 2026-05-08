use super::{load_config, MemoryCommands, OutputFormat};

use std::path::PathBuf;

pub(super) fn handle_memory(
    action: MemoryCommands,
    config_path: Option<&PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::memory::ProjectMemory;

    let config = load_config(config_path.map(PathBuf::as_path))?;
    let project_root = config
        .storage
        .database_path
        .parent()
        .map(|p| p.parent().unwrap_or(p))
        .map_or_else(
            || std::env::current_dir().unwrap_or_default(),
            std::path::Path::to_path_buf,
        );

    match action {
        MemoryCommands::Show { instructions } => {
            let memory = ProjectMemory::load(&project_root)
                .unwrap_or_else(|_| ProjectMemory::new(&project_root));

            if instructions {
                println!("{}", memory.to_instructions());
            } else {
                match output_format {
                    OutputFormat::Json => {
                        let build_commands: Vec<_> = memory
                            .build_commands()
                            .iter()
                            .map(|(cmd, desc)| {
                                serde_json::json!({
                                    "command": cmd,
                                    "description": desc
                                })
                            })
                            .collect();

                        let test_commands: Vec<_> = memory
                            .test_commands()
                            .iter()
                            .map(|(cmd, desc)| {
                                serde_json::json!({
                                    "command": cmd,
                                    "description": desc
                                })
                            })
                            .collect();

                        let insights: Vec<_> = memory
                            .debug_insights()
                            .iter()
                            .map(|(issue, solution)| {
                                serde_json::json!({
                                    "issue": issue,
                                    "solution": solution
                                })
                            })
                            .collect();

                        println!(
                            "{}",
                            serde_json::json!({
                                "instructions": memory.instructions(),
                                "metadata": memory.metadata(),
                                "build_commands": build_commands,
                                "test_commands": test_commands,
                                "debug_insights": insights,
                                "learned_count": memory.learned().len()
                            })
                        );
                    },
                    OutputFormat::Text => {
                        println!("📝 Project Memory\n");

                        if !memory.instructions().is_empty() {
                            println!("## Instructions\n{}\n", memory.instructions());
                        }

                        let metadata = memory.metadata();
                        if let Some(name) = &metadata.project_name {
                            println!("**Project:** {name}");
                        }
                        if let Some(lang) = &metadata.primary_language {
                            println!("**Language:** {lang}");
                        }
                        if let Some(fw) = &metadata.framework {
                            println!("**Framework:** {fw}");
                        }

                        let build_commands = memory.build_commands();
                        if !build_commands.is_empty() {
                            println!("\n## Build Commands");
                            for (cmd, desc) in &build_commands {
                                if let Some(d) = desc {
                                    println!("  • {cmd} - {d}");
                                } else {
                                    println!("  • {cmd}");
                                }
                            }
                        }

                        let test_commands = memory.test_commands();
                        if !test_commands.is_empty() {
                            println!("\n## Test Commands");
                            for (cmd, desc) in &test_commands {
                                if let Some(d) = desc {
                                    println!("  • {cmd} - {d}");
                                } else {
                                    println!("  • {cmd}");
                                }
                            }
                        }

                        let insights = memory.debug_insights();
                        if !insights.is_empty() {
                            println!("\n## Debug Insights");
                            for (issue, solution) in &insights {
                                println!("  • Issue: {issue}");
                                println!("    Solution: {solution}");
                            }
                        }

                        println!("\n📊 {} learned entries", memory.learned().len());
                    },
                    OutputFormat::StreamJson => {
                        println!(
                            "{}",
                            serde_json::json!({
                                "type": "memory_show",
                                "instructions": memory.instructions(),
                                "learned_count": memory.learned().len()
                            })
                        );
                    },
                }
            }
        },

        MemoryCommands::Learn {
            entry_type,
            content,
            description,
        } => {
            let mut memory = ProjectMemory::load(&project_root)
                .unwrap_or_else(|_| ProjectMemory::new(&project_root));

            match entry_type.to_lowercase().as_str() {
                "build" => {
                    memory.learn_build_command(&content, description);
                },
                "test" => {
                    memory.learn_test_command(&content, description);
                },
                "debug" => {
                    let parts: Vec<&str> = content.splitn(2, '=').collect();
                    if parts.len() == 2 {
                        memory.learn_debug_insight(parts[0], parts[1]);
                    } else {
                        anyhow::bail!("Debug format: issue=solution");
                    }
                },
                "pattern" => {
                    let parts: Vec<&str> = content.splitn(2, '=').collect();
                    if parts.len() == 2 {
                        memory.learn_code_pattern(
                            parts[0],
                            parts[1],
                            description.unwrap_or_default(),
                        );
                    } else {
                        anyhow::bail!("Pattern format: name=pattern");
                    }
                },
                "preference" => {
                    let parts: Vec<&str> = content.splitn(2, '=').collect();
                    if parts.len() == 2 {
                        memory.learn_preference(parts[0], parts[1]);
                    } else {
                        anyhow::bail!("Preference format: key=value");
                    }
                },
                _ => {
                    anyhow::bail!(
                        "Unknown entry type: {entry_type}. Use: build, test, debug, pattern, preference"
                    );
                },
            }

            memory.save()?;

            match output_format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "learned",
                            "type": entry_type,
                            "content": content
                        })
                    );
                },
                OutputFormat::Text => {
                    println!("✅ Learned {entry_type} entry");
                },
                OutputFormat::StreamJson => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "type": "memory_learn",
                            "entry_type": entry_type,
                            "content": content
                        })
                    );
                },
            }
        },

        MemoryCommands::Instructions { content } => {
            let mut memory = ProjectMemory::load(&project_root)
                .unwrap_or_else(|_| ProjectMemory::new(&project_root));

            let instructions = if content == "-" {
                use std::io::{self, Read};
                let mut buffer = String::new();
                io::stdin().read_to_string(&mut buffer)?;
                buffer
            } else {
                content
            };

            memory.set_instructions(&instructions);
            memory.save()?;

            match output_format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "updated",
                            "instructions_length": instructions.len()
                        })
                    );
                },
                OutputFormat::Text => {
                    println!("✅ Project instructions updated");
                },
                OutputFormat::StreamJson => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "type": "memory_instructions",
                            "length": instructions.len()
                        })
                    );
                },
            }
        },

        MemoryCommands::List { category } => {
            let memory = ProjectMemory::load(&project_root)
                .unwrap_or_else(|_| ProjectMemory::new(&project_root));

            let entries: Vec<_> = if category == "all" {
                memory.learned().iter().collect()
            } else {
                memory.learned_by_category(&category)
            };

            match output_format {
                OutputFormat::Json => {
                    let items: Vec<_> = entries
                        .iter()
                        .map(|e| {
                            serde_json::json!({
                                "category": e.category(),
                                "entry": format!("{:?}", e)
                            })
                        })
                        .collect();

                    println!(
                        "{}",
                        serde_json::json!({
                            "category": category,
                            "count": items.len(),
                            "entries": items
                        })
                    );
                },
                OutputFormat::Text => {
                    println!("📋 {} entries in category: {}\n", entries.len(), category);

                    for entry in &entries {
                        println!("• [{}] {:?}", entry.category(), entry);
                    }
                },
                OutputFormat::StreamJson => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "type": "memory_list",
                            "category": category,
                            "count": entries.len()
                        })
                    );
                },
            }
        },

        MemoryCommands::Clear { category, yes } => {
            if !yes {
                anyhow::bail!("Use --yes to confirm clearing memory entries");
            }

            let mut memory = ProjectMemory::load(&project_root)
                .unwrap_or_else(|_| ProjectMemory::new(&project_root));

            let count = memory.learned().len();

            if category == "all" {
                memory.clear_learned();
            } else {
                memory.remove_by_category(&category);
            }

            memory.save()?;

            match output_format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "cleared",
                            "category": category,
                            "removed_count": count - memory.learned().len()
                        })
                    );
                },
                OutputFormat::Text => {
                    println!(
                        "✅ Cleared {} entries from category: {}",
                        count - memory.learned().len(),
                        category
                    );
                },
                OutputFormat::StreamJson => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "type": "memory_clear",
                            "category": category
                        })
                    );
                },
            }
        },

        MemoryCommands::Init {
            name,
            language,
            framework,
        } => {
            let mut memory = ProjectMemory::load(&project_root)
                .unwrap_or_else(|_| ProjectMemory::new(&project_root));

            let metadata = memory.metadata_mut();
            if let Some(n) = name {
                metadata.project_name = Some(n);
            }
            if let Some(l) = language {
                metadata.primary_language = Some(l);
            }
            if let Some(f) = framework {
                metadata.framework = Some(f);
            }

            // Create CLAWDIUS.md if it doesn't exist (also check CLAUDE.md for compat)
            let clawdius_md_path = project_root.join("CLAWDIUS.md");
            let claude_md_path = project_root.join("CLAUDE.md");
            let md_path = if clawdius_md_path.exists() || !claude_md_path.exists() {
                &clawdius_md_path
            } else {
                &claude_md_path
            };

            if !md_path.exists() && !clawdius_md_path.exists() {
                let mut content = String::new();

                // Add frontmatter
                content.push_str("---\n");
                if let Some(name) = &memory.metadata().project_name {
                    let _ =
                        std::fmt::Write::write_fmt(&mut content, format_args!("project: {name}"));
                    content.push('\n');
                }
                if let Some(lang) = &memory.metadata().primary_language {
                    let _ =
                        std::fmt::Write::write_fmt(&mut content, format_args!("language: {lang}"));
                    content.push('\n');
                }
                if let Some(fw) = &memory.metadata().framework {
                    let _ =
                        std::fmt::Write::write_fmt(&mut content, format_args!("framework: {fw}"));
                    content.push('\n');
                }
                content.push_str("---\n\n");

                content.push_str("# Project Instructions\n\n");
                content.push_str("Add your project-specific instructions here.\n\n");
                content.push_str("## Guidelines\n\n");
                content.push_str("- Write clear, idiomatic code\n");
                content.push_str("- Follow the project's style guide\n");
                content.push_str("- Add tests for new functionality\n");

                std::fs::write(&clawdius_md_path, content)?;
            }

            memory.save()?;

            match output_format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "initialized",
                            "memory_file": md_path.display().to_string(),
                            "metadata": memory.metadata()
                        })
                    );
                },
                OutputFormat::Text => {
                    println!("✅ Memory initialized");
                    if clawdius_md_path.exists() || !md_path.exists() {
                        println!("   Memory file: {}", clawdius_md_path.display());
                    } else {
                        println!("   Memory file: {}", claude_md_path.display());
                    }
                    println!(
                        "   Storage: {}/.clawdius/memory.json",
                        project_root.display()
                    );
                },
                OutputFormat::StreamJson => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "type": "memory_init",
                            "metadata": memory.metadata()
                        })
                    );
                },
            }
        },
    }

    Ok(())
}
