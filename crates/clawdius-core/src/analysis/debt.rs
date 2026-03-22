//! Technical debt quantification module
//!
//! This module provides functionality to quantify and track technical debt
//! in codebases, helping prioritize refactoring efforts.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Type of technical debt item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DebtType {
    /// Code complexity debt
    CodeComplexity,
    /// Code duplication debt
    CodeDuplication,
    /// Documentation debt
    DocumentationDebt,
    /// Testing debt
    TestingDebt,
    /// Dependency debt
    DependencyDebt,
    /// Architecture debt
    ArchitectureDebt,
    /// Performance debt
    PerformanceDebt,
    /// Security debt
    SecurityDebt,
    /// Maintainability debt
    Maintainability,
}

impl Default for DebtType {
    fn default() -> Self {
        Self::CodeComplexity
    }
}

impl std::fmt::Display for DebtType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CodeComplexity => write!(f, "code_complexity"),
            Self::CodeDuplication => write!(f, "code_duplication"),
            Self::DocumentationDebt => write!(f, "documentation"),
            Self::TestingDebt => write!(f, "testing"),
            Self::DependencyDebt => write!(f, "dependency"),
            Self::ArchitectureDebt => write!(f, "architecture"),
            Self::PerformanceDebt => write!(f, "performance"),
            Self::SecurityDebt => write!(f, "security"),
            Self::Maintainability => write!(f, "maintainability"),
        }
    }
}

/// A single technical debt item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtItem {
    /// Unique identifier
    pub id: String,
    /// Type of debt
    pub debt_type: DebtType,
    /// File path
    pub file_path: PathBuf,
    /// Line number (if applicable)
    pub line_number: Option<usize>,
    /// Human-readable description
    pub description: String,
    /// Estimated effort in hours
    pub estimated_effort_hours: f64,
    /// Priority (1-10, higher = more urgent)
    pub priority: u8,
    /// Impact on system (1-10)
    pub impact: u8,
    /// Whether it's blocking
    pub is_blocking: bool,
    /// Suggested resolution
    pub resolution: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl DebtItem {
    /// Creates a new debt item.
    #[must_use]
    pub fn new(debt_type: DebtType, file_path: PathBuf, description: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            debt_type,
            file_path,
            line_number: None,
            description: description.into(),
            estimated_effort_hours: 1.0,
            priority: 5,
            impact: 5,
            is_blocking: false,
            resolution: None,
            metadata: HashMap::new(),
        }
    }

    /// Sets the line number.
    #[must_use]
    pub const fn at_line(mut self, line: usize) -> Self {
        self.line_number = Some(line);
        self
    }

    /// Sets the estimated effort.
    #[must_use]
    pub const fn with_effort(mut self, hours: f64) -> Self {
        self.estimated_effort_hours = hours;
        self
    }

    /// Sets the priority.
    #[must_use]
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority.clamp(1, 10);
        self
    }

    /// Sets the impact.
    #[must_use]
    pub fn with_impact(mut self, impact: u8) -> Self {
        self.impact = impact.clamp(1, 10);
        self
    }

    /// Sets whether blocking.
    #[must_use]
    pub const fn blocking(mut self, is_blocking: bool) -> Self {
        self.is_blocking = is_blocking;
        self
    }

    /// Sets the resolution.
    #[must_use]
    pub fn with_resolution(mut self, resolution: impl Into<String>) -> Self {
        self.resolution = Some(resolution.into());
        self
    }

    /// Adds metadata.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Report containing all technical debt.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DebtReport {
    /// All debt items
    pub items: Vec<DebtItem>,
    /// Summary by type
    pub summary_by_type: HashMap<DebtType, usize>,
    /// Total estimated effort
    pub total_effort_hours: f64,
    /// Average priority
    pub average_priority: f64,
    /// Average impact
    pub average_impact: f64,
    /// Blocking items count
    pub blocking_count: usize,
    /// Analysis timestamp
    pub timestamp: String,
}

impl DebtReport {
    /// Creates a new empty debt report.
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            summary_by_type: HashMap::new(),
            total_effort_hours: 0.0,
            average_priority: 0.0,
            average_impact: 0.0,
            blocking_count: 0,
            timestamp: chrono::Local::now().to_rfc3339(),
        }
    }

    /// Adds a debt item to the report.
    pub fn add(&mut self, item: DebtItem) {
        *self.summary_by_type.entry(item.debt_type).or_insert(0) += 1;
        self.total_effort_hours += item.estimated_effort_hours;
        let count = self.items.len();
        if count == 0 {
            self.average_priority = 0.0;
            self.average_impact = 0.0;
        } else {
            let count_f64 = count as f64;
            self.average_priority =
                (self.average_priority * count_f64 + f64::from(item.priority)) / (count_f64 + 1.0);
            self.average_impact =
                (self.average_impact * count_f64 + f64::from(item.impact)) / (count_f64 + 1.0);
        }
        if item.is_blocking {
            self.blocking_count += 1;
        }
        self.items.push(item);
    }

    /// Returns the number of debt items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns true if there are no debt items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns debt items filtered by type.
    #[must_use]
    pub fn by_type(&self, debt_type: DebtType) -> Vec<&DebtItem> {
        self.items
            .iter()
            .filter(|d| d.debt_type == debt_type)
            .collect()
    }

    /// Returns debt items filtered by blocking status.
    #[must_use]
    pub fn blocking_items(&self) -> Vec<&DebtItem> {
        self.items.iter().filter(|d| d.is_blocking).collect()
    }

    /// Returns the total debt score.
    #[must_use]
    pub fn debt_score(&self) -> f64 {
        if self.items.is_empty() {
            return 0.0;
        }
        let mut score = 0.0_f64;
        for item in &self.items {
            score +=
                f64::from(item.priority) * f64::from(item.impact) * item.estimated_effort_hours;
        }
        score / self.items.len() as f64
    }

    /// Returns the top N highest priority items.
    #[must_use]
    pub fn top_priorities(&self, n: usize) -> Vec<&DebtItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted.into_iter().take(n).collect()
    }
}

/// Technical debt analyzer.
#[derive(Debug, Default)]
pub struct DebtAnalyzer {
    /// Debt detection rules
    rules: Vec<DebtRule>,
}

impl DebtAnalyzer {
    /// Creates a new debt analyzer with default rules.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rules: vec![
                DebtRule::todo_fixme(),
                DebtRule::unimplemented(),
                DebtRule::hardcoded_values(),
                DebtRule::magic_numbers(),
                DebtRule::complex_functions(),
                DebtRule::deep_nesting(),
                DebtRule::missing_docs(),
                DebtRule::missing_tests(),
                DebtRule::large_files(),
            ],
        }
    }

    /// Returns the active rules.
    #[must_use]
    pub fn rules(&self) -> &[DebtRule] {
        &self.rules
    }

    /// Analyzes a single file for technical debt.
    pub fn analyze_file(&self, file_path: &PathBuf, content: &str) -> DebtReport {
        let mut report = DebtReport::new();

        for rule in &self.rules {
            if !rule.enabled {
                continue;
            }
            let debts = self.apply_rule(rule, file_path, content);
            for debt in debts {
                report.add(debt);
            }
        }

        report.timestamp = chrono::Local::now().to_rfc3339();
        report
    }

    /// Analyzes multiple files for technical debt.
    pub fn analyze_files<'a>(&self, files: impl Iterator<Item = (PathBuf, &'a str)>) -> DebtReport {
        let mut report = DebtReport::new();

        for (path, content) in files {
            for rule in &self.rules {
                if !rule.enabled {
                    continue;
                }
                let debts = self.apply_rule(rule, &path, content);
                for debt in debts {
                    report.add(debt);
                }
            }
        }

        report.timestamp = chrono::Local::now().to_rfc3339();
        report
    }

    fn apply_rule(&self, rule: &DebtRule, file_path: &PathBuf, content: &str) -> Vec<DebtItem> {
        let mut debts = Vec::new();

        match rule.id.as_str() {
            "todo-fixer" => {
                debts.extend(self.detect_todo_fixme(file_path, content));
            }
            "unimplemented" => {
                debts.extend(self.detect_unimplemented(file_path, content));
            }
            "hardcoded-values" => {
                debts.extend(self.detect_hardcoded_values(file_path, content));
            }
            "magic-numbers" => {
                debts.extend(self.detect_magic_numbers(file_path, content));
            }
            "complex-functions" => {
                debts.extend(self.detect_complex_functions(file_path, content));
            }
            "deep-nesting" => {
                debts.extend(self.detect_deep_nesting(file_path, content));
            }
            "missing-docs" => {
                debts.extend(self.detect_missing_docs(file_path, content));
            }
            "missing-tests" => {
                debts.extend(self.detect_missing_tests(file_path, content));
            }
            "large-files" => {
                debts.extend(self.detect_large_files(file_path, content));
            }
            _ => {}
        }
        debts
    }

    fn detect_todo_fixme(&self, file_path: &PathBuf, content: &str) -> Vec<DebtItem> {
        let mut debts = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            if line.contains("TODO") || line.contains("FIXME") {
                debts.push(
                    DebtItem::new(
                        DebtType::CodeComplexity,
                        file_path.clone(),
                        "Unresolved TODO/FIXME found",
                    )
                    .at_line(line_num + 1)
                    .with_priority(3)
                    .with_impact(2)
                    .with_effort(0.0),
                );
            }
        }

        debts
    }

    fn detect_unimplemented(&self, file_path: &PathBuf, content: &str) -> Vec<DebtItem> {
        let mut debts = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            if line.contains("unimplemented!()") || line.contains("todo!()") {
                debts.push(
                    DebtItem::new(
                        DebtType::CodeComplexity,
                        file_path.clone(),
                        "Unimplemented code found",
                    )
                    .at_line(line_num + 1)
                    .with_priority(8)
                    .with_impact(7)
                    .with_effort(2.0)
                    .blocking(true),
                );
            }
        }

        debts
    }

    fn detect_hardcoded_values(&self, file_path: &PathBuf, content: &str) -> Vec<DebtItem> {
        let mut debts = Vec::new();
        let patterns = ["password", "api_key", "secret"];

        for (line_num, line) in content.lines().enumerate() {
            for pattern in &patterns {
                if line.contains(pattern) {
                    debts.push(
                        DebtItem::new(
                            DebtType::SecurityDebt,
                            file_path.clone(),
                            "Hardcoded value detected",
                        )
                        .at_line(line_num + 1)
                        .with_priority(9)
                        .with_impact(9)
                        .with_effort(0.5)
                        .blocking(true)
                        .with_resolution("Use environment variables or secure storage"),
                    );
                    break;
                }
            }
        }

        debts
    }

    fn detect_magic_numbers(&self, file_path: &PathBuf, content: &str) -> Vec<DebtItem> {
        let mut debts = Vec::new();
        let common_constants = [60, 100, 1000, 255, 256, 512, 1024];

        for (line_num, line) in content.lines().enumerate() {
            if line.contains("const ") || line.trim().starts_with("//") {
                continue;
            }
            for word in line.split_whitespace() {
                if let Ok(num) = word.parse::<u64>() {
                    if !common_constants.contains(&num) {
                        debts.push(
                            DebtItem::new(
                                DebtType::CodeComplexity,
                                file_path.clone(),
                                format!("Magic number detected: {}", num),
                            )
                            .at_line(line_num + 1)
                            .with_priority(4)
                            .with_impact(3)
                            .with_effort(0.25)
                            .with_resolution("Consider using a named constant"),
                        );
                        break;
                    }
                }
            }
        }

        debts
    }

    fn detect_complex_functions(&self, file_path: &PathBuf, content: &str) -> Vec<DebtItem> {
        let mut debts = Vec::new();
        let max_lines = 50;

        let mut in_function = false;
        let mut function_start = 0;
        let mut brace_count = 0;
        let mut function_lines = 0;

        for (line_num, line) in content.lines().enumerate() {
            if line.contains("fn ") && line.contains('{') {
                in_function = true;
                function_start = line_num;
                brace_count = 1;
                function_lines = 1;
            } else if in_function {
                function_lines += 1;
                brace_count += line.matches('{').count() as i32;
                brace_count -= line.matches('}').count() as i32;

                if brace_count == 0 {
                    if function_lines > max_lines {
                        debts.push(
                            DebtItem::new(
                                DebtType::CodeComplexity,
                                file_path.clone(),
                                format!(
                                    "Function exceeds {} lines ({} lines)",
                                    max_lines, function_lines
                                ),
                            )
                            .at_line(function_start)
                            .with_priority(5)
                            .with_impact(4)
                            .with_effort(2.0)
                            .with_resolution("Consider breaking into smaller functions"),
                        );
                    }
                    in_function = false;
                }
            }
        }

        debts
    }

    fn detect_deep_nesting(&self, file_path: &PathBuf, content: &str) -> Vec<DebtItem> {
        let mut debts = Vec::new();
        let max_nesting = 4;

        for (line_num, line) in content.lines().enumerate() {
            let indent = line.len() - line.trim_start().len();
            let nesting = indent / 4;

            if nesting > max_nesting && !line.trim().is_empty() {
                debts.push(
                    DebtItem::new(
                        DebtType::CodeComplexity,
                        file_path.clone(),
                        format!("Deep nesting detected (level {})", nesting),
                    )
                    .at_line(line_num + 1)
                    .with_priority(4)
                    .with_impact(3)
                    .with_effort(1.5)
                    .with_resolution("Consider extracting nested logic into separate functions"),
                );
            }
        }

        debts
    }

    fn detect_missing_docs(&self, file_path: &PathBuf, content: &str) -> Vec<DebtItem> {
        let mut debts = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for i in 0..lines.len().saturating_sub(1) {
            let line = lines[i];
            let trimmed = line.trim();

            if trimmed.starts_with("pub fn ") || trimmed.starts_with("pub async fn ") {
                let has_docs = if i > 0 {
                    let prev = lines[i - 1].trim();
                    prev.starts_with("///") || prev.starts_with("/**")
                } else {
                    false
                };

                if !has_docs {
                    debts.push(
                        DebtItem::new(
                            DebtType::DocumentationDebt,
                            file_path.clone(),
                            "Missing documentation for public function",
                        )
                        .at_line(i + 1)
                        .with_priority(3)
                        .with_impact(2)
                        .with_effort(0.25),
                    );
                }
            }
        }

        debts
    }

    fn detect_missing_tests(&self, file_path: &PathBuf, content: &str) -> Vec<DebtItem> {
        let mut debts = Vec::new();

        if content.contains("#[cfg(test)]") || content.contains("#[test]") {
            return debts;
        }

        let pub_fn_count = content
            .lines()
            .filter(|l| l.trim().starts_with("pub fn ") || l.trim().starts_with("pub async fn "))
            .count();

        if pub_fn_count > 3 {
            debts.push(
                DebtItem::new(
                    DebtType::TestingDebt,
                    file_path.clone(),
                    "File may need more test coverage",
                )
                .with_priority(4)
                .with_impact(5)
                .with_effort(1.0),
            );
        }

        debts
    }

    fn detect_large_files(&self, file_path: &PathBuf, content: &str) -> Vec<DebtItem> {
        let mut debts = Vec::new();
        let line_count = content.lines().count();

        if line_count > 500 {
            debts.push(
                DebtItem::new(
                    DebtType::ArchitectureDebt,
                    file_path.clone(),
                    "Large file detected",
                )
                .with_priority(5)
                .with_impact(4)
                .with_effort(2.0)
                .with_resolution("Consider splitting into smaller modules"),
            );
        } else if line_count > 300 {
            debts.push(
                DebtItem::new(
                    DebtType::ArchitectureDebt,
                    file_path.clone(),
                    "File approaching size limit",
                )
                .with_priority(3)
                .with_impact(3)
                .with_effort(1.5),
            );
        }

        debts
    }
}

/// A debt detection rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtRule {
    /// Unique identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description
    pub description: String,
    /// Type of debt this rule detects
    pub debt_type: DebtType,
    /// Whether enabled
    pub enabled: bool,
}

impl DebtRule {
    /// Creates a new debt rule.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        debt_type: DebtType,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            debt_type,
            enabled: true,
        }
    }

    /// Creates a TODO/FIXME detection rule.
    #[must_use]
    pub fn todo_fixme() -> Self {
        Self::new(
            "todo-fixer",
            "TODO/FIXME Detection",
            "Detects unresolved TODO and FIXME comments",
            DebtType::CodeComplexity,
        )
    }

    /// Creates an unimplemented code detection rule.
    #[must_use]
    pub fn unimplemented() -> Self {
        Self::new(
            "unimplemented",
            "Unimplemented Code",
            "Detects unimplemented!() and todo!() macros",
            DebtType::CodeComplexity,
        )
    }

    /// Creates a hardcoded values detection rule.
    #[must_use]
    pub fn hardcoded_values() -> Self {
        Self::new(
            "hardcoded-values",
            "Hardcoded Values",
            "Detects hardcoded configuration values",
            DebtType::SecurityDebt,
        )
    }

    /// Creates a magic numbers detection rule.
    #[must_use]
    pub fn magic_numbers() -> Self {
        Self::new(
            "magic-numbers",
            "Magic Numbers",
            "Detects unexplained numeric literals",
            DebtType::CodeComplexity,
        )
    }

    /// Creates a complex functions detection rule.
    #[must_use]
    pub fn complex_functions() -> Self {
        Self::new(
            "complex-functions",
            "Complex Functions",
            "Detects functions with high cognitive complexity",
            DebtType::CodeComplexity,
        )
    }

    /// Creates a deep nesting detection rule.
    #[must_use]
    pub fn deep_nesting() -> Self {
        Self::new(
            "deep-nesting",
            "Deep Nesting",
            "Detects deeply nested code blocks",
            DebtType::CodeComplexity,
        )
    }

    /// Creates a missing docs detection rule.
    #[must_use]
    pub fn missing_docs() -> Self {
        Self::new(
            "missing-docs",
            "Missing Documentation",
            "Detects public functions without documentation",
            DebtType::DocumentationDebt,
        )
    }

    /// Creates a missing tests detection rule.
    #[must_use]
    pub fn missing_tests() -> Self {
        Self::new(
            "missing-tests",
            "Missing Tests",
            "Detects files that may need more test coverage",
            DebtType::TestingDebt,
        )
    }

    /// Creates a large files detection rule.
    #[must_use]
    pub fn large_files() -> Self {
        Self::new(
            "large-files",
            "Large Files",
            "Detects files that are too large",
            DebtType::ArchitectureDebt,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debt_type_default() {
        let debt_type = DebtType::default();
        assert_eq!(debt_type, DebtType::CodeComplexity);
    }

    #[test]
    fn test_debt_item_creation() {
        let item = DebtItem::new(
            DebtType::CodeComplexity,
            PathBuf::from("test.rs"),
            "Test debt",
        );
        assert!(!item.id.is_empty()); // UUID is generated
        assert_eq!(item.debt_type, DebtType::CodeComplexity);
        assert_eq!(item.file_path, PathBuf::from("test.rs"));
        assert_eq!(item.description, "Test debt");
        assert_eq!(item.priority, 5);
    }

    #[test]
    fn test_debt_item_modifiers() {
        let item = DebtItem::new(DebtType::TestingDebt, PathBuf::from("test.rs"), "Test")
            .at_line(42)
            .with_priority(8)
            .with_impact(9)
            .blocking(true);

        assert_eq!(item.line_number, Some(42));
        assert_eq!(item.priority, 8);
        assert_eq!(item.impact, 9);
        assert!(item.is_blocking);
    }

    #[test]
    fn test_debt_report() {
        let mut report = DebtReport::new();
        assert!(report.is_empty());
        assert_eq!(report.len(), 0);

        report.add(DebtItem::new(
            DebtType::CodeComplexity,
            PathBuf::from("a.rs"),
            "Debt 1",
        ));
        report.add(
            DebtItem::new(DebtType::TestingDebt, PathBuf::from("b.rs"), "Debt 2").blocking(true),
        );

        assert_eq!(report.len(), 2);
        assert_eq!(report.blocking_items().len(), 1);
        assert!(report.total_effort_hours > 0.0);
    }

    #[test]
    fn test_debt_report_by_type() {
        let mut report = DebtReport::new();
        report.add(DebtItem::new(
            DebtType::CodeComplexity,
            PathBuf::from("a.rs"),
            "Debt 1",
        ));
        report.add(DebtItem::new(
            DebtType::CodeComplexity,
            PathBuf::from("b.rs"),
            "Debt 2",
        ));
        report.add(DebtItem::new(
            DebtType::TestingDebt,
            PathBuf::from("c.rs"),
            "Debt 3",
        ));

        let complexity = report.by_type(DebtType::CodeComplexity);
        assert_eq!(complexity.len(), 2);

        let testing = report.by_type(DebtType::TestingDebt);
        assert_eq!(testing.len(), 1);
    }

    #[test]
    fn test_debt_analyzer_creation() {
        let analyzer = DebtAnalyzer::new();
        assert!(!analyzer.rules().is_empty());
    }

    #[test]
    fn test_debt_analyzer_analyze_file_todo() {
        let analyzer = DebtAnalyzer::new();
        let content = r#"
fn main() {
    // TODO: implement this
    println!("Hello");
}
"#;
        let report = analyzer.analyze_file(&PathBuf::from("test.rs"), content);
        assert!(!report.is_empty());
        assert!(report
            .items
            .iter()
            .any(|d| d.debt_type == DebtType::CodeComplexity));
    }

    #[test]
    fn test_debt_analyzer_analyze_file_unimplemented() {
        let analyzer = DebtAnalyzer::new();
        let content = r#"
fn main() {
    unimplemented!()
}
"#;
        let report = analyzer.analyze_file(&PathBuf::from("test.rs"), content);
        assert!(report.items.iter().any(|d| d.is_blocking));
    }

    #[test]
    fn test_debt_analyzer_analyze_file_hardcoded() {
        let analyzer = DebtAnalyzer::new();
        let content = r#"
fn main() {
    let password = "secret123";
}
"#;
        let report = analyzer.analyze_file(&PathBuf::from("test.rs"), content);
        assert!(report
            .items
            .iter()
            .any(|d| d.debt_type == DebtType::SecurityDebt));
    }

    #[test]
    fn test_debt_report_score() {
        let mut report = DebtReport::new();
        report.add(
            DebtItem::new(DebtType::CodeComplexity, PathBuf::from("a.rs"), "Debt")
                .with_priority(8)
                .with_impact(7)
                .with_effort(2.0),
        );
        let score = report.debt_score();
        assert!(score > 0.0);
    }

    #[test]
    fn test_debt_report_top_priorities() {
        let mut report = DebtReport::new();
        report.add(
            DebtItem::new(DebtType::CodeComplexity, PathBuf::from("a.rs"), "Low").with_priority(2),
        );
        report.add(
            DebtItem::new(DebtType::TestingDebt, PathBuf::from("b.rs"), "High").with_priority(9),
        );

        let top = report.top_priorities(1);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].priority, 9);
    }
}
