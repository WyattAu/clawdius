#![cfg(feature = "vector-db")]

use super::OutputFormat;
use std::path::{Path, PathBuf};

use clawdius_core::output::{OutputFormat as CoreOutputFormat, OutputFormatter, OutputOptions};

pub(super) async fn handle_index(
    path: Option<PathBuf>,
    watch: bool,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::output::{IndexResult, OutputOptions};
    use clawdius_core::WorkspaceIndexer;
    use std::io;

    let workspace_path =
        path.unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    if output_format == OutputFormat::Text {
        println!("Indexing workspace: {}", workspace_path.display());
    }

    let clawdius_dir = workspace_path.join(".clawdius");
    tokio::fs::create_dir_all(&clawdius_dir).await?;

    let graph_path = clawdius_dir.join("graph.db");
    let vector_path = clawdius_dir.join("vectors.lance");

    let mut indexer = WorkspaceIndexer::new(&graph_path, &vector_path).await?;

    if watch {
        if output_format == OutputFormat::Text {
            println!("Starting continuous indexing with file watching...");
            println!("Press Ctrl+C to stop.");
        }

        indexer.watch(&workspace_path)?;

        let start = std::time::Instant::now();
        let stats = indexer.index_workspace(&workspace_path).await?;
        let duration = start.elapsed();

        let result = IndexResult::success(
            workspace_path.display().to_string(),
            stats.files_indexed,
            stats.symbols_found,
            stats.references_found,
            stats.embeddings_created,
            duration.as_millis() as u64,
            stats.errors.clone(),
        );

        formatter.format_index_result(&mut io::stdout(), &result)?;

        tokio::signal::ctrl_c().await?;
        if output_format == OutputFormat::Text {
            println!("\nStopping file watcher...");
        }
    } else {
        let start = std::time::Instant::now();
        let stats = indexer.index_workspace(&workspace_path).await?;
        let duration = start.elapsed();

        let result = IndexResult::success(
            workspace_path.display().to_string(),
            stats.files_indexed,
            stats.symbols_found,
            stats.references_found,
            stats.embeddings_created,
            duration.as_millis() as u64,
            stats.errors.clone(),
        );

        formatter.format_index_result(&mut io::stdout(), &result)?;
    }

    Ok(())
}

#[cfg(feature = "vector-db")]
#[allow(dead_code)]
fn print_index_stats(stats: &IndexStats) {
    println!("\nIndexing Complete:");
    println!("  Files indexed: {}", stats.files_indexed);
    println!("  Symbols found: {}", stats.symbols_found);
    println!("  References found: {}", stats.references_found);
    println!("  Embeddings created: {}", stats.embeddings_created);
    println!("  Duration: {}ms", stats.duration_ms);

    if !stats.errors.is_empty() {
        println!("\nErrors ({}):", stats.errors.len());
        for error in &stats.errors {
            println!("  - {error}");
        }
    }
}

#[cfg(feature = "vector-db")]
pub(super) async fn handle_context(
    query: String,
    max_tokens: Option<usize>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::output::{ContextFile, ContextResult, ContextSymbol, OutputOptions};
    use clawdius_core::{ContextAggregator, WorkspaceIndexer};
    use std::io;

    let workspace_path = std::env::current_dir()?;
    let clawdius_dir = workspace_path.join(".clawdius");

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let graph_path = clawdius_dir.join("graph.db");
    let vector_path = clawdius_dir.join("vectors.lance");

    if !graph_path.exists() {
        let result =
            ContextResult::error(&query, "Workspace not indexed. Run 'clawdius index' first.");
        formatter.format_context_result(&mut io::stdout(), &result)?;
        anyhow::bail!("Workspace not indexed. Run 'clawdius index' first.");
    }

    let indexer = WorkspaceIndexer::new(&graph_path, &vector_path).await?;
    let aggregator = ContextAggregator::new(
        indexer.graph_store_arc(),
        indexer.vector_store_arc(),
        workspace_path.clone(),
    );

    let max_tokens = max_tokens.unwrap_or(50_000);

    let result = match aggregator.gather_context(&query, max_tokens).await {
        Ok(context) => {
            let files: Vec<ContextFile> = context
                .files
                .iter()
                .map(|f| ContextFile {
                    path: f.path.display().to_string(),
                    token_count: f.token_count,
                    symbols: f.symbols.clone(),
                })
                .collect();

            let symbols: Vec<ContextSymbol> = context
                .symbols
                .iter()
                .map(|s| ContextSymbol {
                    name: s.name.clone(),
                    kind: s.kind.clone(),
                    location: s.location.clone(),
                    token_count: s.token_count,
                })
                .collect();

            ContextResult::success(&query, max_tokens, context.total_tokens, files, symbols)
        },
        Err(e) => ContextResult::error(&query, e.to_string()),
    };

    formatter.format_context_result(&mut io::stdout(), &result)?;

    Ok(())
}
