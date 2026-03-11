//! Lean 4 proof verifier

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use super::types::{LeanError, LeanErrorSeverity, VerificationResult};

/// Lean 4 proof verifier
#[derive(Debug)]
pub struct LeanVerifier {
    /// Path to lean binary
    lean_path: PathBuf,
    /// Path to lake binary (Lean package manager)
    lake_path: PathBuf,
    /// Timeout for verification
    timeout: Duration,
}

impl LeanVerifier {
    /// Create a new Lean verifier by discovering binaries
    pub fn new() -> Result<Self> {
        let lean_path = which::which("lean")
            .map_err(|_| Error::Config("lean binary not found in PATH".to_string()))?;

        let lake_path = which::which("lake")
            .map_err(|_| Error::Config("lake binary not found in PATH".to_string()))?;

        Ok(Self {
            lean_path,
            lake_path,
            timeout: Duration::from_secs(300),
        })
    }

    /// Create a verifier with explicit paths
    pub fn with_paths(lean_path: PathBuf, lake_path: PathBuf) -> Result<Self> {
        if !lean_path.exists() {
            return Err(Error::NotFound(format!(
                "lean binary not found: {}",
                lean_path.display()
            )));
        }
        if !lake_path.exists() {
            return Err(Error::NotFound(format!(
                "lake binary not found: {}",
                lake_path.display()
            )));
        }

        Ok(Self {
            lean_path,
            lake_path,
            timeout: Duration::from_secs(300),
        })
    }

    /// Set verification timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Check if Lean is available
    pub fn check_available(&self) -> bool {
        self.lean_path.exists() && self.lake_path.exists()
    }

    /// Get Lean version
    pub fn version(&self) -> Result<String> {
        let output = Command::new(&self.lean_path)
            .arg("--version")
            .output()
            .map_err(|e| Error::Sandbox(format!("Failed to run lean --version: {}", e)))?;

        let version = String::from_utf8_lossy(&output.stdout);
        Ok(version.lines().next().unwrap_or("unknown").to_string())
    }

    /// Verify a Lean proof file
    pub fn verify(&self, proof_path: &Path) -> Result<VerificationResult> {
        let start = Instant::now();

        if !proof_path.exists() {
            return Err(Error::NotFound(format!(
                "Proof file not found: {}",
                proof_path.display()
            )));
        }

        let parent = proof_path
            .parent()
            .ok_or_else(|| Error::InvalidInput("Proof file has no parent directory".to_string()))?;

        let output = Command::new(&self.lake_path)
            .arg("build")
            .current_dir(parent)
            .output();

        let output = match output {
            Ok(o) => o,
            Err(e) => {
                return Ok(VerificationResult::failure(
                    vec![LeanError::from_message(format!(
                        "Failed to run lake build: {}",
                        e
                    ))],
                    start.elapsed(),
                ));
            }
        };

        let result = self.parse_output(&output, start.elapsed());
        Ok(result)
    }

    /// Verify inline Lean code by writing to a temp file
    pub fn verify_inline(&self, code: &str) -> Result<VerificationResult> {
        let temp_dir = tempfile::tempdir().map_err(|e| Error::Io(e))?;

        let proof_file = temp_dir.path().join("Proof.lean");
        std::fs::write(&proof_file, code)?;

        let lakefile = temp_dir.path().join("lakefile.lean");
        std::fs::write(
            &lakefile,
            r#"import Lake
open Lake DSL

package proof_verify where
  leanOptions := #[]

@[default_target]
lean_lib Proof where
  globs := #[.oneOrMoreFiles "Proof"]
"#,
        )?;

        let result = self.verify(&proof_file)?;
        Ok(result)
    }

    /// Check a single Lean file without full lake build
    pub fn check_file(&self, proof_path: &Path) -> Result<VerificationResult> {
        let start = Instant::now();

        if !proof_path.exists() {
            return Err(Error::NotFound(format!(
                "Proof file not found: {}",
                proof_path.display()
            )));
        }

        let output = Command::new(&self.lean_path)
            .arg("--make")
            .arg(proof_path)
            .output();

        let output = match output {
            Ok(o) => o,
            Err(e) => {
                return Ok(VerificationResult::failure(
                    vec![LeanError::from_message(format!(
                        "Failed to run lean --make: {}",
                        e
                    ))],
                    start.elapsed(),
                ));
            }
        };

        let result = self.parse_output(&output, start.elapsed());
        Ok(result)
    }

    /// Parse Lean output for errors and warnings
    fn parse_output(
        &self,
        output: &std::process::Output,
        duration: Duration,
    ) -> VerificationResult {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}{}", stdout, stderr);

        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        for line in combined.lines() {
            if let Some(error) = self.parse_error_line(line) {
                match error.severity {
                    LeanErrorSeverity::Error => errors.push(error),
                    LeanErrorSeverity::Warning => warnings.push(error.message),
                    LeanErrorSeverity::Info => {}
                }
            }
        }

        let success = output.status.success() && errors.is_empty();

        VerificationResult {
            success,
            errors,
            warnings,
            duration,
            output: combined.to_string(),
        }
    }

    /// Parse a single error line from Lean output
    ///
    /// Lean error format: `file.lean:line:column: error: message`
    fn parse_error_line(&self, line: &str) -> Option<LeanError> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }

        let severity = if line.contains(": error:") {
            LeanErrorSeverity::Error
        } else if line.contains(": warning:") {
            LeanErrorSeverity::Warning
        } else if line.contains(": info:") {
            LeanErrorSeverity::Info
        } else {
            return None;
        };

        let parts: Vec<&str> = line.splitn(4, ':').collect();
        if parts.len() < 4 {
            return None;
        }

        let line_num: usize = parts[1].trim().parse().ok()?;
        let col: usize = parts[2].trim().parse().ok()?;
        let message = parts[3].trim().to_string();

        Some(LeanError {
            line: line_num,
            column: col,
            end_line: None,
            end_column: None,
            message,
            severity,
        })
    }
}

impl Default for LeanVerifier {
    fn default() -> Self {
        Self::new().expect("Failed to create LeanVerifier")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error_line() {
        let verifier = LeanVerifier::new().ok().unwrap_or_else(|| {
            let fake_path = PathBuf::from("/usr/bin/lean");
            let fake_lake = PathBuf::from("/usr/bin/lake");
            LeanVerifier::with_paths(fake_path, fake_lake).unwrap()
        });

        let line = "Proof.lean:10:5: error: unknown identifier 'foo'";
        let error = verifier.parse_error_line(line);
        assert!(error.is_some());

        let error = error.unwrap();
        assert_eq!(error.line, 10);
        assert_eq!(error.column, 5);
        assert_eq!(error.severity, LeanErrorSeverity::Error);
    }

    #[test]
    fn test_parse_warning_line() {
        let verifier = LeanVerifier::new().ok().unwrap_or_else(|| {
            let fake_path = PathBuf::from("/usr/bin/lean");
            let fake_lake = PathBuf::from("/usr/bin/lake");
            LeanVerifier::with_paths(fake_path, fake_lake).unwrap()
        });

        let line = "Proof.lean:15:3: warning: unused variable";
        let error = verifier.parse_error_line(line);
        assert!(error.is_some());

        let error = error.unwrap();
        assert_eq!(error.severity, LeanErrorSeverity::Warning);
    }
}
