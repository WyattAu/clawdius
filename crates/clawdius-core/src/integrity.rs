//! Container image signing and verification (REQ-003).
//!
//! Provides keyless image verification using cosign + Sigstore transparency log.

use crate::error::Result;
use std::process::Stdio;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerificationResult {
    pub image_ref: String,
    pub signature_valid: bool,
    pub sbom_attached: bool,
    pub signer_identity: Option<String>,
}

pub async fn verify_image(image_ref: &str) -> Result<VerificationResult> {
    let cosign_path = which::which("cosign").map_err(|_| crate::Error::ToolExecution {
        tool: "cosign".to_string(),
        reason: "cosign is not installed. Install it from https://github.com/sigstore/cosign"
            .to_string(),
    })?;

    let output = tokio::process::Command::new(cosign_path)
        .arg("verify")
        .arg(image_ref)
        .arg("--certificate-identity-regexp=.*")
        .arg("--certificate-oidc-issuer-regexp=.*")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| crate::Error::ToolExecution {
            tool: "cosign".to_string(),
            reason: format!("failed to execute cosign: {e}"),
        })?;

    let signature_valid = output.status.success();

    let signer_identity = if signature_valid {
        parse_signer_from_stderr(&String::from_utf8_lossy(&output.stderr))
    } else {
        None
    };

    let sbom_attached = check_sbom_attached(image_ref).await;

    Ok(VerificationResult {
        image_ref: image_ref.to_string(),
        signature_valid,
        sbom_attached,
        signer_identity,
    })
}

async fn check_sbom_attached(image_ref: &str) -> bool {
    let cosign_path = match which::which("cosign") {
        Ok(p) => p,
        Err(_) => return false,
    };

    let output = tokio::process::Command::new(cosign_path)
        .arg("download")
        .arg("sbom")
        .arg(image_ref)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    match output {
        Ok(o) => o.status.success() && !o.stdout.is_empty(),
        Err(_) => false,
    }
}

fn parse_signer_from_stderr(stderr: &str) -> Option<String> {
    for line in stderr.lines() {
        if line.contains("Certificate subject") || line.contains("CommonName") {
            if let Some(rest) = line.split(':').nth(1) {
                return Some(rest.trim().to_string());
            }
        }
    }
    None
}
