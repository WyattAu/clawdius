//! Oracle protection + suppression quotas (blueprint items 15/18, and
//! the convergent "append-only oracle" item).
//!
//! When the agent is tasked with a repair, the existing test suite is an
//! immutable oracle: diffs that delete assertions, weaken `assert!`
//! strength, skip tests (`#[ignore]`), or add lint-suppression
//! directives are flagged before the change lands.

use serde_json::Value;

/// One oracle-protection violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Kind: `assertion_removed`, `test_skipped`, `suppression_added`.
    pub kind: String,
    /// The offending line.
    pub line: String,
}

const SUPPRESSION_MARKERS: [&str; 5] = [
    "#[ignore]",
    "#[allow(dead_code)]",
    "allow(clippy)",
    "# noqa",
    "eslint-disable",
];

/// Compare previous vs new test-file content and return violations.
///
/// `assertions_before`/`assertions_after` are the counts of assertion
/// lines; a decrease is a violation. Suppression markers newly added
/// (present in new, absent in old) are violations.
#[must_use]
pub fn guard_test_diff(
    old_content: &str,
    new_content: &str,
    assertions_before: usize,
    assertions_after: usize,
) -> Vec<Violation> {
    let mut violations = Vec::new();

    if assertions_after < assertions_before {
        violations.push(Violation {
            kind: "assertion_removed".to_string(),
            line: format!("assertion count decreased: {assertions_before} -> {assertions_after}"),
        });
    }

    let old_suppressions: Vec<&str> = old_content
        .lines()
        .filter(|l| SUPPRESSION_MARKERS.iter().any(|m| l.contains(m)))
        .collect();
    let new_suppressions: Vec<&str> = new_content
        .lines()
        .filter(|l| SUPPRESSION_MARKERS.iter().any(|m| l.contains(m)))
        .collect();
    if new_suppressions.len() > old_suppressions.len() {
        for line in new_suppressions.iter().skip(old_suppressions.len()) {
            violations.push(Violation {
                kind: "suppression_added".to_string(),
                line: (*line).to_string(),
            });
        }
    }

    // Test-skip markers newly added.
    let old_skips =
        old_content.matches("#[ignore]").count() + old_content.matches("#[cfg(skip)]").count();
    let new_skips =
        new_content.matches("#[ignore]").count() + new_content.matches("#[cfg(skip)]").count();
    if new_skips > old_skips {
        violations.push(Violation {
            kind: "test_skipped".to_string(),
            line: "#[ignore] newly added".to_string(),
        });
    }

    violations
}

/// Count assertion-shaped lines in a test file (`assert`, `assert_eq`,
/// `assert_matches`, `expect(...)`).
#[must_use]
pub fn count_assertions(content: &str) -> usize {
    content
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            (t.starts_with("assert") || t.contains(".expect(")) && !t.contains("assertion count")
        })
        .count()
}

/// JSON-decode an LLM tool-arguments payload into a violation list for a
/// test-file edit: `{ old_content, new_content }` plus computed counts.
#[must_use]
pub fn guard_from_args(old_content: &str, new_content: &str) -> Vec<Violation> {
    guard_test_diff(
        old_content,
        new_content,
        count_assertions(old_content),
        count_assertions(new_content),
    )
}

/// Parse guard violations from an LLM tool-arguments JSON payload
/// (`{old_content, new_content}` strings) — for direct engine dispatch.
#[must_use]
pub fn guard_from_json(args: &Value) -> Vec<Violation> {
    let old = args
        .get("old_content")
        .and_then(Value::as_str)
        .unwrap_or("");
    let new = args
        .get("new_content")
        .and_then(Value::as_str)
        .unwrap_or("");
    guard_test_diff(old, new, count_assertions(old), count_assertions(new))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assertion_removal_is_violation() {
        let old = "fn t() {\n    assert_eq!(add(2, 3), 5);\n    assert!(add(0, 0) == 0);\n}";
        let new = "fn t() {\n    assert_eq!(add(2, 3), 5);\n}";
        let v = guard_test_diff(old, new, 2, 1);
        assert!(v.iter().any(|x| x.kind == "assertion_removed"));
    }

    #[test]
    fn suppression_addition_is_violation() {
        let old = "fn t() { assert!(true); }";
        let new = "#[allow(clippy::all)]\nfn t() { assert!(true); }\n// eslint-disable eqeqeq";
        let v = guard_test_diff(old, new, 1, 1);
        assert!(v.iter().any(|x| x.kind == "suppression_added"));
    }

    #[test]
    fn clean_edits_pass() {
        let old = "fn t() { assert_eq!(add(2, 3), 5); }";
        let new = "fn t() { assert_eq!(add(2, 3), 5); assert!(add(1, 1) == 2); }";
        let v = guard_test_diff(old, new, 1, 2);
        assert!(v.is_empty());
    }

    #[test]
    fn json_payload_parse_and_guard() {
        let args = serde_json::json!({
            "old_content": "assert_eq!(a, 1);\nassert_eq!(b, 2);",
            "new_content": "assert_eq!(a, 1);"
        });
        let v = guard_from_json(&args);
        assert!(v.iter().any(|x| x.kind == "assertion_removed"));
    }
}
