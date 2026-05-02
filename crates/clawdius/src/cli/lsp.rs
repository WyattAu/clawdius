use super::{OutputFormat, LspCommands};

pub(super) async fn handle_lsp(action: LspCommands, output_format: OutputFormat) -> anyhow::Result<()> {
    use clawdius_core::lsp::{LspClient, LspClientConfig};

    match action {
        LspCommands::Start { server, args, root } => {
            // Create LSP client config
            let config = LspClientConfig::new(&server).with_args(args);

            // Show spinner for text output
            let spinner = if output_format == OutputFormat::Text {
                let mut s =
                    crate::cli_progress::Spinner::new(format!("Connecting to {server}..."));
                s.start();
                Some(s)
            } else {
                None
            };

            // Try to create and start the client
            let mut client = LspClient::new(config);

            match client.start(root.as_deref()).await {
                Ok(()) => {
                    // Stop spinner
                    if let Some(spinner) = spinner {
                        spinner.stop(Some(&format!("Connected to {server}")));
                    }

                    let capabilities = client.capabilities().await;

                    match output_format {
                        OutputFormat::Json => {
                            println!(
                                "{}",
                                serde_json::json!({
                                    "action": "start",
                                    "server": server,
                                    "status": "connected",
                                    "capabilities": capabilities.as_ref().map(|c| {
                                        serde_json::json!({
                                            "completion": c.completion_provider.is_some(),
                                            "hover": c.hover_provider.unwrap_or(false),
                                            "definition": c.definition_provider.unwrap_or(false),
                                            "references": c.references_provider.unwrap_or(false),
                                            "symbols": c.document_symbol_provider.unwrap_or(false),
                                            "code_actions": c.code_action_provider.unwrap_or(false),
                                        })
                                    })
                                })
                            );
                        },
                        OutputFormat::Text => {
                            crate::cli_progress::success(&format!(
                                "LSP server started: {server}"
                            ));
                            if let Some(r) = &root {
                                println!("   Root: {r}");
                            }
                            if let Some(caps) = capabilities {
                                println!("\n   Capabilities:");
                                // Text synchronization
                                if caps.text_document_sync.is_some() {
                                    println!("   ✓ Text Synchronization");
                                }
                                // Completions
                                if caps.completion_provider.is_some() {
                                    let triggers = caps
                                        .completion_provider
                                        .as_ref()
                                        .map(|c| c.trigger_characters.join(", "))
                                        .unwrap_or_default();
                                    if triggers.is_empty() {
                                        println!("   ✓ Completions");
                                    } else {
                                        println!("   ✓ Completions (triggers: {triggers})");
                                    }
                                }
                                // Hover
                                if caps.hover_provider.unwrap_or(false) {
                                    println!("   ✓ Hover");
                                }
                                // Go to Definition
                                if caps.definition_provider.unwrap_or(false) {
                                    println!("   ✓ Go to Definition");
                                }
                                // Find References
                                if caps.references_provider.unwrap_or(false) {
                                    println!("   ✓ Find References");
                                }
                                // Document Symbols
                                if caps.document_symbol_provider.unwrap_or(false) {
                                    println!("   ✓ Document Symbols");
                                }
                                // Workspace Symbols
                                if caps.workspace_symbol_provider.unwrap_or(false) {
                                    println!("   ✓ Workspace Symbols");
                                }
                                // Code Actions
                                if caps.code_action_provider.unwrap_or(false) {
                                    println!("   ✓ Code Actions");
                                }
                            } else {
                                println!("\n   ⚠ No capabilities reported");
                            }
                        },
                        OutputFormat::StreamJson => {
                            println!(
                                "{}",
                                serde_json::json!({
                                    "type": "lsp_start",
                                    "server": server,
                                    "status": "connected"
                                })
                            );
                        },
                    }

                    // Stop the client (for now, we start/stop per command)
                    let _ = client.stop().await;
                },
                Err(e) => match output_format {
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::json!({
                                "action": "start",
                                "server": server,
                                "status": "error",
                                "error": e.to_string()
                            })
                        );
                    },
                    OutputFormat::Text => {
                        println!("❌ Failed to start LSP server: {server}");
                        println!("   Error: {e}");
                        println!("\n   Make sure '{server}' is installed and in your PATH.");
                    },
                    OutputFormat::StreamJson => {
                        println!(
                            "{}",
                            serde_json::json!({
                                "type": "lsp_start",
                                "server": server,
                                "status": "error",
                                "error": e.to_string()
                            })
                        );
                    },
                },
            }
        },

        LspCommands::Complete { uri, line, column } => match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "action": "complete",
                        "uri": uri,
                        "position": {"line": line, "column": column},
                        "items": [],
                        "note": "Use 'clawdius lsp start' to connect to an LSP server"
                    })
                );
            },
            OutputFormat::Text => {
                println!("Completions for {uri}:{line}:{column}");
                println!("\n💡 Tip: Start an LSP server first with:");
                println!("   clawdius lsp start rust-analyzer --root file://$(pwd)");
            },
            OutputFormat::StreamJson => {
                println!(
                    "{}",
                    serde_json::json!({
                        "type": "lsp_complete",
                        "uri": uri,
                        "line": line,
                        "column": column,
                        "items": []
                    })
                );
            },
        },

        LspCommands::Hover { uri, line, column } => match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "action": "hover",
                        "uri": uri,
                        "position": {"line": line, "column": column},
                        "content": null,
                        "note": "Use 'clawdius lsp start' to connect to an LSP server"
                    })
                );
            },
            OutputFormat::Text => {
                println!("Hover at {uri}:{line}:{column}");
                println!("\n💡 Tip: Start an LSP server first with:");
                println!("   clawdius lsp start rust-analyzer --root file://$(pwd)");
            },
            OutputFormat::StreamJson => {
                println!(
                    "{}",
                    serde_json::json!({
                        "type": "lsp_hover",
                        "uri": uri,
                        "line": line,
                        "column": column,
                        "content": null
                    })
                );
            },
        },

        LspCommands::Definition { uri, line, column } => match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "action": "definition",
                        "uri": uri,
                        "position": {"line": line, "column": column},
                        "locations": [],
                        "note": "Use 'clawdius lsp start' to connect to an LSP server"
                    })
                );
            },
            OutputFormat::Text => {
                println!("Definition for {uri}:{line}:{column}");
                println!("\n💡 Tip: Start an LSP server first with:");
                println!("   clawdius lsp start rust-analyzer --root file://$(pwd)");
            },
            OutputFormat::StreamJson => {
                println!(
                    "{}",
                    serde_json::json!({
                        "type": "lsp_definition",
                        "uri": uri,
                        "line": line,
                        "column": column,
                        "locations": []
                    })
                );
            },
        },

        LspCommands::References {
            uri,
            line,
            column,
            include_declaration,
        } => match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "action": "references",
                        "uri": uri,
                        "position": {"line": line, "column": column},
                        "include_declaration": include_declaration,
                        "locations": [],
                        "note": "Use 'clawdius lsp start' to connect to an LSP server"
                    })
                );
            },
            OutputFormat::Text => {
                println!(
                    "References for {uri}:{line}:{column} (include_declaration: {include_declaration})"
                );
                println!("\n💡 Tip: Start an LSP server first with:");
                println!("   clawdius lsp start rust-analyzer --root file://$(pwd)");
            },
            OutputFormat::StreamJson => {
                println!(
                    "{}",
                    serde_json::json!({
                        "type": "lsp_references",
                        "uri": uri,
                        "line": line,
                        "column": column,
                        "locations": []
                    })
                );
            },
        },

        LspCommands::Symbols { uri } => match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "action": "symbols",
                        "uri": uri,
                        "symbols": [],
                        "note": "Use 'clawdius lsp start' to connect to an LSP server"
                    })
                );
            },
            OutputFormat::Text => {
                println!("Symbols for {uri}");
                println!("\n💡 Tip: Start an LSP server first with:");
                println!("   clawdius lsp start rust-analyzer --root file://$(pwd)");
            },
            OutputFormat::StreamJson => {
                println!(
                    "{}",
                    serde_json::json!({
                        "type": "lsp_symbols",
                        "uri": uri,
                        "symbols": []
                    })
                );
            },
        },

        LspCommands::Diagnostics { uri } => match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "action": "diagnostics",
                        "uri": uri,
                        "diagnostics": [],
                        "note": "Use 'clawdius lsp start' to connect to an LSP server"
                    })
                );
            },
            OutputFormat::Text => {
                println!("Diagnostics for {uri}");
                println!("\n💡 Tip: Start an LSP server first with:");
                println!("   clawdius lsp start rust-analyzer --root file://$(pwd)");
            },
            OutputFormat::StreamJson => {
                println!(
                    "{}",
                    serde_json::json!({
                        "type": "lsp_diagnostics",
                        "uri": uri,
                        "diagnostics": []
                    })
                );
            },
        },

        LspCommands::CodeActions {
            uri,
            start_line,
            start_column,
            end_line,
            end_column,
        } => match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "action": "code_actions",
                        "uri": uri,
                        "range": {
                            "start": {"line": start_line, "column": start_column},
                            "end": {"line": end_line, "column": end_column}
                        },
                        "actions": [],
                        "note": "Use 'clawdius lsp start' to connect to an LSP server"
                    })
                );
            },
            OutputFormat::Text => {
                println!(
                    "Code actions for {uri} ({start_line}:{start_column}-{end_line}:{end_column})"
                );
                println!("\n💡 Tip: Start an LSP server first with:");
                println!("   clawdius lsp start rust-analyzer --root file://$(pwd)");
            },
            OutputFormat::StreamJson => {
                println!(
                    "{}",
                    serde_json::json!({
                        "type": "lsp_code_actions",
                        "uri": uri,
                        "range": {
                            "start": {"line": start_line, "column": start_column},
                            "end": {"line": end_line, "column": end_column}
                        },
                        "actions": []
                    })
                );
            },
        },
    }

    Ok(())
}
