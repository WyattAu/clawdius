//! Architecture drift detection module
//!
//! This module provides functionality to detect architectural drift
//! in codebases, helping maintain code quality and consistency.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Severity level of architectural drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DriftSeverity {
    /// Low severity - minor deviation
    Low,
    /// Medium severity - moderate deviation
    Medium,
    /// High severity - significant deviation
    High,
    /// Critical severity - requires immediate attention
    Critical,
}

impl Default for DriftSeverity {
    fn default() -> Self {
        Self::Low
    }
}

/// Category of architectural drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DriftCategory {
    /// Structural drift - changes to module/file organization
    Structural,
    /// Pattern drift - deviation from established patterns
    Pattern,
    /// Dependency drift - unexpected dependencies
    Dependency,
    /// Style drift - code style inconsistencies
    Style,
    /// API drift - interface changes
    Api,
    /// Performance drift - performance-related issues
    Performance,
}

impl Default for DriftCategory {
    fn default() -> Self {
        Self::Style
    }
}

/// A single drift rule for detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftRule {
    /// Unique identifier for the rule
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of what the rule detects
    pub description: String,
    /// Category of drift
    pub category: DriftCategory,
    /// Default severity
    pub default_severity: DriftSeverity,
    /// Whether the rule is enabled
    pub enabled: bool,
}

impl DriftRule {
    /// Creates a new drift rule.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        category: DriftCategory,
        severity: DriftSeverity,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            category,
            default_severity: severity,
            enabled: true,
        }
    }

    /// Disables the rule.
    #[must_use]
    pub const fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// A detected instance of architectural drift.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureDrift {
    /// Unique identifier for this drift instance
    pub id: String,
    /// The rule that detected this drift
    pub rule_id: String,
    /// File where the drift was detected
    pub file_path: PathBuf,
    /// Line number (if applicable)
    pub line_number: Option<usize>,
    /// Actual severity (may override default)
    pub severity: DriftSeverity,
    /// Category of drift
    pub category: DriftCategory,
    /// Human-readable message
    pub message: String,
    /// Suggested fix (if available)
    pub suggestion: Option<String>,
    /// Additional context/metadata
    pub context: HashMap<String, String>,
}

impl ArchitectureDrift {
    /// Creates a new architecture drift instance.
    #[must_use]
    pub fn new(rule_id: impl Into<String>, file_path: PathBuf, message: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            rule_id: rule_id.into(),
            file_path,
            line_number: None,
            severity: DriftSeverity::Low,
            category: DriftCategory::Style,
            message: message.into(),
            suggestion: None,
            context: HashMap::new(),
        }
    }

    /// Sets the line number.
    #[must_use]
    pub const fn at_line(mut self, line: usize) -> Self {
        self.line_number = Some(line);
        self
    }

    /// Sets the severity.
    #[must_use]
    pub const fn with_severity(mut self, severity: DriftSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Sets the category.
    #[must_use]
    pub const fn with_category(mut self, category: DriftCategory) -> Self {
        self.category = category;
        self
    }

    /// Sets the suggestion.
    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Adds context metadata.
    #[must_use]
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }
}

/// Report containing all detected drifts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DriftReport {
    /// All detected drifts
    pub drifts: Vec<ArchitectureDrift>,
    /// Summary by category
    pub summary_by_category: HashMap<DriftCategory, usize>,
    /// Summary by severity
    pub summary_by_severity: HashMap<DriftSeverity, usize>,
    /// Total files analyzed
    pub files_analyzed: usize,
    /// Analysis timestamp
    pub timestamp: String,
}

impl DriftReport {
    /// Creates a new empty drift report.
    #[must_use]
    pub fn new() -> Self {
        Self {
            drifts: Vec::new(),
            summary_by_category: HashMap::new(),
            summary_by_severity: HashMap::new(),
            files_analyzed: 0,
            timestamp: chrono::Local::now().to_rfc3339(),
        }
    }

    /// Adds a drift to the report.
    pub fn add(&mut self, drift: ArchitectureDrift) {
        *self.summary_by_category.entry(drift.category).or_insert(0) += 1;
        *self.summary_by_severity.entry(drift.severity).or_insert(0) += 1;
        self.drifts.push(drift);
    }

    /// Returns the number of drifts.
    #[must_use]
    pub fn len(&self) -> usize {
        self.drifts.len()
    }

    /// Returns true if there are no drifts.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.drifts.is_empty()
    }

    /// Returns drifts filtered by severity.
    #[must_use]
    pub fn by_severity(&self, severity: DriftSeverity) -> Vec<&ArchitectureDrift> {
        self.drifts
            .iter()
            .filter(|d| d.severity == severity)
            .collect()
    }

    /// Returns drifts filtered by category.
    #[must_use]
    pub fn by_category(&self, category: DriftCategory) -> Vec<&ArchitectureDrift> {
        self.drifts
            .iter()
            .filter(|d| d.category == category)
            .collect()
    }

    /// Returns the total severity score (weighted sum).
    #[must_use]
    pub fn total_severity_score(&self) -> u64 {
        self.drifts
            .iter()
            .map(|d| match d.severity {
                DriftSeverity::Low => 1,
                DriftSeverity::Medium => 3,
                DriftSeverity::High => 5,
                DriftSeverity::Critical => 10,
            })
            .sum()
    }

    /// Returns true if there are any critical drifts.
    #[must_use]
    pub fn has_critical(&self) -> bool {
        self.drifts
            .iter()
            .any(|d| d.severity == DriftSeverity::Critical)
    }

    /// Returns true if there are any high or critical drifts.
    #[must_use]
    pub fn has_high_or_critical(&self) -> bool {
        self.drifts
            .iter()
            .any(|d| matches!(d.severity, DriftSeverity::High | DriftSeverity::Critical))
    }
}

/// Drift detector with configurable rules.
#[derive(Debug, Clone)]
pub struct DriftDetector {
    /// Active drift detection rules
    rules: Vec<DriftRule>,
    /// File patterns to include
    include_patterns: Vec<String>,
    /// File patterns to exclude
    exclude_patterns: Vec<String>,
}

impl Default for DriftDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl DriftDetector {
    /// Creates a new drift detector with default rules.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rules: Self::default_rules(),
            include_patterns: vec!["**/*.rs".to_string()],
            exclude_patterns: vec!["**/target/**".to_string()],
        }
    }

    /// Returns the default set of drift detection rules.
    #[must_use]
    pub fn default_rules() -> Vec<DriftRule> {
        vec![
            DriftRule::new(
                "todo-fixer",
                "TODO/FIXME Detection",
                "Detects unresolved TODO and FIXME comments",
                DriftCategory::Style,
                DriftSeverity::Low,
            ),
            DriftRule::new(
                "unwrap-usage",
                "unwrap() Usage",
                "Detects usage of .unwrap() which may panic",
                DriftCategory::Pattern,
                DriftSeverity::Medium,
            ),
            DriftRule::new(
                "expect-usage",
                "expect() Usage",
                "Detects usage of .expect() without context",
                DriftCategory::Pattern,
                DriftSeverity::Low,
            ),
            DriftRule::new(
                "unsafe-block",
                "Unsafe Block",
                "Detects unsafe blocks that require careful review",
                DriftCategory::Pattern,
                DriftSeverity::High,
            ),
            DriftRule::new(
                "clone-on-large-type",
                "Clone on Large Type",
                "Detects .clone() calls that may be expensive",
                DriftCategory::Performance,
                DriftSeverity::Medium,
            ),
            DriftRule::new(
                "deprecated-api",
                "Deprecated API Usage",
                "Detects usage of deprecated functions or APIs",
                DriftCategory::Api,
                DriftSeverity::Medium,
            ),
            DriftRule::new(
                "circular-dependency",
                "Circular Dependency Risk",
                "Detects potential circular dependencies between modules",
                DriftCategory::Dependency,
                DriftSeverity::High,
            ),
            DriftRule::new(
                "magic-number",
                "Magic Number",
                "Detects hardcoded magic numbers without explanation",
                DriftCategory::Style,
                DriftSeverity::Low,
            ),
            DriftRule::new(
                "long-function",
                "Long Function",
                "Detects functions exceeding recommended length",
                DriftCategory::Structural,
                DriftSeverity::Medium,
            ),
            DriftRule::new(
                "deep-nesting",
                "Deep Nesting",
                "Detects deeply nested code blocks",
                DriftCategory::Structural,
                DriftSeverity::Medium,
            ),
        ]
    }

    /// Adds a custom rule.
    #[must_use]
    pub fn with_rule(mut self, rule: DriftRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Sets include patterns.
    #[must_use]
    pub fn with_include_patterns(mut self, patterns: Vec<String>) -> Self {
        self.include_patterns = patterns;
        self
    }

    /// Sets exclude patterns.
    #[must_use]
    pub fn with_exclude_patterns(mut self, patterns: Vec<String>) -> Self {
        self.exclude_patterns = patterns;
        self
    }

    /// Returns the active rules.
    #[must_use]
    pub fn rules(&self) -> &[DriftRule] {
        &self.rules
    }

    /// Analyzes a single file for architectural drift.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read.
    pub fn analyze_file(&self, file_path: &PathBuf, content: &str) -> DriftReport {
        let mut report = DriftReport::new();
        report.files_analyzed = 1;

        for rule in &self.rules {
            if !rule.enabled {
                continue;
            }

            let drifts = self.apply_rule(rule, file_path, content);
            for drift in drifts {
                report.add(drift);
            }
        }

        report
    }

    /// Analyzes multiple files for architectural drift.
    pub fn analyze_files<'a>(
        &self,
        files: impl Iterator<Item = (PathBuf, &'a str)>,
    ) -> DriftReport {
        let mut report = DriftReport::new();

        for (path, content) in files {
            report.files_analyzed += 1;

            for rule in &self.rules {
                if !rule.enabled {
                    continue;
                }

                let drifts = self.apply_rule(rule, &path, content);
                for drift in drifts {
                    report.add(drift);
                }
            }
        }

        report.timestamp = chrono::Local::now().to_rfc3339();
        report
    }

    /// Applies a single rule to a file.
    fn apply_rule(
        &self,
        rule: &DriftRule,
        file_path: &PathBuf,
        content: &str,
    ) -> Vec<ArchitectureDrift> {
        let mut drifts = Vec::new();

        match rule.id.as_str() {
            "todo-fixer" => {
                drifts.extend(self.detect_todo_fixme(rule, file_path, content));
            },
            "unwrap-usage" => {
                drifts.extend(self.detect_unwrap(rule, file_path, content));
            },
            "expect-usage" => {
                drifts.extend(self.detect_expect(rule, file_path, content));
            },
            "unsafe-block" => {
                drifts.extend(self.detect_unsafe(rule, file_path, content));
            },
            "clone-on-large-type" => {
                drifts.extend(self.detect_expensive_clone(rule, file_path, content));
            },
            "magic-number" => {
                drifts.extend(self.detect_magic_numbers(rule, file_path, content));
            },
            "long-function" => {
                drifts.extend(self.detect_long_functions(rule, file_path, content));
            },
            "deep-nesting" => {
                drifts.extend(self.detect_deep_nesting(rule, file_path, content));
            },
            _ => {
                // Unknown rule - skip
            },
        }

        drifts
    }

    fn detect_todo_fixme(
        &self,
        rule: &DriftRule,
        file_path: &PathBuf,
        content: &str,
    ) -> Vec<ArchitectureDrift> {
        let mut drifts = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            if line.contains("TODO") || line.contains("FIXME") {
                drifts.push(
                    ArchitectureDrift::new(
                        &rule.id,
                        file_path.clone(),
                        "Unresolved TODO/FIXME found",
                    )
                    .at_line(line_num + 1)
                    .with_category(rule.category)
                    .with_severity(rule.default_severity)
                    .with_suggestion("Consider resolving or documenting this item"),
                );
            }
        }

        drifts
    }

    fn detect_unwrap(
        &self,
        rule: &DriftRule,
        file_path: &PathBuf,
        content: &str,
    ) -> Vec<ArchitectureDrift> {
        let mut drifts = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            if line.contains(".unwrap()") && !line.trim().starts_with("//") {
                // Allow in test code
                if !file_path.to_string_lossy().contains("test") {
                    drifts.push(
                        ArchitectureDrift::new(
                            &rule.id,
                            file_path.clone(),
                            "Usage of .unwrap() may panic",
                        )
                        .at_line(line_num + 1)
                        .with_category(rule.category)
                        .with_severity(rule.default_severity)
                        .with_suggestion("Consider using ? operator or match expression"),
                    );
                }
            }
        }

        drifts
    }

    fn detect_expect(
        &self,
        rule: &DriftRule,
        file_path: &PathBuf,
        content: &str,
    ) -> Vec<ArchitectureDrift> {
        let mut drifts = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            if line.contains(".expect(\"") && line.matches('"').count() == 2 {
                // Simple expect with just a string - could be improved
                drifts.push(
                    ArchitectureDrift::new(
                        &rule.id,
                        file_path.clone(),
                        "Consider more descriptive expect message",
                    )
                    .at_line(line_num + 1)
                    .with_category(rule.category)
                    .with_severity(rule.default_severity),
                );
            }
        }

        drifts
    }

    fn detect_unsafe(
        &self,
        rule: &DriftRule,
        file_path: &PathBuf,
        content: &str,
    ) -> Vec<ArchitectureDrift> {
        let mut drifts = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            if line.contains("unsafe") && !line.trim().starts_with("//") {
                drifts.push(
                    ArchitectureDrift::new(
                        &rule.id,
                        file_path.clone(),
                        "Unsafe block requires careful review",
                    )
                    .at_line(line_num + 1)
                    .with_category(rule.category)
                    .with_severity(rule.default_severity)
                    .with_suggestion("Document safety invariants in a SAFETY comment"),
                );
            }
        }

        drifts
    }

    fn detect_expensive_clone(
        &self,
        rule: &DriftRule,
        file_path: &PathBuf,
        content: &str,
    ) -> Vec<ArchitectureDrift> {
        let mut drifts = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            // Detect clone on potentially large types
            if line.contains(".clone()") {
                let trimmed_line = line.trim();
                // Simple heuristic: clone in loops or on Vec/String
                if trimmed_line.contains("for ") || trimmed_line.contains("while ") {
                    drifts.push(
                        ArchitectureDrift::new(
                            &rule.id,
                            file_path.clone(),
                            "Clone in loop may be expensive",
                        )
                        .at_line(line_num + 1)
                        .with_category(rule.category)
                        .with_severity(rule.default_severity)
                        .with_suggestion("Consider using references or moving ownership"),
                    );
                }
            }
        }

        drifts
    }

    fn detect_magic_numbers(
        &self,
        rule: &DriftRule,
        file_path: &PathBuf,
        content: &str,
    ) -> Vec<ArchitectureDrift> {
        let mut drifts = Vec::new();
        let common_constants = [
            ("60", "seconds", "Consider using a constant for time values"),
            (
                "1000",
                "milliseconds",
                "Consider using a constant for time values",
            ),
            (
                "1024",
                "bytes/buffer",
                "Consider using a constant for buffer sizes",
            ),
            ("80", "columns", "Consider using a constant for line width"),
        ];

        for (line_num, line) in content.lines().enumerate() {
            for (num, _context, suggestion) in &common_constants {
                if line.contains(num) && !line.contains("const ") && !line.trim().starts_with("//")
                {
                    drifts.push(
                        ArchitectureDrift::new(
                            &rule.id,
                            file_path.clone(),
                            format!("Magic number detected: {}", num),
                        )
                        .at_line(line_num + 1)
                        .with_category(rule.category)
                        .with_severity(rule.default_severity)
                        .with_suggestion(*suggestion),
                    );
                }
            }
        }

        drifts
    }

    fn detect_long_functions(
        &self,
        rule: &DriftRule,
        file_path: &PathBuf,
        content: &str,
    ) -> Vec<ArchitectureDrift> {
        let mut drifts = Vec::new();
        let max_lines = 50;

        let mut in_function = false;
        let mut function_start = 0;
        let mut brace_count = 0;
        let mut function_lines = 0;

        for (line_num, line) in content.lines().enumerate() {
            if line.contains("fn ") && line.contains('{') {
                in_function = true;
                function_start = line_num + 1;
                brace_count = 1;
                function_lines = 1;
            } else if in_function {
                function_lines += 1;
                brace_count += line.matches('{').count() as i32;
                brace_count -= line.matches('}').count() as i32;

                if brace_count == 0 {
                    if function_lines > max_lines {
                        drifts.push(
                            ArchitectureDrift::new(
                                &rule.id,
                                file_path.clone(),
                                format!(
                                    "Function exceeds {} lines ({} lines)",
                                    max_lines, function_lines
                                ),
                            )
                            .at_line(function_start)
                            .with_category(rule.category)
                            .with_severity(rule.default_severity)
                            .with_suggestion("Consider breaking into smaller functions"),
                        );
                    }
                    in_function = false;
                }
            }
        }

        drifts
    }

    fn detect_deep_nesting(
        &self,
        rule: &DriftRule,
        file_path: &PathBuf,
        content: &str,
    ) -> Vec<ArchitectureDrift> {
        let mut drifts = Vec::new();
        let max_nesting = 4;

        for (line_num, line) in content.lines().enumerate() {
            let indent = line.len() - line.trim_start().len();
            let nesting = indent / 4; // Assuming 4-space indent

            if nesting > max_nesting && !line.trim().is_empty() {
                drifts.push(
                    ArchitectureDrift::new(
                        &rule.id,
                        file_path.clone(),
                        format!("Deep nesting detected (level {})", nesting),
                    )
                    .at_line(line_num + 1)
                    .with_category(rule.category)
                    .with_severity(rule.default_severity)
                    .with_suggestion("Consider extracting nested logic into separate functions"),
                );
            }
        }

        drifts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drift_severity_default() {
        let severity = DriftSeverity::default();
        assert_eq!(severity, DriftSeverity::Low);
    }

    #[test]
    fn test_drift_category_default() {
        let category = DriftCategory::default();
        assert_eq!(category, DriftCategory::Style);
    }

    #[test]
    fn test_drift_rule_creation() {
        let rule = DriftRule::new(
            "test-rule",
            "Test Rule",
            "A test rule",
            DriftCategory::Pattern,
            DriftSeverity::High,
        );
        assert_eq!(rule.id, "test-rule");
        assert!(rule.enabled);
    }

    #[test]
    fn test_drift_rule_disabled() {
        let rule = DriftRule::new(
            "test-rule",
            "Test Rule",
            "A test rule",
            DriftCategory::Pattern,
            DriftSeverity::High,
        )
        .disabled();
        assert!(!rule.enabled);
    }

    #[test]
    fn test_architecture_drift_creation() {
        let drift = ArchitectureDrift::new("test-rule", PathBuf::from("test.rs"), "Test message");
        assert_eq!(drift.rule_id, "test-rule");
        assert!(drift.line_number.is_none());
    }

    #[test]
    fn test_architecture_drift_builder() {
        let drift = ArchitectureDrift::new("test-rule", PathBuf::from("test.rs"), "Test message")
            .at_line(42)
            .with_severity(DriftSeverity::Critical)
            .with_category(DriftCategory::Api)
            .with_suggestion("Fix this");

        assert_eq!(drift.line_number, Some(42));
        assert_eq!(drift.severity, DriftSeverity::Critical);
        assert_eq!(drift.category, DriftCategory::Api);
        assert!(drift.suggestion.is_some());
    }

    #[test]
    fn test_drift_report() {
        let mut report = DriftReport::new();
        assert!(report.is_empty());
        assert_eq!(report.len(), 0);

        let drift = ArchitectureDrift::new("test-rule", PathBuf::from("test.rs"), "Test message");
        report.add(drift);

        assert!(!report.is_empty());
        assert_eq!(report.len(), 1);
    }

    #[test]
    fn test_drift_report_filtering() {
        let mut report = DriftReport::new();

        report.add(
            ArchitectureDrift::new("rule1", PathBuf::from("a.rs"), "msg1")
                .with_severity(DriftSeverity::Critical),
        );
        report.add(
            ArchitectureDrift::new("rule2", PathBuf::from("b.rs"), "msg2")
                .with_severity(DriftSeverity::Low),
        );
        report.add(
            ArchitectureDrift::new("rule3", PathBuf::from("c.rs"), "msg3")
                .with_severity(DriftSeverity::Critical),
        );

        assert!(report.has_critical());
        assert_eq!(report.by_severity(DriftSeverity::Critical).len(), 2);
        assert_eq!(report.by_severity(DriftSeverity::Low).len(), 1);
    }

    #[test]
    fn test_drift_report_severity_score() {
        let mut report = DriftReport::new();

        report.add(
            ArchitectureDrift::new("rule", PathBuf::from("a.rs"), "msg")
                .with_severity(DriftSeverity::Low),
        );
        report.add(
            ArchitectureDrift::new("rule", PathBuf::from("b.rs"), "msg")
                .with_severity(DriftSeverity::Critical),
        );

        // Low = 1, Critical = 10
        assert_eq!(report.total_severity_score(), 11);
    }

    #[test]
    fn test_drift_detector_creation() {
        let detector = DriftDetector::new();
        assert!(!detector.rules().is_empty());
    }

    #[test]
    fn test_drift_detector_analyze_file_todo() {
        let detector = DriftDetector::new();
        let content = r#"
fn main() {
    // TODO: implement this
    println!("Hello");
}
"#;
        let report = detector.analyze_file(&PathBuf::from("test.rs"), content);
        assert!(!report.is_empty());
        assert!(report.drifts.iter().any(|d| d.rule_id == "todo-fixer"));
    }

    #[test]
    fn test_drift_detector_analyze_file_unwrap() {
        let detector = DriftDetector::new();
        let content = r#"
fn main() {
    let x = Some(1).unwrap();
}
"#;
        let report = detector.analyze_file(&PathBuf::from("src/main.rs"), content);
        assert!(report.drifts.iter().any(|d| d.rule_id == "unwrap-usage"));
    }

    #[test]
    fn test_drift_detector_analyze_file_unsafe() {
        let detector = DriftDetector::new();
        let content = r#"
fn main() {
    unsafe {
        println!("unsafe code");
    }
}
"#;
        let report = detector.analyze_file(&PathBuf::from("test.rs"), content);
        assert!(report.drifts.iter().any(|d| d.rule_id == "unsafe-block"));
    }

    #[test]
    fn test_drift_detector_with_custom_rule() {
        let custom_rule = DriftRule::new(
            "custom-rule",
            "Custom Rule",
            "A custom detection rule",
            DriftCategory::Style,
            DriftSeverity::Low,
        );

        let detector = DriftDetector::new().with_rule(custom_rule);
        assert!(detector.rules().iter().any(|r| r.id == "custom-rule"));
    }
}
