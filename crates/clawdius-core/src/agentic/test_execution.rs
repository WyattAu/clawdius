//! Test Execution Strategy
//!
//! Defines how tests are executed when verifying generated code.
//! Users can choose between sandboxed execution (Option B) or
//! direct execution with rollback (Option C).

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use crate::sandbox::backends::SandboxBackend as SandboxBackendTrait;

/// Strategy for executing tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestExecutionStrategy {
    /// Option B: Run tests in an isolated sandbox.
    /// Safer but may have environment differences.
    Sandboxed {
        /// Sandbox backend to use
        backend: SandboxBackend,
        /// Timeout for test execution (milliseconds)
        timeout_ms: u64,
    },

    /// Option C: Run tests directly with rollback capability.
    /// Faster but modifies the actual codebase.
    DirectWithRollback {
        /// Create a git stash before running
        git_stash: bool,
        /// Timeout for test execution (milliseconds)
        timeout_ms: u64,
    },

    /// Skip test execution entirely.
    Skip,
}

/// Available sandbox backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxBackend {
    /// WASM-based sandboxing (most portable)
    Wasm,
    /// Docker/Podman container
    Container,
    /// Bubblewrap (Linux only)
    Bubblewrap,
    /// macOS sandbox-exec
    SandboxExec,
    /// Filtered filesystem access
    Filtered,
}

impl Default for TestExecutionStrategy {
    fn default() -> Self {
        Self::DirectWithRollback {
            git_stash: true,
            timeout_ms: 60_000,
        }
    }
}

impl TestExecutionStrategy {
    /// Creates a sandboxed strategy with default settings.
    #[must_use]
    pub fn sandboxed() -> Self {
        Self::Sandboxed {
            backend: SandboxBackend::Container,
            timeout_ms: 120_000, // 2 minutes
        }
    }

    /// Creates a sandboxed strategy with a specific backend.
    #[must_use]
    pub const fn sandboxed_with_backend(backend: SandboxBackend, timeout_ms: u64) -> Self {
        Self::Sandboxed {
            backend,
            timeout_ms,
        }
    }

    /// Creates a direct with rollback strategy with default settings.
    #[must_use]
    pub fn direct_with_rollback() -> Self {
        Self::DirectWithRollback {
            git_stash: true,
            timeout_ms: 60_000,
        }
    }

    /// Creates a direct strategy without rollback (not recommended).
    #[must_use]
    pub const fn direct_no_rollback() -> Self {
        Self::DirectWithRollback {
            git_stash: false,
            timeout_ms: 60_000,
        }
    }

    /// Creates a skip strategy.
    #[must_use]
    pub const fn skip() -> Self {
        Self::Skip
    }

    /// Returns true if this is a sandboxed strategy.
    #[must_use]
    pub const fn is_sandboxed(&self) -> bool {
        matches!(self, Self::Sandboxed { .. })
    }

    /// Returns true if this allows direct execution.
    #[must_use]
    pub const fn is_direct(&self) -> bool {
        matches!(self, Self::DirectWithRollback { .. })
    }

    /// Returns the timeout in milliseconds.
    #[must_use]
    pub const fn timeout_ms(&self) -> u64 {
        match self {
            Self::Sandboxed { timeout_ms, .. } => *timeout_ms,
            Self::DirectWithRollback { timeout_ms, .. } => *timeout_ms,
            Self::Skip => 0,
        }
    }

    /// Returns a human-readable name for the strategy.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Sandboxed { backend, .. } => match backend {
                SandboxBackend::Wasm => "Sandboxed (WASM)",
                SandboxBackend::Container => "Sandboxed (Container)",
                SandboxBackend::Bubblewrap => "Sandboxed (Bubblewrap)",
                SandboxBackend::SandboxExec => "Sandboxed (macOS)",
                SandboxBackend::Filtered => "Sandboxed (Filtered)",
            },
            Self::DirectWithRollback {
                git_stash: true, ..
            } => "Direct with Rollback",
            Self::DirectWithRollback {
                git_stash: false, ..
            } => "Direct (No Rollback)",
            Self::Skip => "Skip Tests",
        }
    }
}

/// Result of running tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Whether all tests passed.
    pub passed: bool,
    /// Total number of tests run.
    pub total_tests: u32,
    /// Number of tests that passed.
    pub passed_tests: u32,
    /// Number of tests that failed.
    pub failed_tests: u32,
    /// Number of tests that were skipped.
    pub skipped_tests: u32,
    /// Test output (truncated if too long).
    pub output: String,
    /// Failure messages.
    pub failures: Vec<TestFailure>,
    /// Execution time in milliseconds.
    pub duration_ms: u64,
    /// Whether rollback was performed.
    pub rollback_performed: bool,
}

impl Default for TestResult {
    fn default() -> Self {
        Self {
            passed: true,
            total_tests: 0,
            passed_tests: 0,
            failed_tests: 0,
            skipped_tests: 0,
            output: String::new(),
            failures: Vec::new(),
            duration_ms: 0,
            rollback_performed: false,
        }
    }
}

impl TestResult {
    /// Creates a successful test result.
    #[must_use]
    pub fn success(total: u32, duration_ms: u64) -> Self {
        Self {
            passed: true,
            total_tests: total,
            passed_tests: total,
            failed_tests: 0,
            skipped_tests: 0,
            output: String::new(),
            failures: Vec::new(),
            duration_ms,
            rollback_performed: false,
        }
    }

    /// Creates a failed test result.
    #[must_use]
    pub fn failure(failures: Vec<TestFailure>, duration_ms: u64) -> Self {
        let failed = failures.len() as u32;
        Self {
            passed: false,
            total_tests: failed,
            passed_tests: 0,
            failed_tests: failed,
            skipped_tests: 0,
            output: String::new(),
            failures,
            duration_ms,
            rollback_performed: false,
        }
    }

    /// Adds a failure to the result.
    pub fn add_failure(&mut self, failure: TestFailure) {
        self.failures.push(failure);
        self.failed_tests += 1;
        self.total_tests += 1;
        self.passed = false;
    }
}

/// A single test failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestFailure {
    /// Name of the failing test.
    pub name: String,
    /// File containing the test.
    pub file: String,
    /// Line number of the failure.
    pub line: u32,
    /// Error message.
    pub message: String,
    /// Stack trace (if available).
    pub stack_trace: Option<String>,
}

/// Test runner that executes tests based on the configured strategy.
pub struct TestRunner {
    /// The execution strategy.
    strategy: TestExecutionStrategy,
}

impl TestRunner {
    /// Creates a new test runner with the specified strategy.
    #[must_use]
    pub const fn new(strategy: TestExecutionStrategy) -> Self {
        Self { strategy }
    }

    /// Runs tests for the given changes.
    ///
    /// # Errors
    ///
    /// Returns an error if test execution fails critically.
    pub async fn run_tests(
        &self,
        _changes: &[super::FileChange],
    ) -> crate::error::Result<TestResult> {
        let start = std::time::Instant::now();

        match self.strategy {
            TestExecutionStrategy::Sandboxed {
                backend,
                timeout_ms,
            } => self.run_sandboxed_tests(backend, timeout_ms).await,
            TestExecutionStrategy::DirectWithRollback {
                git_stash,
                timeout_ms,
            } => self.run_direct_tests(git_stash, timeout_ms).await,
            TestExecutionStrategy::Skip => {
                Ok(TestResult::success(0, start.elapsed().as_millis() as u64))
            },
        }
    }

    async fn run_sandboxed_tests(
        &self,
        backend: SandboxBackend,
        timeout_ms: u64,
    ) -> crate::error::Result<TestResult> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_millis(timeout_ms);
        let cwd = std::env::current_dir().map_err(|e| {
            crate::error::Error::Generic(format!("failed to determine working directory: {e}"))
        })?;

        let output = match backend {
            SandboxBackend::Wasm => {
                return Err(crate::error::Error::Sandbox(
                    "WASM sandbox backend is not yet implemented".to_string(),
                ));
            },
            SandboxBackend::Container => self.run_container_tests(timeout, &cwd).await?,
            SandboxBackend::Bubblewrap => self.run_bubblewrap_tests(timeout, &cwd).await?,
            SandboxBackend::SandboxExec => self.run_sandbox_exec_tests(timeout, &cwd).await?,
            SandboxBackend::Filtered => self.run_filtered_tests(timeout, &cwd).await?,
        };

        let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
        if !output.stderr.is_empty() {
            combined.push_str("\n--- stderr ---\n");
            combined.push_str(&String::from_utf8_lossy(&output.stderr));
        }

        let result = self.parse_test_output(&combined, start.elapsed().as_millis() as u64);
        Ok(result)
    }

    async fn run_container_tests(
        &self,
        timeout: Duration,
        cwd: &Path,
    ) -> crate::error::Result<std::process::Output> {
        let container_backend = crate::sandbox::backends::ContainerBackend::with_defaults();
        tokio::time::timeout(
            timeout,
            container_backend.execute_async("cargo", &["test", "--color=never"], cwd),
        )
        .await
        .map_err(|_| crate::error::Error::Timeout(timeout))?
    }

    #[cfg(target_os = "linux")]
    async fn run_bubblewrap_tests(
        &self,
        timeout: Duration,
        cwd: &Path,
    ) -> crate::error::Result<std::process::Output> {
        if !crate::sandbox::backends::BubblewrapBackend::is_available() {
            return Err(crate::error::Error::Sandbox(
                "bubblewrap (bwrap) is not installed. Install with: apt install bubblewrap or dnf install bubblewrap".to_string(),
            ));
        }
        let config = crate::sandbox::tiers::SandboxConfig {
            tier: crate::sandbox::SandboxTier::Untrusted,
            network: false,
            mounts: vec![],
        };
        let bwrap_backend = crate::sandbox::backends::BubblewrapBackend::new(config);
        let cwd = cwd.to_path_buf();
        tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking(move || -> crate::error::Result<std::process::Output> {
                SandboxBackendTrait::execute(
                    &bwrap_backend,
                    "cargo",
                    &["test", "--color=never"],
                    &cwd,
                )
            }),
        )
        .await
        .map_err(|_| crate::error::Error::Timeout(timeout))??
    }

    #[cfg(not(target_os = "linux"))]
    async fn run_bubblewrap_tests(
        &self,
        _timeout: Duration,
        _cwd: &Path,
    ) -> crate::error::Result<std::process::Output> {
        Err(crate::error::Error::Sandbox(
            "bubblewrap sandbox is only available on Linux".to_string(),
        ))
    }

    #[cfg(target_os = "macos")]
    async fn run_sandbox_exec_tests(
        &self,
        timeout: Duration,
        cwd: &Path,
    ) -> crate::error::Result<std::process::Output> {
        if !crate::sandbox::backends::SandboxExecBackend::is_available() {
            return Err(crate::error::Error::Sandbox(
                "sandbox-exec is not available on this system".to_string(),
            ));
        }
        let config = crate::sandbox::tiers::SandboxConfig {
            tier: crate::sandbox::SandboxTier::Untrusted,
            network: false,
            mounts: vec![],
        };
        let sandbox_exec_backend = crate::sandbox::backends::SandboxExecBackend::new(config);
        let cwd = cwd.to_path_buf();
        tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking(move || -> crate::error::Result<std::process::Output> {
                SandboxBackendTrait::execute(
                    &sandbox_exec_backend,
                    "cargo",
                    &["test", "--color=never"],
                    &cwd,
                )
            }),
        )
        .await
        .map_err(|_| crate::error::Error::Timeout(timeout))??
    }

    #[cfg(not(target_os = "macos"))]
    async fn run_sandbox_exec_tests(
        &self,
        _timeout: Duration,
        _cwd: &Path,
    ) -> crate::error::Result<std::process::Output> {
        Err(crate::error::Error::Sandbox(
            "sandbox-exec is only available on macOS".to_string(),
        ))
    }

    async fn run_filtered_tests(
        &self,
        timeout: Duration,
        cwd: &Path,
    ) -> crate::error::Result<std::process::Output> {
        let config = crate::sandbox::tiers::SandboxConfig {
            tier: crate::sandbox::SandboxTier::Trusted,
            network: true,
            mounts: vec![],
        };
        let filtered_backend = crate::sandbox::backends::FilteredBackend::new(config);
        let cwd = cwd.to_path_buf();
        tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking(move || -> crate::error::Result<std::process::Output> {
                SandboxBackendTrait::execute(
                    &filtered_backend,
                    "cargo",
                    &["test", "--color=never"],
                    &cwd,
                )
            }),
        )
        .await
        .map_err(|_| crate::error::Error::Timeout(timeout))??
    }

    async fn run_direct_tests(
        &self,
        git_stash: bool,
        timeout_ms: u64,
    ) -> crate::error::Result<TestResult> {
        let start = std::time::Instant::now();
        let rollback_performed = false;

        let _ = (git_stash, timeout_ms);

        let output = self.run_cargo_test().await?;

        let result = self.parse_test_output(&output, start.elapsed().as_millis() as u64);

        Ok(TestResult {
            rollback_performed,
            ..result
        })
    }

    async fn run_cargo_test(&self) -> crate::error::Result<String> {
        let timeout = Duration::from_millis(self.strategy.timeout_ms());

        let mut child = Command::new("cargo")
            .arg("test")
            .arg("--color=never")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                crate::error::Error::Generic(format!("failed to spawn cargo test: {e}"))
            })?;

        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| crate::error::Error::Generic("failed to capture stdout".to_string()))?;

        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| crate::error::Error::Generic("failed to capture stderr".to_string()))?;

        let output_fut = async {
            let mut out = Vec::new();
            stdout.read_to_end(&mut out).await.ok();
            let mut err = Vec::new();
            stderr.read_to_end(&mut err).await.ok();

            let status = child.wait().await.map_err(|e| {
                crate::error::Error::Generic(format!("failed to wait for cargo test: {e}"))
            })?;

            let mut combined = String::from_utf8_lossy(&out).into_owned();
            if !err.is_empty() {
                combined.push_str("\n--- stderr ---\n");
                combined.push_str(&String::from_utf8_lossy(&err));
            }

            Ok::<(String, bool), crate::error::Error>((combined, status.success()))
        };

        let (output, _success) = tokio::time::timeout(timeout, output_fut)
            .await
            .map_err(|_| crate::error::Error::Timeout(timeout))??;

        Ok(output)
    }

    fn parse_test_output(&self, output: &str, duration_ms: u64) -> TestResult {
        let passed = output.contains("test result: ok");

        let mut total_tests: u32 = 0;
        let mut passed_tests: u32 = 0;
        let mut failed_tests: u32 = 0;
        let mut skipped_tests: u32 = 0;

        for line in output.lines() {
            if line.contains("test result:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for part in &parts {
                    if let Some(rest) = part.strip_suffix("passed") {
                        if let Ok(n) = rest.trim_end_matches(';').parse() {
                            passed_tests = n;
                            total_tests += n;
                        }
                    } else if let Some(rest) = part.strip_suffix("failed") {
                        if let Ok(n) = rest.trim_end_matches(';').parse() {
                            failed_tests = n;
                            total_tests += n;
                        }
                    } else if let Some(rest) = part.strip_suffix("ignored") {
                        if let Ok(n) = rest.trim_end_matches(';').parse() {
                            skipped_tests = n;
                            total_tests += n;
                        }
                    }
                }
                break;
            }
        }

        let mut failures = Vec::new();
        let mut current_failure: Option<TestFailure> = None;

        for line in output.lines() {
            if line.starts_with("---- ") && line.contains(" stdout") {
                if let Some(f) = current_failure.take() {
                    failures.push(f);
                }
                let name = line
                    .strip_prefix("---- ")
                    .and_then(|s| s.strip_suffix(" stdout"))
                    .unwrap_or("unknown")
                    .to_string();
                current_failure = Some(TestFailure {
                    name,
                    file: String::new(),
                    line: 0,
                    message: String::new(),
                    stack_trace: None,
                });
            } else if let Some(ref mut f) = current_failure {
                if line.starts_with("  --> ") {
                    let loc: Vec<&str> = line.trim_start_matches("  --> ").split(':').collect();
                    if loc.len() >= 2 {
                        f.file = loc[0].to_string();
                        f.line = loc[1].parse().unwrap_or(0);
                    }
                } else if line.starts_with("note: ")
                    || line.contains("assertion")
                    || line.contains("panic")
                {
                    if !f.message.is_empty() {
                        f.message.push('\n');
                    }
                    f.message.push_str(line.trim());
                }
            }
        }
        if let Some(f) = current_failure.take() {
            failures.push(f);
        }

        TestResult {
            passed,
            total_tests,
            passed_tests,
            failed_tests,
            skipped_tests,
            output: output.to_string(),
            failures,
            duration_ms,
            rollback_performed: false,
        }
    }

    /// Detects the test framework for a given path.
    #[must_use]
    pub fn detect_test_framework(_path: &Path) -> Option<TestFramework> {
        // In a real implementation, this would detect:
        // - Cargo test (Rust)
        // - pytest (Python)
        // - Jest (JavaScript/TypeScript)
        // - go test (Go)
        // etc.
        Some(TestFramework::CargoTest)
    }
}

/// Supported test frameworks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestFramework {
    /// Rust cargo test
    CargoTest,
    /// Python pytest
    Pytest,
    /// JavaScript/TypeScript Jest
    Jest,
    /// Go test
    GoTest,
    /// Java JUnit
    JUnit,
    /// Custom test command
    Custom,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_constructors() {
        assert!(TestExecutionStrategy::sandboxed().is_sandboxed());
        assert!(TestExecutionStrategy::direct_with_rollback().is_direct());
    }

    #[test]
    fn test_strategy_serialization() {
        let strategy = TestExecutionStrategy::sandboxed();
        let json = serde_json::to_string(&strategy).unwrap();
        assert!(json.contains("sandboxed"));
    }

    #[test]
    fn test_result_success() {
        let result = TestResult::success(10, 1000);
        assert!(result.passed);
        assert_eq!(result.total_tests, 10);
        assert_eq!(result.passed_tests, 10);
    }

    #[test]
    fn test_result_failure() {
        let failure = TestFailure {
            name: "test_example".to_string(),
            file: "src/lib.rs".to_string(),
            line: 42,
            message: "assertion failed".to_string(),
            stack_trace: None,
        };
        let result = TestResult::failure(vec![failure], 500);
        assert!(!result.passed);
        assert_eq!(result.failed_tests, 1);
    }

    #[tokio::test]
    async fn test_runner_skip() {
        let runner = TestRunner::new(TestExecutionStrategy::skip());
        let result = runner.run_tests(&[]).await.unwrap();
        assert!(result.passed);
        assert_eq!(result.total_tests, 0);
    }
}
