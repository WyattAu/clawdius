use super::OutputFormat;

use std::path::{Path, PathBuf};
use clawdius_core::output::{OutputFormat as CoreOutputFormat, OutputFormatter, OutputOptions};
use clawdius_core::proof::LeanVerifier;

pub(super) fn handle_verify(
    proof: &Path,
    lean_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::output::{OutputOptions, ProofError, VerifyResult};
    use std::io;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let verifier = match lean_path {
        Some(path) => {
            let lake_path = path
                .parent()
                .map_or_else(|| path.clone(), |p| p.join("lake"));
            LeanVerifier::with_paths(path, lake_path)?
        },
        None => LeanVerifier::new()?,
    };

    if !verifier.check_available() {
        let result = VerifyResult::failure(
            proof.display().to_string(),
            0,
            vec![ProofError {
                line: 0,
                column: 0,
                message: "Lean binaries not found. Please install Lean 4 and ensure 'lean' and 'lake' are in PATH.".to_string(),
            }],
            vec![],
        );
        formatter.format_verify_result(&mut io::stdout(), &result)?;
        anyhow::bail!("Lean binaries not found");
    }

    if output_format == OutputFormat::Text {
        println!("Lean version: {}", verifier.version()?);
        println!();
    }

    let start = std::time::Instant::now();
    let verification_result = verifier.verify(proof)?;
    let duration = start.elapsed();

    let result = if verification_result.success {
        VerifyResult::success(proof.display().to_string(), duration.as_millis() as u64)
    } else {
        let errors: Vec<ProofError> = verification_result
            .errors
            .iter()
            .map(|e| ProofError {
                line: e.line,
                column: e.column,
                message: e.message.clone(),
            })
            .collect();

        VerifyResult::failure(
            proof.display().to_string(),
            duration.as_millis() as u64,
            errors,
            verification_result.warnings,
        )
    };

    formatter.format_verify_result(&mut io::stdout(), &result)?;

    Ok(())
}
