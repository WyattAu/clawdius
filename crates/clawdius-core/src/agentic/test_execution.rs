//! Test Execution Strategy
//!
//! Defines how tests are executed when verifying generated code.
//! Users can choose between sandboxed execution (Option B) or
//! direct execution with rollback (Option C).

use serde::{Deserialize, Serialize};
use std::path::Path;

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
    /// gVisor runsc
    GVisor,
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
                SandboxBackend::GVisor => "Sandboxed (gVisor)",
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
            }
        }
    }

    async fn run_sandboxed_tests(
        &self,
        backend: SandboxBackend,
        timeout_ms: u64,
    ) -> crate::error::Result<TestResult> {
        let start = std::time::Instant::now();

        // In a real implementation, this would:
        // 1. Copy changed files to sandbox
        // 2. Set up the environment in the sandbox
        // 3. Run tests inside the sandbox
        // 4. Collect and parse results

        // Placeholder implementation
        let _ = (backend, timeout_ms);

        Ok(TestResult {
            passed: true,
            total_tests: 1,
            passed_tests: 1,
            failed_tests: 0,
            skipped_tests: 0,
            output: "Sandboxed tests passed".to_string(),
            failures: Vec::new(),
            duration_ms: start.elapsed().as_millis() as u64,
            rollback_performed: false,
        })
    }

    async fn run_direct_tests(
        &self,
        git_stash: bool,
        timeout_ms: u64,
    ) -> crate::error::Result<TestResult> {
        let start = std::time::Instant::now();
        let rollback_performed = false;

        // In a real implementation, this would:
        // 1. Optionally create git stash
        // 2. Apply changes directly
        // 3. Run tests
        // 4. Rollback on failure if git_stash is true

        let _ = (git_stash, timeout_ms);

        // Placeholder - simulate running cargo test
        let output = self.run_cargo_test().await?;

        // Parse test results
        let result = self.parse_test_output(&output, start.elapsed().as_millis() as u64);

        Ok(TestResult {
            rollback_performed,
            ..result
        })
    }

    async fn run_cargo_test(&self) -> crate::error::Result<String> {
        // In a real implementation, this would run `cargo test` and capture output
        // For now, return a placeholder
        Ok("test result: ok. 0 passed; 0 failed; 0 ignored".to_string())
    }

    fn parse_test_output(&self, output: &str, duration_ms: u64) -> TestResult {
        // Simple parser for cargo test output
        let passed = output.contains("test result: ok");

        TestResult {
            passed,
            total_tests: 0,
            passed_tests: 0,
            failed_tests: 0,
            skipped_tests: 0,
            output: output.to_string(),
            failures: Vec::new(),
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
