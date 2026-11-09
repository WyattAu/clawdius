//! Metamorphic oracle synthesis (blueprint item 50).
//!
//! When a repo has no ground-truth tests, the agent synthesizes
//! *metamorphic relations* — transformations on inputs where the relation
//! between outputs is known a priori — and emits executable Rust test
//! stubs asserting those relations. Relation classes follow the standard
//! taxonomy (Chen et al.): idempotence, involution, delta-invariance,
//! symmetry, and zero/identity preservation.

use serde_json::Value;

/// Metamorphic relation classes with known a-priori output relations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MrClass {
    /// `f(f(x)) == f(x)` — running twice changes nothing.
    Idempotence,
    /// `f⁻¹(f(x)) == x` — applying the inverse undoes the transform
    /// (requires an inverse function name).
    Involution,
    /// `f(x + d) == f(x) + d'` — shifting the input shifts the output
    /// predictably (translation invariance).
    DeltaInvariance,
    /// `f(a, b) == f(b, a)` — argument order does not matter.
    Symmetry,
    /// `f(zero) == identity` — the absorbing/identity input maps to the
    /// documented identity output.
    ZeroPreservation,
}

/// A synthesized metamorphic test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynthesizedTest {
    /// Test function name (snake_case, unique per relation).
    pub name: String,
    /// The emitted Rust source (complete `#[test] fn`).
    pub source: String,
    /// Human-readable relation description for the PR/summary.
    pub relation: String,
}

/// Sanitize an identifier fragment into a safe snake_case token.
#[must_use]
pub fn ident(fragment: &str) -> String {
    let cleaned: String = fragment
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let collapsed = cleaned
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    let out = if collapsed.is_empty() {
        "subject".to_string()
    } else {
        collapsed
    };
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("f_{out}")
    } else {
        out
    }
}

/// Emit the metamorphic test stubs for `fn_name` across the requested
/// relation classes.
///
/// The stubs assert the a-priori relations with placeholder inputs
/// (`$INPUT`) the caller replaces with real values before use.
#[must_use]
pub fn synthesize(fn_name: &str, classes: &[MrClass]) -> Vec<SynthesizedTest> {
    let fname = ident(fn_name);
    classes
        .iter()
        .map(|class| match class {
            MrClass::Idempotence => SynthesizedTest {
                name: format!("{fname}_idempotent"),
                relation: "f(f(x)) == f(x)".to_string(),
                source: format!(
                    "#[test]\nfn {fname}_idempotent() {{\n    let x = /* INPUT */;\n    \
                     let once = {fname}(x.clone());\n    \
                     let twice = {fname}(once.clone());\n    \
                     assert_eq!(twice, once, \"f(f(x)) must equal f(x)\");\n}}"
                ),
            },
            MrClass::Involution => SynthesizedTest {
                name: format!("{fname}_involution"),
                relation: "inv(f(x)) == x".to_string(),
                source: format!(
                    "#[test]\nfn {fname}_involution() {{\n    let x = /* INPUT */;\n    \
                     let transformed = {fname}(x.clone());\n    \
                     let restored = {fname}_inverse(transformed);\n    \
                     assert_eq!(restored, x, \"inv(f(x)) must equal x\");\n}}"
                ),
            },
            MrClass::DeltaInvariance => SynthesizedTest {
                name: format!("{fname}_delta_invariant"),
                relation: "f(x + d) == f(x) + d".to_string(),
                source: format!(
                    "#[test]\nfn {fname}_delta_invariant() {{\n    let x = /* INPUT */;\n    \
                     let d = /* DELTA */;\n    \
                     assert_eq!({fname}(x + d), {fname}(x) + d, \"delta must transfer\");\n}}"
                ),
            },
            MrClass::Symmetry => SynthesizedTest {
                name: format!("{fname}_symmetric"),
                relation: "f(a, b) == f(b, a)".to_string(),
                source: format!(
                    "#[test]\nfn {fname}_symmetric() {{\n    let a = /* INPUT */;\n    \
                     let b = /* INPUT */;\n    \
                     assert_eq!({fname}(a, b), {fname}(b, a), \"must be symmetric\");\n}}"
                ),
            },
            MrClass::ZeroPreservation => SynthesizedTest {
                name: format!("{fname}_zero_preserving"),
                relation: "f(0) == identity".to_string(),
                source: format!(
                    "#[test]\nfn {fname}_zero_preserving() {{\n    \
                     assert_eq!({fname}(Default::default()), /* IDENTITY */, \
                     \"zero input must yield the identity output\");\n}}"
                ),
            },
        })
        .collect()
}

/// Parse an MR class list from a JSON array of strings
/// (`"idempotence" | "involution" | "delta" | "symmetry" | "zero"`).
#[must_use]
pub fn parse_classes(value: &Value) -> Vec<MrClass> {
    let mut out = Vec::new();
    if let Some(arr) = value.as_array() {
        for item in arr {
            if let Some(s) = item.as_str() {
                match s.to_lowercase().as_str() {
                    "idempotence" | "idempotent" => out.push(MrClass::Idempotence),
                    "involution" | "inverse" => out.push(MrClass::Involution),
                    "delta" | "delta_invariance" => out.push(MrClass::DeltaInvariance),
                    "symmetry" | "symmetric" => out.push(MrClass::Symmetry),
                    "zero" | "zero_preservation" => out.push(MrClass::ZeroPreservation),
                    _ => {},
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn synthesizes_all_requested_relations() {
        let tests = synthesize(
            "normalize_path",
            &[MrClass::Idempotence, MrClass::ZeroPreservation],
        );
        assert_eq!(tests.len(), 2);
        assert_eq!(tests[0].name, "normalize_path_idempotent");
        assert!(tests[0].source.contains("normalize_path(once.clone())"));
        assert!(tests[1].relation.contains("identity"));
    }

    #[test]
    fn identifiers_are_sanitized() {
        assert_eq!(ident("fix the 2 failing tests!"), "fix_the_2_failing_tests");
        assert_eq!(ident(""), "subject");
        assert_eq!(ident("123abc"), "f_123abc");
    }

    #[test]
    fn parses_class_list_from_json() {
        let classes = parse_classes(&json!(["idempotence", "symmetry", "unknown-class"]));
        assert_eq!(classes, vec![MrClass::Idempotence, MrClass::Symmetry]);
    }

    #[test]
    fn emitted_sources_are_distinct_per_class() {
        let tests = synthesize("sort_items", &[MrClass::Idempotence, MrClass::Symmetry]);
        assert_ne!(tests[0].source, tests[1].source);
        assert!(tests[1].source.contains("sort_items(a, b)"));
    }
}
