//! Test impact analysis (blueprint item 2): map changed source files to
//! the tests that exercise them.
//!
//! Heuristic two-signal scorer over a test inventory (e.g. from
//! `cargo test -- --list`): (a) file-stem match — a test module named
//! after a changed file's stem, (b) path-segment match — the test's
//! declared module path contains a changed file's stem. Deterministic,
//! zero network, zero LLM.

use std::collections::BTreeMap;

/// One scored test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactedTest {
    /// Full test path (e.g. `session::tests::test_resume`).
    pub test_path: String,
    /// Score: number of distinct changed-file stems it references.
    pub score: usize,
}

/// Score the test inventory against changed files.
///
/// `changed_files` are relative paths (`src/session/manager.rs`);
/// `test_inventory` are full test paths (`session::tests::test_resume`).
/// Returns tests with score > 0, sorted by descending score then path.
#[must_use]
pub fn rank_impacted(changed_files: &[String], test_inventory: &[String]) -> Vec<ImpactedTest> {
    let stems: Vec<String> = changed_files
        .iter()
        .filter_map(|f| PathStem::from(f))
        .collect();

    let mut scored: Vec<ImpactedTest> = test_inventory
        .iter()
        .map(|test| {
            let mut score = 0usize;
            for stem in &stems {
                if test_path_matches(test, stem) {
                    score += 1;
                }
            }
            ImpactedTest {
                test_path: (*test).clone(),
                score,
            }
        })
        .filter(|t| t.score > 0)
        .collect();
    scored.sort_by(|a, b| b.score.cmp(&a.score).then(a.test_path.cmp(&b.test_path)));
    scored
}

/// Rank from a `cargo test -- --list` style mapping: test path → source
/// file that declares it (stronger signal than name heuristics).
#[must_use]
pub fn rank_from_source_map(
    changed_files: &[String],
    source_map: &BTreeMap<String, String>,
) -> Vec<ImpactedTest> {
    let mut scored: Vec<ImpactedTest> = Vec::new();
    for (test, source) in source_map {
        let mut score = 0usize;
        for changed in changed_files {
            if path_overlaps(source, changed) {
                score += 1;
            }
        }
        if score > 0 {
            scored.push(ImpactedTest {
                test_path: (*test).clone(),
                score,
            });
        }
    }
    scored.sort_by(|a, b| b.score.cmp(&a.score).then(a.test_path.cmp(&b.test_path)));
    scored
}

/// True when two paths refer to overlapping module trees (one's module
/// segments are a prefix/suffix of the other's path segments).
fn path_overlaps(a: &str, b: &str) -> bool {
    let norm = |p: &str| p.replace('\\', "/").to_lowercase();
    let (a, b) = (norm(a), norm(b));
    a == b || a.contains(&b) || b.contains(&a) || stem_of(&a) == stem_of(&b)
}

fn stem_of(path: &str) -> String {
    let file = path.rsplit('/').next().unwrap_or(path);
    file.split('.').next().unwrap_or(file).to_lowercase()
}

fn stem_of_checked(path: &str) -> String {
    stem_of(path)
}

/// File stem from a path (`src/session/manager.rs` → `manager`).
#[must_use]
pub fn file_stem(path: &str) -> String {
    stem_of_checked(path)
}

/// Whether a dotted test path references a stem as a segment (e.g. stem
/// `manager` matches `session::manager::tests::resume`).
fn test_path_matches(test_path: &str, stem: &str) -> bool {
    let dotted = test_path.replace("::", ".");
    let segments: Vec<&str> = dotted.split('.').collect();
    segments.iter().any(|seg| seg.eq_ignore_ascii_case(stem))
}

struct PathStem;

impl PathStem {
    fn from(path: &str) -> Option<String> {
        let stem = stem_of_checked(path);
        if stem.is_empty() {
            None
        } else {
            Some(stem)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn ranks_by_stem_reference() {
        let changed = vec!["src/session/manager.rs".to_string()];
        let inventory = vec![
            "session::manager::tests::resume".to_string(),
            "session::tests::pool".to_string(),
            "tools::file::tests::read".to_string(),
        ];
        let ranked = rank_impacted(&changed, &inventory);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].test_path, "session::manager::tests::resume");
    }

    #[test]
    fn multiple_changes_score_higher() {
        let changed = vec![
            "src/session/manager.rs".to_string(),
            "src/tools/search.rs".to_string(),
        ];
        let inventory = vec![
            "tools::search::tests::grep".to_string(),
            "session::manager::tests::resume".to_string(),
            "unrelated::tests::x".to_string(),
        ];
        let ranked = rank_impacted(&changed, &inventory);
        assert_eq!(ranked.len(), 2);
        assert!(ranked[0].score >= ranked[1].score);
    }

    #[test]
    fn source_map_overlap_counts() {
        let mut map = BTreeMap::new();
        map.insert(
            "fleet::tests::journal_roundtrip".to_string(),
            "crates/clawdius-core/src/session/journal.rs".to_string(),
        );
        map.insert(
            "fleet::tests::unrelated".to_string(),
            "crates/clawdius-core/src/tools/file.rs".to_string(),
        );
        let changed = vec!["crates/clawdius-core/src/session/journal.rs".to_string()];
        let ranked = rank_from_source_map(&changed, &map);
        assert_eq!(ranked.len(), 1);
        assert!(ranked[0].test_path.contains("journal"));
    }

    #[test]
    fn file_stem_extracts() {
        assert_eq!(file_stem("src/session/manager.rs"), "manager");
        assert_eq!(file_stem("README.md"), "readme");
    }
}
