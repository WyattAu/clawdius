use super::{load_config, MetricsOutputFormat, OutputFormat};

use clawdius_core::output::{OutputFormat as CoreOutputFormat, OutputFormatter};
use std::path::PathBuf;

pub(super) async fn handle_metrics(
    format: MetricsOutputFormat,
    output: Option<PathBuf>,
    reset: bool,
    watch: bool,
    _output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::telemetry::MetricsDashboard;

    let dashboard = MetricsDashboard::new();

    if watch {
        println!("Watch mode not yet implemented. Displaying metrics once...\n");
    }

    let content = match format {
        MetricsOutputFormat::Json => dashboard.format_json()?,
        MetricsOutputFormat::Html => dashboard.format_html(),
        MetricsOutputFormat::Text => dashboard.format_terminal(),
    };

    if let Some(path) = output {
        tokio::fs::write(&path, &content).await?;
        println!("Metrics written to {}", path.display());
    } else {
        println!("{content}");
    }

    if reset {
        let m = clawdius_core::telemetry::metrics();
        m.reset();
        println!("\n✓ Metrics reset");
    }

    Ok(())
}

pub(super) fn handle_telemetry(
    enable: bool,
    disable: bool,
    enable_metrics: bool,
    enable_crash_reporting: bool,
    config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::output::{OutputOptions, TelemetryResult};
    use std::io;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let mut config = load_config(config_path.as_deref())?;

    if enable {
        config.telemetry.metrics_enabled = true;
        config.telemetry.crash_reporting = true;
        config.telemetry.performance_monitoring = true;
    }

    if disable {
        config.telemetry.metrics_enabled = false;
        config.telemetry.crash_reporting = false;
        config.telemetry.performance_monitoring = false;
    }

    if enable_metrics {
        config.telemetry.metrics_enabled = true;
    }

    if enable_crash_reporting {
        config.telemetry.crash_reporting = true;
    }

    let config_path = config_path.unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".clawdius/config.toml")
    });

    let result = match config.save(&config_path) {
        Ok(()) => TelemetryResult::success(
            config.telemetry.metrics_enabled,
            config.telemetry.crash_reporting,
            config.telemetry.performance_monitoring,
            config_path.display().to_string(),
        ),
        Err(e) => TelemetryResult::error(e.to_string()),
    };

    formatter.format_telemetry_result(&mut io::stdout(), &result)?;

    Ok(())
}
