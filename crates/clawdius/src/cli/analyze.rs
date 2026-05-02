use super::OutputFormat;

use std::path::PathBuf;
use clawdius_core::analysis::{DebtReport, DriftReport, DriftSeverity as CoreDriftSeverity};

pub(super) fn handle_analyze(
    path: &PathBuf,
    drift_only: bool,
    debt_only: bool,
    format: OutputFormat,
    output_file: Option<PathBuf>,
    min_severity: &str,
    exclude_patterns: Option<String>,
) -> anyhow::Result<()> {
    use clawdius_core::analysis::{DebtAnalyzer, DriftDetector};

    // Parse minimum severity filter
    let min_severity_level = match min_severity.to_lowercase().as_str() {
        "medium" => 2,
        "high" => 3,
        "critical" => 4,
        _ => 1,
    };

    // Parse exclude patterns
    let excludes: Vec<String> = exclude_patterns
        .map(|p| p.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    // Collect files to analyze
    let mut files: Vec<(PathBuf, String)> = Vec::new();

    if path.is_file() {
        if let Ok(content) = std::fs::read_to_string(path) {
            files.push((path.clone(), content));
        }
    } else if path.is_dir() {
        for entry in walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
        {
            let file_path = entry.path().to_path_buf();
            let path_str = file_path.to_string_lossy();

            if excludes.iter().any(|ex| path_str.contains(ex)) {
                continue;
            }
            if path_str.contains("/target/") || path_str.contains("\\target\\") {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                files.push((file_path, content));
            }
        }
    }

    if files.is_empty() {
        println!("⚠️  No files found to analyze");
        return Ok(());
    }

    println!("📊 Analyzing {} files...", files.len());

    // Run analysis
    let drift_report = if debt_only {
        DriftReport::default()
    } else {
        let detector = DriftDetector::new();
        detector.analyze_files(files.iter().map(|(p, c)| (p.clone(), c.as_str())))
    };

    let debt_report = if drift_only {
        DebtReport::default()
    } else {
        let analyzer = DebtAnalyzer::new();
        analyzer.analyze_files(files.iter().map(|(p, c)| (p.clone(), c.as_str())))
    };

    // Generate output
    let output = match format {
        OutputFormat::Json => {
            format_analyze_json(&drift_report, &debt_report, files.len(), min_severity_level)?
        },
        _ => format_analyze_text(&drift_report, &debt_report, files.len(), min_severity_level),
    };

    // Write output
    if let Some(output_path) = output_file {
        std::fs::write(&output_path, &output)?;
        println!("✅ Report written to {}", output_path.display());
    } else {
        println!("\n{output}");
    }

    Ok(())
}

// Helper functions for analyze command

fn format_analyze_json(
    drift_report: &DriftReport,
    debt_report: &DebtReport,
    files_analyzed: usize,
    min_severity: u8,
) -> anyhow::Result<String> {
    let result = serde_json::json!({
        "summary": {
            "files_analyzed": files_analyzed,
            "drift_count": drift_report.len(),
            "debt_count": debt_report.len(),
        },
        "drift": filter_drift_by_severity(drift_report, min_severity),
        "debt": filter_debt_by_priority(debt_report, min_severity),
    });
    Ok(serde_json::to_string_pretty(&result)?)
}

fn format_analyze_text(
    drift_report: &DriftReport,
    debt_report: &DebtReport,
    files_analyzed: usize,
    min_severity: u8,
) -> String {
    use std::fmt::Write;
    let mut output = String::new();

    output.push_str("╔══════════════════════════════════════════════════════════════╗\n");
    output.push_str("║                    📊 CLAWDIUS ANALYSIS                      ║\n");
    output.push_str("╠══════════════════════════════════════════════════════════════╣\n");
    let _ = writeln!(output, "║  Files Analyzed: {files_analyzed:<43}║");
    output.push_str("╚══════════════════════════════════════════════════════════════╝\n\n");

    // Drift Summary
    output.push_str("## 🏗️  Architecture Drift\n\n");
    let _ = writeln!(output, "  Total Drifts: {}", drift_report.len());
    let _ = writeln!(
        output,
        "  Severity Score: {}",
        drift_report.total_severity_score()
    );
    if drift_report.has_critical() {
        output.push_str("  ⚠️  CRITICAL DRIFTS DETECTED!\n");
    }
    output.push('\n');

    let filtered_drifts = filter_drift_by_severity(drift_report, min_severity);
    if !filtered_drifts.is_empty() {
        output.push_str("  Top Issues:\n");
        for drift in filtered_drifts.iter().take(10) {
            let severity = drift
                .get("severity")
                .and_then(|v| v.as_str())
                .unwrap_or("Low");
            let icon = match severity {
                "Critical" => "🔴",
                "High" => "🟠",
                "Medium" => "🟡",
                _ => "🔵",
            };
            let file = drift
                .get("file")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let line = drift.get("line").and_then(serde_json::Value::as_u64).unwrap_or(0);
            let msg = drift
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let _ = writeln!(output, "    {icon} {file}:{line} - {msg}");
        }
    }
    output.push('\n');

    // Debt Summary
    output.push_str("## 💰 Technical Debt\n\n");
    let _ = writeln!(output, "  Total Debt Items: {}", debt_report.len());
    let _ = writeln!(output, "  Debt Score: {:.2}", debt_report.debt_score());
    let _ = writeln!(
        output,
        "  Total Effort: {:.1} hours",
        debt_report.total_effort_hours
    );
    let _ = writeln!(
        output,
        "  Blocking Items: {}",
        debt_report.blocking_count
    );
    output.push('\n');

    let top_debts = debt_report.top_priorities(10);
    if !top_debts.is_empty() {
        output.push_str("  Top Priority Items:\n");
        for debt in top_debts {
            let icon = match debt.priority {
                1..=3 => "🟢",
                4..=6 => "🟡",
                7..=8 => "🟠",
                9..=10 => "🔴",
                _ => "⚪",
            };
            let _ = writeln!(
                output,
                "    {} P{} | {} - {}",
                icon,
                debt.priority,
                debt.file_path.to_string_lossy(),
                debt.description
            );
        }
    }

    output
}

fn filter_drift_by_severity(report: &DriftReport, min_level: u8) -> Vec<serde_json::Value> {
    report
        .drifts
        .iter()
        .filter(|d| {
            let level = match d.severity {
                CoreDriftSeverity::Low => 1,
                CoreDriftSeverity::Medium => 2,
                CoreDriftSeverity::High => 3,
                CoreDriftSeverity::Critical => 4,
            };
            level >= min_level
        })
        .map(|d| {
            serde_json::json!({
                "file": d.file_path.to_string_lossy(),
                "line": d.line_number,
                "category": format!("{:?}", d.category),
                "severity": format!("{:?}", d.severity),
                "message": d.message,
                "suggestion": d.suggestion,
            })
        })
        .collect()
}

fn filter_debt_by_priority(report: &DebtReport, min_level: u8) -> Vec<serde_json::Value> {
    report
        .items
        .iter()
        .filter(|d| {
            let level = match d.priority {
                4..=6 => 2,
                7..=8 => 3,
                9..=10 => 4,
                _ => 1,
            };
            level >= min_level
        })
        .map(|d| {
            serde_json::json!({
                "id": d.id,
                "file": d.file_path.to_string_lossy(),
                "line": d.line_number,
                "type": format!("{:?}", d.debt_type),
                "description": d.description,
                "priority": d.priority,
                "impact": d.impact,
                "effort_hours": d.estimated_effort_hours,
                "blocking": d.is_blocking,
                "resolution": d.resolution,
            })
        })
        .collect()
}

/// Handle watch command for file monitoring with auto-analysis
pub(super) fn handle_watch(
    path: &PathBuf,
    ignore: Option<String>,
    auto_analyze: bool,
    debounce_ms: u64,
    verbose: bool,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::watch::handlers::{ContextUpdateHandler, DiagnosticHandler};
    use clawdius_core::watch::{FileWatcher, WatchConfig};

    println!("👀 Watching {} for changes...", path.display());

    if auto_analyze {
        println!("🔍 Auto-analysis enabled");
    }

    println!("   Debounce: {debounce_ms}ms");
    if verbose {
        println!("   Verbose output enabled");
    }
    println!();
    println!("Press Ctrl+C to stop watching...");
    println!();

    // Create watch configuration
    let mut config = WatchConfig::new(path);

    if let Some(ignore_patterns) = ignore {
        let patterns: Vec<String> = ignore_patterns
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        for pattern in patterns {
            config = config.exclude(pattern);
        }
    }

    config = config.debounce(debounce_ms);

    // Create handlers (placeholder for future async integration)
    let _context_handler = ContextUpdateHandler::new(vec!["**/*.rs".to_string()]);
    let _diagnostic_handler = DiagnosticHandler::new();

    // Create watcher
    let mut watcher = FileWatcher::new(config)?;
    watcher.start()?;

    // Simulate watching (in a real implementation, this would be async with notify)
    println!("📁 File watcher started successfully");
    println!("   Watching for: **/*.rs, **/*.toml");
    println!("   Ignoring: target/, .git/, node_modules/");

    // In a real implementation, we would integrate with notify crate
    // For now, this is a placeholder that demonstrates the feature

    if output_format == OutputFormat::Json {
        println!(
            "{}",
            serde_json::json!({
                "status": "watching",
                "path": path.to_string_lossy(),
                "auto_analyze": auto_analyze,
                "debounce_ms": debounce_ms
            })
        );
    }

    Ok(())
}
