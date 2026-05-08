use super::{OutputFormat, ShipAction};

pub(super) async fn handle_ship(
    action: ShipAction,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::agentic::ShipPipeline;

    match action {
        ShipAction::Checks { branch, files } => {
            let pipeline = ShipPipeline::new_default();
            let report = pipeline
                .run_pre_ship_checks(&branch, &files, true, false)
                .await;

            if output_format == OutputFormat::Json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("📋 Pre-Ship Checks — branch: {branch}");
                println!("   All passed: {}", report.all_passed);
                println!("   Checks: {} total", report.checks.len());
                for check in &report.checks {
                    let icon = if check.passed { "✅" } else { "❌" };
                    println!("   {icon} {} ({:?})", check.check_name, check.severity);
                    if !check.passed {
                        println!("      {}", check.message);
                    }
                }
                if report.all_passed {
                    println!("\n✨ All checks passed — ready to ship!");
                } else {
                    println!("\n⚠️  Some checks failed — address before shipping.");
                }
            }
        },
        ShipAction::CommitMessage {
            files,
            description,
            scope,
        } => {
            let pipeline = ShipPipeline::new_default();
            let msg = pipeline.generate_commit_message(&files, &description, scope.as_deref());

            if output_format == OutputFormat::Json {
                println!("{}", serde_json::to_string_pretty(&msg)?);
            } else {
                println!("📝 Generated commit message:");
                println!();
                println!("{msg}");
            }
        },
    }

    Ok(())
}
