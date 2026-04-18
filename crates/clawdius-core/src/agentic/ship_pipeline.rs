//! Ship Pipeline — Automated shipping with safety checks and benchmarks
//!
//! Provides automated code shipping workflow:
//! - Branch safety checks (don't ship to main without PR)
//! - Auto-generated commit messages via LLM
//! - Canary deployment hooks
//! - Benchmark integration (pre/post ship comparison)
//!
//! # Ship Flow
//!
//! ```text
//! 1. Pre-ship checks (branch safety, test pass, review approval)
//! 2. Generate commit message (LLM or conventional commits)
//! 3. Create git commit
//! 4. Optional: push to remote
//! 5. Optional: create PR
//! 6. Post-ship: run benchmarks, record metrics
//! ```

use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

// ─── Branch Safety ──────────────────────────────────────────────────────────

/// Protection rules for branches.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BranchProtection {
    /// No protection — direct push allowed
    None,
    /// Require tests to pass before shipping
    RequireTestsPass,
    /// Require review approval before shipping
    RequireReviewApproval,
    /// Full protection: tests + review + no direct push to main
    Full,
}

impl Default for BranchProtection {
    fn default() -> Self {
        Self::RequireTestsPass
    }
}

/// A branch rule mapping branch patterns to protection levels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchRule {
    /// Branch name pattern (exact match or glob-like prefix)
    pub pattern: String,
    /// Protection level for this branch
    pub protection: BranchProtection,
    /// Whether to auto-create a PR instead of direct push
    pub require_pr: bool,
}

impl BranchRule {
    pub fn main() -> Self {
        Self {
            pattern: "main".to_string(),
            protection: BranchProtection::Full,
            require_pr: true,
        }
    }

    pub fn develop() -> Self {
        Self {
            pattern: "develop".to_string(),
            protection: BranchProtection::RequireTestsPass,
            require_pr: false,
        }
    }

    pub fn feature() -> Self {
        Self {
            pattern: "feature/".to_string(),
            protection: BranchProtection::None,
            require_pr: false,
        }
    }

    /// Check if a branch name matches this rule's pattern.
    pub fn matches(&self, branch: &str) -> bool {
        if self.pattern == "*" {
            return true;
        }
        if self.pattern.ends_with('/') {
            branch.starts_with(&self.pattern)
        } else {
            branch == self.pattern
        }
    }
}

// ─── Commit Message Generation ─────────────────────────────────────────────

/// Strategy for generating commit messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CommitMessageStrategy {
    /// Use conventional commit format (feat:, fix:, etc.)
    ConventionalCommits,
    /// Use LLM to generate a descriptive commit message
    LlmGenerated,
    /// Use a custom template
    CustomTemplate(String),
}

impl Default for CommitMessageStrategy {
    fn default() -> Self {
        Self::ConventionalCommits
    }
}

/// A conventional commit type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConventionalCommitType {
    Feat,
    Fix,
    Docs,
    Style,
    Refactor,
    Perf,
    Test,
    Build,
    Ci,
    Chore,
    Revert,
}

impl ConventionalCommitType {
    pub fn from_changed_files(files: &[String], has_test_changes: bool) -> Self {
        if has_test_changes {
            return Self::Test;
        }
        let has_docs = files.iter().any(|f| {
            let ext = std::path::Path::new(f)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            matches!(ext, "md" | "txt" | "rst")
        });
        if has_docs {
            return Self::Docs;
        }
        let has_ci = files
            .iter()
            .any(|f| f.contains(".github/") || f.contains(".ci/") || f.contains("Dockerfile"));
        if has_ci {
            return Self::Ci;
        }
        Self::Feat
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Feat => "feat",
            Self::Fix => "fix",
            Self::Docs => "docs",
            Self::Style => "style",
            Self::Refactor => "refactor",
            Self::Perf => "perf",
            Self::Test => "test",
            Self::Build => "build",
            Self::Ci => "ci",
            Self::Chore => "chore",
            Self::Revert => "revert",
        }
    }
}

/// A generated commit message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitMessage {
    /// The full commit message (subject + body)
    pub message: String,
    /// The subject line (first line)
    pub subject: String,
    /// Optional body
    pub body: Option<String>,
    /// Optional footer (e.g. " BREAKING CHANGE: ...")
    pub footer: Option<String>,
    /// Conventional commit type (if applicable)
    pub commit_type: Option<ConventionalCommitType>,
    /// Scope (e.g. "browser", "sprint")
    pub scope: Option<String>,
    /// Whether this is a breaking change
    pub breaking: bool,
}

impl CommitMessage {
    /// Generate a conventional commit message.
    pub fn conventional(
        commit_type: ConventionalCommitType,
        scope: Option<&str>,
        description: &str,
        body: Option<&str>,
        breaking: bool,
    ) -> Self {
        let subject = match scope {
            Some(s) => format!("{}({}): {}", commit_type.as_str(), s, description),
            None => format!("{}: {}", commit_type.as_str(), description),
        };

        let mut message = subject.clone();
        if let Some(b) = body {
            message.push_str("\n\n");
            message.push_str(b);
        }
        if breaking {
            message.push_str("\n\nBREAKING CHANGE: ");
            message.push_str(description);
        }

        Self {
            message,
            subject,
            body: body.map(String::from),
            footer: if breaking {
                Some(format!("BREAKING CHANGE: {description}"))
            } else {
                None
            },
            commit_type: Some(commit_type),
            scope: scope.map(String::from),
            breaking,
        }
    }
}

impl std::fmt::Display for CommitMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

// ─── Pre-Ship Check Results ────────────────────────────────────────────────

/// Result of a pre-ship check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipCheckResult {
    /// Name of the check
    pub check_name: String,
    /// Whether the check passed
    pub passed: bool,
    /// Human-readable message
    pub message: String,
    /// Severity if failed (warn, error, block)
    pub severity: ShipCheckSeverity,
}

/// Severity of a failed ship check.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShipCheckSeverity {
    /// Informational warning — ship can proceed
    Warn,
    /// Error — should fix but can override
    Error,
    /// Blocking — cannot ship until resolved
    Block,
}

/// Aggregate result of all pre-ship checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipCheckReport {
    pub checks: Vec<ShipCheckResult>,
    pub all_passed: bool,
    pub blocked: bool,
}

impl ShipCheckReport {
    pub fn new(checks: Vec<ShipCheckResult>) -> Self {
        let blocked = checks
            .iter()
            .any(|c| c.severity == ShipCheckSeverity::Block && !c.passed);
        let all_passed = checks.iter().all(|c| c.passed);
        Self {
            checks,
            all_passed,
            blocked,
        }
    }

    pub fn summary(&self) -> String {
        let passed = self.checks.iter().filter(|c| c.passed).count();
        let total = self.checks.len();
        let mut lines = vec![format!(
            "Ship checks: {passed}/{total} passed{}",
            if self.blocked { " [BLOCKED]" } else { "" }
        )];
        for check in &self.checks {
            let icon = if check.passed { "✓" } else { "✗" };
            lines.push(format!("  {icon} {}: {}", check.check_name, check.message));
        }
        lines.join("\n")
    }
}

// ─── Ship Configuration ────────────────────────────────────────────────────

/// Configuration for the ship pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipConfig {
    /// Project root directory
    pub project_root: PathBuf,
    /// Branch rules
    pub branch_rules: Vec<BranchRule>,
    /// Commit message strategy
    pub commit_strategy: CommitMessageStrategy,
    /// Whether to auto-push after commit
    pub auto_push: bool,
    /// Whether to auto-create a PR when required
    pub auto_pr: bool,
    /// Whether to run benchmarks before/after shipping
    pub run_benchmarks: bool,
    /// Custom commit message template (for CustomTemplate strategy)
    pub custom_template: Option<String>,
}

impl Default for ShipConfig {
    fn default() -> Self {
        Self {
            project_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            branch_rules: vec![
                BranchRule::main(),
                BranchRule::develop(),
                BranchRule::feature(),
            ],
            commit_strategy: CommitMessageStrategy::ConventionalCommits,
            auto_push: false,
            auto_pr: true,
            run_benchmarks: false,
            custom_template: None,
        }
    }
}

// ─── Ship Pipeline ─────────────────────────────────────────────────────────

/// The ship pipeline orchestrates the full shipping workflow.
pub struct ShipPipeline {
    config: ShipConfig,
    /// Ship history for metrics
    history: RwLock<Vec<ShipRecord>>,
}

/// A record of a completed (or attempted) ship operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipRecord {
    /// Timestamp of the ship attempt
    pub timestamp_ms: u64,
    /// Branch that was shipped to
    pub branch: String,
    /// Whether the ship succeeded
    pub success: bool,
    /// Commit hash (if committed)
    pub commit_hash: Option<String>,
    /// Commit message subject
    pub commit_subject: Option<String>,
    /// Pre-ship check report
    pub check_report: ShipCheckReport,
    /// Files changed
    pub files_changed: Vec<String>,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

impl ShipPipeline {
    /// Create a new ship pipeline with the given configuration.
    pub fn new(config: ShipConfig) -> Self {
        Self {
            config,
            history: RwLock::new(Vec::new()),
        }
    }

    /// Create with default configuration.
    pub fn new_default() -> Self {
        Self::new(ShipConfig::default())
    }

    /// Get the branch rules applicable to a given branch.
    pub fn get_branch_rule(&self, branch: &str) -> BranchRule {
        self.config
            .branch_rules
            .iter()
            .find(|r| r.matches(branch))
            .cloned()
            .unwrap_or_else(|| BranchRule {
                pattern: "*".to_string(),
                protection: BranchProtection::None,
                require_pr: false,
            })
    }

    /// Run pre-ship checks.
    ///
    /// Returns a report with all check results.
    pub async fn run_pre_ship_checks(
        &self,
        branch: &str,
        changed_files: &[String],
        tests_passed: bool,
        has_review_approval: bool,
    ) -> ShipCheckReport {
        let rule = self.get_branch_rule(branch);
        let mut checks = Vec::new();

        // Check 1: Branch protection level
        match rule.protection {
            BranchProtection::None => {
                checks.push(ShipCheckResult {
                    check_name: "branch_protection".to_string(),
                    passed: true,
                    message: "No branch protection required".to_string(),
                    severity: ShipCheckSeverity::Warn,
                });
            },
            BranchProtection::RequireTestsPass => {
                checks.push(ShipCheckResult {
                    check_name: "tests_pass".to_string(),
                    passed: tests_passed,
                    message: if tests_passed {
                        "All tests pass".to_string()
                    } else {
                        "Some tests failed".to_string()
                    },
                    severity: ShipCheckSeverity::Error,
                });
            },
            BranchProtection::RequireReviewApproval => {
                checks.push(ShipCheckResult {
                    check_name: "tests_pass".to_string(),
                    passed: tests_passed,
                    message: if tests_passed {
                        "All tests pass".to_string()
                    } else {
                        "Some tests failed".to_string()
                    },
                    severity: ShipCheckSeverity::Error,
                });
                checks.push(ShipCheckResult {
                    check_name: "review_approved".to_string(),
                    passed: has_review_approval,
                    message: if has_review_approval {
                        "Review approved".to_string()
                    } else {
                        "Review approval required".to_string()
                    },
                    severity: ShipCheckSeverity::Error,
                });
            },
            BranchProtection::Full => {
                checks.push(ShipCheckResult {
                    check_name: "tests_pass".to_string(),
                    passed: tests_passed,
                    message: if tests_passed {
                        "All tests pass".to_string()
                    } else {
                        "Some tests failed".to_string()
                    },
                    severity: ShipCheckSeverity::Block,
                });
                checks.push(ShipCheckResult {
                    check_name: "review_approved".to_string(),
                    passed: has_review_approval,
                    message: if has_review_approval {
                        "Review approved".to_string()
                    } else {
                        "Review approval required for main branch".to_string()
                    },
                    severity: ShipCheckSeverity::Block,
                });
                if rule.require_pr && branch == "main" {
                    checks.push(ShipCheckResult {
                        check_name: "direct_push".to_string(),
                        passed: false,
                        message: "Direct push to main is not allowed — use a PR".to_string(),
                        severity: ShipCheckSeverity::Block,
                    });
                }
            },
        }

        // Check 2: Empty commit
        checks.push(ShipCheckResult {
            check_name: "has_changes".to_string(),
            passed: !changed_files.is_empty(),
            message: if changed_files.is_empty() {
                "No changes to ship".to_string()
            } else {
                format!("{} file(s) changed", changed_files.len())
            },
            severity: ShipCheckSeverity::Block,
        });

        // Check 3: Large file warning
        let has_large_files = changed_files
            .iter()
            .any(|f| f.ends_with(".lock") || f.ends_with(".min.js") || f.ends_with(".min.css"));
        if has_large_files {
            checks.push(ShipCheckResult {
                check_name: "large_files".to_string(),
                passed: true,
                message: "Contains generated/minified files — consider .gitignore".to_string(),
                severity: ShipCheckSeverity::Warn,
            });
        }

        ShipCheckReport::new(checks)
    }

    /// Generate a commit message based on the configured strategy.
    pub fn generate_commit_message(
        &self,
        changed_files: &[String],
        description: &str,
        scope: Option<&str>,
    ) -> CommitMessage {
        match &self.config.commit_strategy {
            CommitMessageStrategy::ConventionalCommits => {
                let has_test_changes = changed_files.iter().any(|f| {
                    let path = std::path::Path::new(f);
                    path.components().any(|c| c.as_os_str() == "tests")
                });
                let commit_type =
                    ConventionalCommitType::from_changed_files(changed_files, has_test_changes);
                CommitMessage::conventional(commit_type, scope, description, None, false)
            },
            CommitMessageStrategy::LlmGenerated => {
                // LLM generation would be done externally — return a placeholder
                CommitMessage::conventional(
                    ConventionalCommitType::Feat,
                    scope,
                    description,
                    Some("[LLM-generated commit message — configure LLM for auto-generation]"),
                    false,
                )
            },
            CommitMessageStrategy::CustomTemplate(template) => {
                let message = template
                    .replace("{description}", description)
                    .replace("{scope}", scope.unwrap_or("none"))
                    .replace("{files}", &changed_files.join(", "));
                CommitMessage {
                    message: message.clone(),
                    subject: message.lines().next().unwrap_or("").to_string(),
                    body: Some(message),
                    footer: None,
                    commit_type: None,
                    scope: scope.map(String::from),
                    breaking: false,
                }
            },
        }
    }

    /// Get the ship history.
    pub async fn history(&self) -> Vec<ShipRecord> {
        self.history.read().await.clone()
    }

    /// Get ship statistics.
    pub async fn stats(&self) -> ShipStats {
        let history = self.history.read().await;
        let total = history.len();
        let succeeded = history.iter().filter(|h| h.success).count();
        let failed = total - succeeded;
        let avg_duration_ms = if total > 0 {
            history.iter().map(|h| h.duration_ms).sum::<u64>() / total as u64
        } else {
            0
        };

        ShipStats {
            total_ships: total,
            succeeded,
            failed,
            success_rate: if total > 0 {
                succeeded as f64 / total as f64
            } else {
                0.0
            },
            avg_duration_ms,
        }
    }
}

/// Ship pipeline statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipStats {
    pub total_ships: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub success_rate: f64,
    pub avg_duration_ms: u64,
}

impl std::fmt::Display for ShipStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Ships: {} total, {} succeeded, {} failed ({:.1}% success, avg {:.0}ms)",
            self.total_ships,
            self.succeeded,
            self.failed,
            self.success_rate * 100.0,
            self.avg_duration_ms
        )
    }
}

// ─── Canary Deployment ─────────────────────────────────────────────────────

/// Canary deployment configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryConfig {
    /// Percentage of traffic to route to canary (0-100)
    pub traffic_percentage: u8,
    /// Duration to monitor canary before full rollout (seconds)
    pub observation_period_secs: u64,
    /// Error rate threshold that triggers rollback (0.0-1.0)
    pub error_threshold: f64,
    /// Latency threshold in milliseconds for rollback
    pub latency_threshold_ms: u64,
    /// Whether to auto-rollback on failure
    pub auto_rollback: bool,
}

impl Default for CanaryConfig {
    fn default() -> Self {
        Self {
            traffic_percentage: 5,
            observation_period_secs: 300,
            error_threshold: 0.01,
            latency_threshold_ms: 500,
            auto_rollback: true,
        }
    }
}

/// Status of a canary deployment.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CanaryStatus {
    /// Canary is being prepared
    Preparing,
    /// Canary is deployed and being observed
    Observing,
    /// Canary passed all checks — ready for full rollout
    Passed,
    /// Canary failed — rollback triggered
    Failed,
    /// Full rollout completed
    RolledOut,
    /// Canary was rolled back
    RolledBack,
}

/// Metrics observed during canary deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryMetrics {
    /// Total requests during observation
    pub total_requests: u64,
    /// Error count
    pub error_count: u64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// P99 latency in milliseconds
    pub p99_latency_ms: f64,
    /// Whether error threshold was exceeded
    pub error_threshold_exceeded: bool,
    /// Whether latency threshold was exceeded
    pub latency_threshold_exceeded: bool,
}

impl CanaryMetrics {
    /// Determine if the canary should pass based on observed metrics.
    pub fn should_pass(&self, config: &CanaryConfig) -> bool {
        !self.error_threshold_exceeded && !self.latency_threshold_exceeded
    }

    /// Create a simulated passing metrics set.
    pub fn passing() -> Self {
        Self {
            total_requests: 1000,
            error_count: 5,
            avg_latency_ms: 120.0,
            p99_latency_ms: 300.0,
            error_threshold_exceeded: false,
            latency_threshold_exceeded: false,
        }
    }

    /// Create a simulated failing metrics set.
    pub fn failing() -> Self {
        Self {
            total_requests: 1000,
            error_count: 150,
            avg_latency_ms: 800.0,
            p99_latency_ms: 2000.0,
            error_threshold_exceeded: true,
            latency_threshold_exceeded: true,
        }
    }
}

/// A canary deployment record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryDeployment {
    /// Commit hash being deployed
    pub commit_hash: String,
    /// Current status
    pub status: CanaryStatus,
    /// Configuration used
    pub config: CanaryConfig,
    /// Metrics observed (if in/after observation phase)
    pub metrics: Option<CanaryMetrics>,
    /// Start timestamp
    pub started_at_ms: u64,
    /// End timestamp
    pub ended_at_ms: Option<u64>,
}

impl CanaryDeployment {
    pub fn new(commit_hash: &str, config: CanaryConfig) -> Self {
        Self {
            commit_hash: commit_hash.to_string(),
            status: CanaryStatus::Preparing,
            config,
            metrics: None,
            started_at_ms: current_timestamp_ms(),
            ended_at_ms: None,
        }
    }

    /// Transition to the next status.
    pub fn transition(&mut self, new_status: CanaryStatus) {
        self.status = new_status;
        if matches!(
            new_status,
            CanaryStatus::Passed
                | CanaryStatus::Failed
                | CanaryStatus::RolledOut
                | CanaryStatus::RolledBack
        ) {
            self.ended_at_ms = Some(current_timestamp_ms());
        }
    }
}

// ─── Benchmark Framework ───────────────────────────────────────────────────

/// A single benchmark result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Benchmark name
    pub name: String,
    /// Execution time in nanoseconds
    pub duration_ns: u64,
    /// Memory allocated in bytes (if measured)
    pub memory_bytes: Option<u64>,
    /// Whether the benchmark succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// A benchmark comparison between two runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkComparison {
    /// Benchmark name
    pub name: String,
    /// Baseline duration (nanoseconds)
    pub baseline_ns: u64,
    /// Current duration (nanoseconds)
    pub current_ns: u64,
    /// Change ratio (current / baseline)
    pub change_ratio: f64,
    /// Percentage change
    pub change_percent: f64,
    /// Whether this is a regression (significantly slower)
    pub is_regression: bool,
    /// Regression threshold (default: 10%)
    pub regression_threshold: f64,
}

impl BenchmarkComparison {
    pub fn new(
        baseline: &BenchmarkResult,
        current: &BenchmarkResult,
        regression_threshold: f64,
    ) -> Self {
        let change_ratio = if baseline.duration_ns > 0 {
            current.duration_ns as f64 / baseline.duration_ns as f64
        } else {
            1.0
        };
        let change_percent = (change_ratio - 1.0) * 100.0;
        let is_regression = change_percent > regression_threshold;

        Self {
            name: baseline.name.clone(),
            baseline_ns: baseline.duration_ns,
            current_ns: current.duration_ns,
            change_ratio,
            change_percent,
            is_regression,
            regression_threshold,
        }
    }

    /// Format as a human-readable comparison line.
    pub fn format_line(&self) -> String {
        let icon = if self.is_regression { "⚠" } else { "✓" };
        let direction = if self.change_percent > 0.0 {
            format!("+{:.1}%", self.change_percent)
        } else {
            format!("{:.1}%", self.change_percent)
        };
        format!(
            "  {} {}: {:.0}ns → {:.0}ns ({})",
            icon, self.name, self.baseline_ns, self.current_ns, direction
        )
    }
}

/// A benchmark suite result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSuite {
    /// Suite name
    pub name: String,
    /// Individual benchmark results
    pub results: Vec<BenchmarkResult>,
    /// Timestamp
    pub timestamp_ms: u64,
}

impl BenchmarkSuite {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            results: Vec::new(),
            timestamp_ms: current_timestamp_ms(),
        }
    }

    /// Add a benchmark result.
    pub fn add_result(&mut self, result: BenchmarkResult) {
        self.results.push(result);
    }

    /// Compare this suite against a baseline suite.
    pub fn compare(&self, baseline: &BenchmarkSuite, regression_threshold: f64) -> BenchmarkReport {
        let mut comparisons = Vec::new();

        // Build a map of baseline results by name
        let baseline_map: HashMap<String, &BenchmarkResult> = baseline
            .results
            .iter()
            .map(|r| (r.name.clone(), r))
            .collect();

        for current in &self.results {
            if let Some(baseline_result) = baseline_map.get(&current.name) {
                comparisons.push(BenchmarkComparison::new(
                    baseline_result,
                    current,
                    regression_threshold,
                ));
            }
        }

        let regressions = comparisons.iter().filter(|c| c.is_regression).count();
        let improvements = comparisons
            .iter()
            .filter(|c| c.change_percent < -5.0)
            .count();
        let total_compared = comparisons.len();

        BenchmarkReport {
            baseline_name: baseline.name.clone(),
            current_name: self.name.clone(),
            comparisons,
            total_compared,
            regressions,
            improvements,
            regression_threshold,
        }
    }
}

/// A report comparing two benchmark suites.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub baseline_name: String,
    pub current_name: String,
    pub comparisons: Vec<BenchmarkComparison>,
    pub total_compared: usize,
    pub regressions: usize,
    pub improvements: usize,
    pub regression_threshold: f64,
}

impl BenchmarkReport {
    /// Format as a human-readable report.
    pub fn format(&self) -> String {
        let mut lines = vec![format!(
            "Benchmark: {} vs {} ({} compared, {} regressions, {} improvements)",
            self.current_name,
            self.baseline_name,
            self.total_compared,
            self.regressions,
            self.improvements
        )];
        for comp in &self.comparisons {
            lines.push(comp.format_line());
        }
        lines.join("\n")
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branch_rule_matches() {
        let main = BranchRule::main();
        assert!(main.matches("main"));
        assert!(!main.matches("main-2"));

        let feature = BranchRule::feature();
        assert!(feature.matches("feature/auth"));
        assert!(!feature.matches("fix/auth"));
    }

    #[test]
    fn test_branch_protection_equality() {
        assert_eq!(BranchProtection::None, BranchProtection::None);
        assert_ne!(BranchProtection::Full, BranchProtection::RequireTestsPass);
    }

    #[test]
    fn test_conventional_commit_type_from_files() {
        let type_test =
            ConventionalCommitType::from_changed_files(&["src/test/foo.rs".to_string()], true);
        assert_eq!(type_test, ConventionalCommitType::Test);

        let type_docs =
            ConventionalCommitType::from_changed_files(&["README.md".to_string()], false);
        assert_eq!(type_docs, ConventionalCommitType::Docs);

        let type_ci = ConventionalCommitType::from_changed_files(
            &[".github/workflows/ci.yml".to_string()],
            false,
        );
        assert_eq!(type_ci, ConventionalCommitType::Ci);

        let type_feat =
            ConventionalCommitType::from_changed_files(&["src/lib.rs".to_string()], false);
        assert_eq!(type_feat, ConventionalCommitType::Feat);
    }

    #[test]
    fn test_commit_message_conventional() {
        let msg = CommitMessage::conventional(
            ConventionalCommitType::Feat,
            Some("browser"),
            "add daemon with accessibility refs",
            Some("Implements persistent browser with @e1 ref system"),
            false,
        );
        assert_eq!(
            msg.subject,
            "feat(browser): add daemon with accessibility refs"
        );
        assert!(msg.message.contains("Implements persistent browser"));
        assert!(!msg.breaking);
    }

    #[test]
    fn test_commit_message_breaking() {
        let msg = CommitMessage::conventional(
            ConventionalCommitType::Fix,
            None,
            "change API signature",
            None,
            true,
        );
        assert!(msg.breaking);
        assert!(msg.message.contains("BREAKING CHANGE"));
    }

    #[test]
    fn test_commit_message_display() {
        let msg = CommitMessage::conventional(
            ConventionalCommitType::Docs,
            None,
            "update README",
            None,
            false,
        );
        let display = format!("{msg}");
        assert!(display.starts_with("docs:"));
    }

    #[test]
    fn test_ship_check_report() {
        let checks = vec![
            ShipCheckResult {
                check_name: "tests".to_string(),
                passed: true,
                message: "All pass".to_string(),
                severity: ShipCheckSeverity::Block,
            },
            ShipCheckResult {
                check_name: "changes".to_string(),
                passed: true,
                message: "3 files".to_string(),
                severity: ShipCheckSeverity::Block,
            },
        ];
        let report = ShipCheckReport::new(checks);
        assert!(report.all_passed);
        assert!(!report.blocked);
    }

    #[test]
    fn test_ship_check_report_blocked() {
        let checks = vec![ShipCheckResult {
            check_name: "tests".to_string(),
            passed: false,
            message: "Failed".to_string(),
            severity: ShipCheckSeverity::Block,
        }];
        let report = ShipCheckReport::new(checks);
        assert!(!report.all_passed);
        assert!(report.blocked);
    }

    #[test]
    fn test_ship_check_report_summary() {
        let checks = vec![
            ShipCheckResult {
                check_name: "tests".to_string(),
                passed: true,
                message: "Pass".to_string(),
                severity: ShipCheckSeverity::Block,
            },
            ShipCheckResult {
                check_name: "review".to_string(),
                passed: false,
                message: "Missing".to_string(),
                severity: ShipCheckSeverity::Error,
            },
        ];
        let report = ShipCheckReport::new(checks);
        let summary = report.summary();
        assert!(summary.contains("1/2 passed"));
    }

    #[tokio::test]
    async fn test_ship_pipeline_branch_rule() {
        let pipeline = ShipPipeline::new_default();
        let rule = pipeline.get_branch_rule("main");
        assert_eq!(rule.protection, BranchProtection::Full);

        let rule = pipeline.get_branch_rule("feature/new-auth");
        assert_eq!(rule.protection, BranchProtection::None);
    }

    #[tokio::test]
    async fn test_ship_pipeline_pre_ship_checks() {
        let pipeline = ShipPipeline::new_default();
        let report = pipeline
            .run_pre_ship_checks(
                "feature/new-auth",
                &["src/auth.rs".to_string()],
                true,
                false,
            )
            .await;
        assert!(report.all_passed);
        assert!(!report.blocked);
    }

    #[tokio::test]
    async fn test_ship_pipeline_pre_ship_checks_main_blocked() {
        let pipeline = ShipPipeline::new_default();
        let report = pipeline
            .run_pre_ship_checks("main", &["src/lib.rs".to_string()], true, false)
            .await;
        assert!(!report.all_passed);
        assert!(report.blocked);
    }

    #[tokio::test]
    async fn test_ship_pipeline_generate_commit_message() {
        let pipeline = ShipPipeline::new_default();
        let msg = pipeline.generate_commit_message(
            &["src/lib.rs".to_string()],
            "add new feature",
            Some("core"),
        );
        assert!(msg.subject.contains("feat(core):"));
    }

    #[tokio::test]
    async fn test_ship_pipeline_custom_template() {
        let config = ShipConfig {
            commit_strategy: CommitMessageStrategy::CustomTemplate(
                "SHIP: {description} (scope={scope})".to_string(),
            ),
            ..ShipConfig::default()
        };
        let pipeline = ShipPipeline::new(config);
        let msg = pipeline.generate_commit_message(
            &["src/lib.rs".to_string()],
            "add feature",
            Some("core"),
        );
        assert_eq!(msg.subject, "SHIP: add feature (scope=core)");
    }

    #[tokio::test]
    async fn test_ship_pipeline_stats() {
        let pipeline = ShipPipeline::new_default();
        let stats = pipeline.stats().await;
        assert_eq!(stats.total_ships, 0);
        assert_eq!(stats.success_rate, 0.0);
    }

    #[test]
    fn test_canary_config_default() {
        let config = CanaryConfig::default();
        assert_eq!(config.traffic_percentage, 5);
        assert!(config.auto_rollback);
    }

    #[test]
    fn test_canary_metrics_passing() {
        let metrics = CanaryMetrics::passing();
        let config = CanaryConfig::default();
        assert!(metrics.should_pass(&config));
    }

    #[test]
    fn test_canary_metrics_failing() {
        let metrics = CanaryMetrics::failing();
        let config = CanaryConfig::default();
        assert!(!metrics.should_pass(&config));
    }

    #[test]
    fn test_canary_deployment_lifecycle() {
        let mut deployment = CanaryDeployment::new("abc123", CanaryConfig::default());
        assert_eq!(deployment.status, CanaryStatus::Preparing);

        deployment.transition(CanaryStatus::Observing);
        assert_eq!(deployment.status, CanaryStatus::Observing);

        deployment.transition(CanaryStatus::Passed);
        assert_eq!(deployment.status, CanaryStatus::Passed);
        assert!(deployment.ended_at_ms.is_some());
    }

    #[test]
    fn test_benchmark_comparison() {
        let baseline = BenchmarkResult {
            name: "sprint_execute".to_string(),
            duration_ns: 1_000_000,
            memory_bytes: Some(1024),
            success: true,
            error: None,
            metadata: HashMap::new(),
        };
        let current = BenchmarkResult {
            name: "sprint_execute".to_string(),
            duration_ns: 1_050_000,
            memory_bytes: Some(1100),
            success: true,
            error: None,
            metadata: HashMap::new(),
        };
        let comp = BenchmarkComparison::new(&baseline, &current, 10.0);
        assert!((comp.change_percent - 5.0).abs() < 0.1);
        assert!(!comp.is_regression);
    }

    #[test]
    fn test_benchmark_comparison_regression() {
        let baseline = BenchmarkResult {
            name: "sprint_execute".to_string(),
            duration_ns: 1_000_000,
            memory_bytes: None,
            success: true,
            error: None,
            metadata: HashMap::new(),
        };
        let current = BenchmarkResult {
            name: "sprint_execute".to_string(),
            duration_ns: 1_200_000,
            memory_bytes: None,
            success: true,
            error: None,
            metadata: HashMap::new(),
        };
        let comp = BenchmarkComparison::new(&baseline, &current, 10.0);
        assert!(comp.is_regression);
        assert!(comp.format_line().contains("⚠"));
    }

    #[test]
    fn test_benchmark_suite_comparison() {
        let mut baseline = BenchmarkSuite::new("v1.0");
        baseline.add_result(BenchmarkResult {
            name: "test_a".to_string(),
            duration_ns: 100,
            memory_bytes: None,
            success: true,
            error: None,
            metadata: HashMap::new(),
        });

        let mut current = BenchmarkSuite::new("v1.1");
        current.add_result(BenchmarkResult {
            name: "test_a".to_string(),
            duration_ns: 80,
            memory_bytes: None,
            success: true,
            error: None,
            metadata: HashMap::new(),
        });

        let report = current.compare(&baseline, 10.0);
        assert_eq!(report.total_compared, 1);
        assert_eq!(report.improvements, 1);
        assert_eq!(report.regressions, 0);
    }

    #[test]
    fn test_benchmark_report_format() {
        let mut baseline = BenchmarkSuite::new("v1.0");
        baseline.add_result(BenchmarkResult {
            name: "test_fast".to_string(),
            duration_ns: 50,
            memory_bytes: None,
            success: true,
            error: None,
            metadata: HashMap::new(),
        });
        let mut current = BenchmarkSuite::new("v1.1");
        current.add_result(BenchmarkResult {
            name: "test_fast".to_string(),
            duration_ns: 75,
            memory_bytes: None,
            success: true,
            error: None,
            metadata: HashMap::new(),
        });

        let report = current.compare(&baseline, 10.0);
        let formatted = report.format();
        assert!(formatted.contains("v1.1 vs v1.0"));
        assert!(formatted.contains("+50.0%"));
    }

    #[test]
    fn test_ship_stats_display() {
        let stats = ShipStats {
            total_ships: 10,
            succeeded: 8,
            failed: 2,
            success_rate: 0.8,
            avg_duration_ms: 5000,
        };
        let display = format!("{stats}");
        assert!(display.contains("10 total"));
        assert!(display.contains("80.0% success"));
    }

    #[test]
    fn test_ship_config_serialization() {
        let config = ShipConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ShipConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.auto_push, false);
    }

    #[test]
    fn test_canary_deployment_serialization() {
        let deployment = CanaryDeployment::new("abc123", CanaryConfig::default());
        let json = serde_json::to_string(&deployment).unwrap();
        let parsed: CanaryDeployment = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.status, CanaryStatus::Preparing);
    }
}
