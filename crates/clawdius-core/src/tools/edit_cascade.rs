//! Edit Cascade: Multi-strategy fuzzy matching for `edit_file` operations.
//!
//! When an LLM generates an edit, the `old_string` may not match exactly due to:
//! - Whitespace differences (tabs vs spaces, trailing whitespace)
//! - Line ending differences (LF vs CRLF)
//! - The file having changed since the LLM last read it
//! - Minor formatting drift
//!
//! This module implements a cascade of 5 matching strategies, tried in order:
//!
//! 1. **Exact** — `str::contains` / `str::replacen` (current behavior, zero overhead)
//! 2. **Whitespace-tolerant** — Normalize whitespace then match
//! 3. **Line-number anchored** — Use line numbers from old_string to find approximate region
//! 4. **Prefix/suffix anchored** — Match on unique first/last lines, allow middle to differ
//! 5. **Fuzzy** — Aho-Corasick multi-pattern search with Levenshtein distance on candidates
//!
//! Each strategy returns a [`MatchResult`] indicating success/failure and the byte range
//! to replace. The cascade short-circuits on the first successful match.

use std::fmt;

/// Result of a matching attempt.
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// Byte offset in the file content where the match starts.
    pub start: usize,
    /// Byte offset where the match ends (exclusive).
    pub end: usize,
    /// Which strategy produced this match.
    pub strategy: Strategy,
    /// Confidence score 0.0-1.0 (1.0 = exact match).
    pub confidence: f64,
}

impl fmt::Display for MatchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MatchResult(strategy={}, start={}, end={}, confidence={:.2})",
            self.strategy, self.start, self.end, self.confidence
        )
    }
}

/// The 5 matching strategies, tried in cascade order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, fmt::Display)]
pub enum Strategy {
    /// Exact substring match (zero tolerance).
    Exact,
    /// Whitespace-normalized match.
    WhitespaceTolerant,
    /// Line-number anchored approximate match.
    LineNumberAnchored,
    /// Prefix/suffix anchored with fuzzy middle.
    PrefixSuffixAnchored,
    /// Fuzzy match using Aho-Corasick + Levenshtein.
    Fuzzy,
}

/// Error information when no strategy matches.
#[derive(Debug, Clone)]
pub struct NoMatchError {
    /// Diagnostics from each strategy that was tried.
    pub diagnostics: Vec<StrategyDiagnostic>,
}

impl fmt::Display for NoMatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "No matching strategy found for edit. Diagnostics:")?;
        for d in &self.diagnostics {
            writeln!(f, "  [{}] {}", d.strategy, d.reason)?;
        }
        Ok(())
    }
}

/// Diagnostic from a single strategy attempt.
#[derive(Debug, Clone)]
pub struct StrategyDiagnostic {
    pub strategy: Strategy,
    pub reason: String,
}

/// Configuration for the edit cascade.
#[derive(Debug, Clone)]
pub struct CascadeConfig {
    /// Whether to try whitespace-tolerant matching.
    pub enable_whitespace_tolerant: bool,
    /// Whether to try line-number anchored matching.
    pub enable_line_number_anchored: bool,
    /// Whether to try prefix/suffix anchored matching.
    pub enable_prefix_suffix_anchored: bool,
    /// Whether to try fuzzy matching.
    pub enable_fuzzy: bool,
    /// Minimum confidence threshold for accepting a non-exact match (0.0-1.0).
    pub min_confidence: f64,
    /// Maximum Levenshtein distance for fuzzy matching (0 = exact, higher = more lenient).
    pub max_levenshtein_distance: usize,
}

impl Default for CascadeConfig {
    fn default() -> Self {
        Self {
            enable_whitespace_tolerant: true,
            enable_line_number_anchored: true,
            enable_prefix_suffix_anchored: true,
            enable_fuzzy: true,
            min_confidence: 0.6,
            max_levenshtein_distance: 10,
        }
    }
}

/// Edit cascade parameters.
pub struct EditParams<'a> {
    /// The full file content.
    pub content: &'a str,
    /// The old text the LLM wants to replace.
    pub old_text: &'a str,
    /// The new text to insert.
    pub new_text: &'a str,
    /// Whether to replace all occurrences (only works with Exact strategy).
    pub replace_all: bool,
}

/// Result of applying the edit cascade.
pub struct EditCascadeResult {
    /// The modified file content.
    pub new_content: String,
    /// Which strategy was used.
    pub strategy: Strategy,
    /// Confidence of the match.
    pub confidence: f64,
    /// Number of replacements made.
    pub replacements: usize,
}

/// Run the edit cascade: try each strategy in order, apply the first match.
///
/// Returns `Ok(EditCascadeResult)` on success, `Err(NoMatchError)` if no strategy matches.
pub fn apply_edit_cascade(params: &EditParams<'_>) -> Result<EditCascadeResult, NoMatchError> {
    let config = CascadeConfig::default();
    apply_edit_cascade_with_config(params, &config)
}

/// Run the edit cascade with custom configuration.
pub fn apply_edit_cascade_with_config(
    params: &EditParams<'_>,
    config: &CascadeConfig,
) -> Result<EditCascadeResult, NoMatchError> {
    let mut diagnostics = Vec::new();

    // Strategy 1: Exact match (always enabled, zero overhead)
    if let Some(result) = try_exact(params) {
        return Ok(apply_match(params, &result));
    }

    // Strategy 2: Whitespace-tolerant
    if config.enable_whitespace_tolerant {
        match try_whitespace_tolerant(params) {
            Ok(Some(result)) if result.confidence >= config.min_confidence => {
                return Ok(apply_match(params, &result));
            }
            Ok(Some(result)) => {
                diagnostics.push(StrategyDiagnostic {
                    strategy: Strategy::WhitespaceTolerant,
                    reason: format!(
                        "matched but confidence {:.2} below threshold {:.2}",
                        result.confidence, config.min_confidence
                    ),
                });
            }
            Ok(None) => {
                diagnostics.push(StrategyDiagnostic {
                    strategy: Strategy::WhitespaceTolerant,
                    reason: "no match after whitespace normalization".into(),
                });
            }
            Err(e) => {
                diagnostics.push(StrategyDiagnostic {
                    strategy: Strategy::WhitespaceTolerant,
                    reason: format!("error: {e}"),
                });
            }
        }
    }

    // Strategy 3: Line-number anchored
    if config.enable_line_number_anchored {
        match try_line_number_anchored(params) {
            Ok(Some(result)) if result.confidence >= config.min_confidence => {
                return Ok(apply_match(params, &result));
            }
            Ok(Some(result)) => {
                diagnostics.push(StrategyDiagnostic {
                    strategy: Strategy::LineNumberAnchored,
                    reason: format!(
                        "matched but confidence {:.2} below threshold {:.2}",
                        result.confidence, config.min_confidence
                    ),
                });
            }
            Ok(None) => {
                diagnostics.push(StrategyDiagnostic {
                    strategy: Strategy::LineNumberAnchored,
                    reason: "no line number anchors found in old_text".into(),
                });
            }
            Err(e) => {
                diagnostics.push(StrategyDiagnostic {
                    strategy: Strategy::LineNumberAnchored,
                    reason: format!("error: {e}"),
                });
            }
        }
    }

    // Strategy 4: Prefix/suffix anchored
    if config.enable_prefix_suffix_anchored {
        match try_prefix_suffix_anchored(params) {
            Ok(Some(result)) if result.confidence >= config.min_confidence => {
                return Ok(apply_match(params, &result));
            }
            Ok(Some(result)) => {
                diagnostics.push(StrategyDiagnostic {
                    strategy: Strategy::PrefixSuffixAnchored,
                    reason: format!(
                        "matched but confidence {:.2} below threshold {:.2}",
                        result.confidence, config.min_confidence
                    ),
                });
            }
            Ok(None) => {
                diagnostics.push(StrategyDiagnostic {
                    strategy: Strategy::PrefixSuffixAnchored,
                    reason: "no unique prefix/suffix anchors found".into(),
                });
            }
            Err(e) => {
                diagnostics.push(StrategyDiagnostic {
                    strategy: Strategy::PrefixSuffixAnchored,
                    reason: format!("error: {e}"),
                });
            }
        }
    }

    // Strategy 5: Fuzzy
    if config.enable_fuzzy {
        match try_fuzzy(params, config.max_levenshtein_distance) {
            Ok(Some(result)) if result.confidence >= config.min_confidence => {
                return Ok(apply_match(params, &result));
            }
            Ok(Some(result)) => {
                diagnostics.push(StrategyDiagnostic {
                    strategy: Strategy::Fuzzy,
                    reason: format!(
                        "matched but confidence {:.2} below threshold {:.2}",
                        result.confidence, config.min_confidence
                    ),
                });
            }
            Ok(None) => {
                diagnostics.push(StrategyDiagnostic {
                    strategy: Strategy::Fuzzy,
                    reason: "no fuzzy match within distance threshold".into(),
                });
            }
            Err(e) => {
                diagnostics.push(StrategyDiagnostic {
                    strategy: Strategy::Fuzzy,
                    reason: format!("error: {e}"),
                });
            }
        }
    }

    // Add diagnostic for exact match failure
    diagnostics.insert(
        0,
        StrategyDiagnostic {
            strategy: Strategy::Exact,
            reason: "exact substring not found in content".into(),
        },
    );

    Err(NoMatchError { diagnostics })
}

// =============================================================================
// Strategy 1: Exact Match
// =============================================================================

fn try_exact(params: &EditParams<'_>) -> Option<MatchResult> {
    if params.replace_all {
        // For replace_all, we still need exact — fuzzy doesn't make sense
        if params.content.contains(params.old_text) {
            // Return the first match position
            let start = params.content.find(params.old_text)?;
            return Some(MatchResult {
                start,
                end: start + params.old_text.len(),
                strategy: Strategy::Exact,
                confidence: 1.0,
            });
        }
        return None;
    }

    let start = params.content.find(params.old_text)?;
    Some(MatchResult {
        start,
        end: start + params.old_text.len(),
        strategy: Strategy::Exact,
        confidence: 1.0,
    })
}

// =============================================================================
// Strategy 2: Whitespace-Tolerant Match
// =============================================================================

/// Normalize whitespace in a string for comparison:
/// - Collapse runs of spaces/tabs into a single space
/// - Strip leading/trailing whitespace from each line
/// - Normalize line endings to LF
fn normalize_whitespace(s: &str) -> String {
    s.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn try_whitespace_tolerant(params: &EditParams<'_>) -> Result<Option<MatchResult>, String> {
    let normalized_old = normalize_whitespace(params.old_text);
    if normalized_old.is_empty() {
        return Err("old_text is empty after whitespace normalization".into());
    }

    // Build normalized version of content, but keep track of byte offsets
    let lines: Vec<&str> = params.content.lines().collect();
    let old_line_count = normalized_old.lines().count();

    if old_line_count > lines.len() {
        return Ok(None);
    }

    // Slide a window over the content's lines, normalize each window, compare
    for window_start in 0..=lines.len().saturating_sub(old_line_count) {
        let window = &lines[window_start..window_start + old_line_count];
        let normalized_window: String = window
            .iter()
            .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join("\n");

        if normalized_window == normalized_old {
            // Compute byte offsets for the matched region
            let byte_start = lines[..window_start]
                .iter()
                .map(|l| l.len() + 1) // +1 for newline
                .sum::<usize>();
            let byte_end = lines[..window_start + old_line_count]
                .iter()
                .map(|l| l.len() + 1)
                .sum::<usize>()
                .saturating_sub(1); // don't include trailing newline

            // Compute confidence based on how much whitespace was different
            let original_window: String = window.join("\n");
            let distance = levenshtein(&original_window, params.old_text);
            let max_distance = original_window.len().max(params.old_text.len());
            let confidence = if max_distance == 0 {
                1.0
            } else {
                1.0 - (distance as f64 / max_distance as f64)
            };

            return Ok(Some(MatchResult {
                start: byte_start,
                end: byte_end,
                strategy: Strategy::WhitespaceTolerant,
                confidence,
            }));
        }
    }

    Ok(None)
}

// =============================================================================
// Strategy 3: Line-Number Anchored
// =============================================================================

/// Try to extract line numbers from patterns like `42: content` or `// line 42`.
fn extract_line_hints(old_text: &str) -> Vec<(usize, &str)> {
    let mut hints = Vec::new();
    for line in old_text.lines() {
        // Pattern: "42: " or "42| " or "  42: "
        let trimmed = line.trim_start();
        if let Some(colon_pos) = trimmed.find(':') {
            let prefix = &trimmed[..colon_pos];
            if let Ok(line_num) = prefix.trim().parse::<usize>() {
                if line_num > 0 {
                    hints.push((line_num, trimmed));
                }
            }
        }
    }
    hints
}

fn try_line_number_anchored(params: &EditParams<'_>) -> Result<Option<MatchResult>, String> {
    let hints = extract_line_hints(params.old_text);
    if hints.is_empty() {
        return Ok(None);
    }

    let content_lines: Vec<&str> = params.content.lines().collect();

    // Try each line hint: the line number in old_text should correspond to
    // a similar line in the actual content
    for (hint_line_num, hint_content) in &hints {
        // Line numbers in old_text are typically from the LLM's perspective
        // Try to find this line in the content (with some offset tolerance)
        let search_start = hint_line_num.saturating_sub(3).saturating_sub(1);
        let search_end = (hint_line_num + 3).min(content_lines.len());

        for i in search_start..search_end {
            if i >= content_lines.len() {
                continue;
            }
            let actual_line = content_lines[i].trim();
            let hint_trimmed = hint_content.trim_start_matches(|c: char| c.is_ascii_digit() || c == ':' || c == '|');
            let hint_trimmed = hint_trimmed.trim();

            if !hint_trimmed.is_empty() && actual_line.contains(hint_trimmed) {
                // Found the anchor line! Now try to match the full old_text
                // starting from a few lines before the anchor
                let old_lines: Vec<&str> = params.old_text.lines().collect();
                // Estimate where old_text starts relative to the anchor
                let anchor_in_old = old_lines.iter().position(|l| *l == *hint_content);
                if let Some(anchor_offset) = anchor_in_old {
                    let estimated_start = i.saturating_sub(anchor_offset);
                    let estimated_end = (estimated_start + old_lines.len()).min(content_lines.len());

                    if estimated_start < content_lines.len() && estimated_end <= content_lines.len() {
                        // Compute byte range
                        let byte_start = content_lines[..estimated_start]
                            .iter()
                            .map(|l| l.len() + 1)
                            .sum::<usize>();
                        let byte_end = content_lines[..estimated_end]
                            .iter()
                            .map(|l| l.len() + 1)
                            .sum::<usize>()
                            .saturating_sub(1);

                        let actual_region: String =
                            content_lines[estimated_start..estimated_end].join("\n");
                        let distance = levenshtein(&actual_region, params.old_text);
                        let max_len = actual_region.len().max(params.old_text.len());
                        let confidence = if max_len == 0 {
                            1.0
                        } else {
                            1.0 - (distance as f64 / max_len as f64)
                        };

                        // Accept if reasonably close
                        if confidence >= 0.5 && distance <= params.old_text.len() / 3 {
                            return Ok(Some(MatchResult {
                                start: byte_start,
                                end: byte_end,
                                strategy: Strategy::LineNumberAnchored,
                                confidence,
                            }));
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

// =============================================================================
// Strategy 4: Prefix/Suffix Anchored
// =============================================================================

fn try_prefix_suffix_anchored(params: &EditParams<'_>) -> Result<Option<MatchResult>, String> {
    let old_lines: Vec<&str> = params.old_text.lines().collect();
    if old_lines.len() < 3 {
        // Need at least 3 lines: prefix, middle, suffix
        return Ok(None);
    }

    let prefix = old_lines[0].trim();
    let suffix = old_lines[old_lines.len() - 1].trim();

    // Skip if prefix/suffix are empty or too short
    if prefix.len() < 4 || suffix.len() < 4 {
        return Ok(None);
    }

    // Skip if prefix/suffix are too common (likely not unique)
    let prefix_count = params.content.matches(prefix).count();
    let suffix_count = params.content.matches(suffix).count();
    if prefix_count > 5 || suffix_count > 5 {
        return Ok(None);
    }

    let content_lines: Vec<&str> = params.content.lines().collect();

    // Find lines matching prefix and suffix
    let prefix_positions: Vec<usize> = content_lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim() == prefix)
        .map(|(i, _)| i)
        .collect();

    let suffix_positions: Vec<usize> = content_lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim() == suffix)
        .map(|(i, _)| i)
        .collect();

    // Find a (prefix_pos, suffix_pos) pair where the distance matches old_text's line count
    let target_distance = old_lines.len() - 1; // prefix to suffix line distance
    for &p_pos in &prefix_positions {
        for &s_pos in &suffix_positions {
            let line_distance = s_pos.saturating_sub(p_pos);
            // Allow some tolerance in line distance
            if line_distance >= target_distance.saturating_sub(2)
                && line_distance <= target_distance + 2
            {
                let byte_start = content_lines[..p_pos]
                    .iter()
                    .map(|l| l.len() + 1)
                    .sum::<usize>();
                let byte_end = content_lines[..s_pos + 1]
                    .iter()
                    .map(|l| l.len() + 1)
                    .sum::<usize>()
                    .saturating_sub(1);

                let actual_region: String = params.content[byte_start..byte_end].to_string();
                let distance = levenshtein(&actual_region, params.old_text);
                let max_len = actual_region.len().max(params.old_text.len());
                let confidence = if max_len == 0 {
                    1.0
                } else {
                    1.0 - (distance as f64 / max_len as f64)
                };

                if confidence >= 0.5 {
                    return Ok(Some(MatchResult {
                        start: byte_start,
                        end: byte_end,
                        strategy: Strategy::PrefixSuffixAnchored,
                        confidence,
                    }));
                }
            }
        }
    }

    Ok(None)
}

// =============================================================================
// Strategy 5: Fuzzy Match (Aho-Corasick + Levenshtein)
// =============================================================================

fn try_fuzzy(
    params: &EditParams<'_>,
    max_distance: usize,
) -> Result<Option<MatchResult>, String> {
    // For fuzzy matching, we use significant lines from old_text as Aho-Corasick
    // patterns to find candidate regions, then verify with Levenshtein.

    let old_lines: Vec<&str> = params.old_text.lines().collect();
    if old_lines.is_empty() {
        return Ok(None);
    }

    // Extract significant lines (non-empty, > 3 chars) as search patterns
    let patterns: Vec<&str> = old_lines
        .iter()
        .filter(|line| line.trim().len() > 3)
        .take(5) // limit to 5 patterns for performance
        .copied()
        .collect();

    if patterns.is_empty() {
        return Ok(None);
    }

    // Use Aho-Corasick to find candidate regions
    let ac = aho_corasick::AhoCorasick::builder()
        .ascii_case_insensitive(false)
        .build(&patterns)
        .map_err(|e| format!("Aho-Corasick build failed: {e}"))?;

    let mut best_match: Option<MatchResult> = None;
    let mut best_confidence = 0.0_f64;

    for mat in ac.find_iter(params.content) {
        let match_start = mat.start();
        // Extend the candidate region to encompass the full old_text length
        let estimated_end = (match_start + params.old_text.len()).min(params.content.len());
        let candidate = &params.content[match_start..estimated_end];

        let distance = levenshtein(candidate, params.old_text);
        if distance <= max_distance {
            let max_len = candidate.len().max(params.old_text.len());
            let confidence = if max_len == 0 {
                1.0
            } else {
                1.0 - (distance as f64 / max_len as f64)
            };

            if confidence > best_confidence {
                best_confidence = confidence;
                best_match = Some(MatchResult {
                    start: match_start,
                    end: estimated_end,
                    strategy: Strategy::Fuzzy,
                    confidence,
                });
            }
        }
    }

    Ok(best_match)
}

// =============================================================================
// Apply Match
// =============================================================================

fn apply_match(params: &EditParams<'_>, match_result: &MatchResult) -> EditCascadeResult {
    let new_content = if params.replace_all && match_result.strategy == Strategy::Exact {
        // Only Exact supports replace_all
        params.content.replace(params.old_text, params.new_text)
    } else {
        let mut result = String::with_capacity(
            params.content.len() - (match_result.end - match_result.start)
                + params.new_text.len(),
        );
        result.push_str(&params.content[..match_result.start]);
        result.push_str(params.new_text);
        result.push_str(&params.content[match_result.end..]);
        result
    };

    EditCascadeResult {
        new_content,
        strategy: match_result.strategy,
        confidence: match_result.confidence,
        replacements: 1,
    }
}

// =============================================================================
// Levenshtein Distance
// =============================================================================

/// Compute the Levenshtein edit distance between two strings.
/// Uses the Wagner-Fischer algorithm with O(min(m,n)) space optimization.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a_bytes: Vec<char> = a.chars().collect();
    let b_bytes: Vec<char> = b.chars().collect();

    let m = a_bytes.len();
    let n = b_bytes.len();

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    // Use two rows for space optimization
    let mut prev = vec![0usize; n + 1];
    let mut curr = vec![0usize; n + 1];

    // Initialize first row
    for j in 0..=n {
        prev[j] = j;
    }

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1)
                .min(curr[j - 1] + 1)
                .min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let old = "println!(\"hello\")";
        let new = "println!(\"world\")";

        let params = EditParams {
            content,
            old_text: old,
            new_text: new,
            replace_all: false,
        };

        let result = apply_edit_cascade(&params).unwrap();
        assert_eq!(result.strategy, Strategy::Exact);
        assert!((result.confidence - 1.0).abs() < f64::EPSILON);
        assert!(result.new_content.contains("world"));
        assert!(!result.new_content.contains("hello"));
    }

    #[test]
    fn test_exact_match_not_found() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let old = "printlne!(\"goodbye\")";
        let new = "println!(\"world\")";

        let params = EditParams {
            content,
            old_text: old,
            new_text: new,
            replace_all: false,
        };

        let result = apply_edit_cascade(&params);
        assert!(result.is_err());
    }

    #[test]
    fn test_whitespace_tolerant_tabs_vs_spaces() {
        let content = "fn main() {\n\tprintln!(\"hello\");\n}\n";
        let old = "fn main() {\n    println!(\"hello\");\n}";
        let new = "fn main() {\n    println!(\"world\");\n}";

        let params = EditParams {
            content,
            old_text: old,
            new_text: new,
            replace_all: false,
        };

        let result = apply_edit_cascade(&params).unwrap();
        assert_eq!(result.strategy, Strategy::WhitespaceTolerant);
        assert!(result.confidence > 0.5);
    }

    #[test]
    fn test_whitespace_tolerant_trailing_whitespace() {
        let content = "fn main() {    \n    println!(\"hello\");\n}\n";
        let old = "fn main() {\n    println!(\"hello\");\n}";
        let new = "fn main() {\n    println!(\"world\");\n}";

        let params = EditParams {
            content,
            old_text: old,
            new_text: new,
            replace_all: false,
        };

        let result = apply_edit_cascade(&params).unwrap();
        assert_eq!(result.strategy, Strategy::WhitespaceTolerant);
    }

    #[test]
    fn test_prefix_suffix_anchored() {
        let content = "fn main() {\n    let x = 42;\n    println!(\"{}\");\n}\n";
        let old = "fn main() {\n    let x = 99;\n    println!(\"{}\");\n}";
        let new = "fn main() {\n    let x = 42;\n    println!(\"{}\");\n}";

        let params = EditParams {
            content,
            old_text: old,
            new_text: new,
            replace_all: false,
        };

        let result = apply_edit_cascade(&params).unwrap();
        // The middle line differs, but prefix/suffix match
        assert!(result.strategy == Strategy::PrefixSuffixAnchored
            || result.strategy == Strategy::Exact);
    }

    #[test]
    fn test_fuzzy_match() {
        let content = "fn main() {\n    println!(\"hello world\");\n}\n";
        let old = "fn main() {\n    println!(\"hello wurld\");\n}";
        let new = "fn main() {\n    println!(\"hello world\");\n}";

        let params = EditParams {
            content,
            old_text: old,
            new_text: new,
            replace_all: false,
        };

        let result = apply_edit_cascade(&params).unwrap();
        // Should match via fuzzy or prefix/suffix
        assert!(result.confidence > 0.5);
    }

    #[test]
    fn test_levenshtein() {
        assert_eq!(levenshtein("", ""), 0);
        assert_eq!(levenshtein("abc", "abc"), 0);
        assert_eq!(levenshtein("abc", "abd"), 1);
        assert_eq!(levenshtein("abc", "adc"), 1);
        assert_eq!(levenshtein("abc", ""), 3);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn test_replace_all_exact() {
        let content = "foo bar foo baz foo";
        let old = "foo";
        let new = "qux";

        let params = EditParams {
            content,
            old_text: old,
            new_text: new,
            replace_all: true,
        };

        let result = apply_edit_cascade(&params).unwrap();
        assert_eq!(result.new_content, "qux bar qux baz qux");
    }

    #[test]
    fn test_empty_old_text_returns_error() {
        let content = "hello world";
        let params = EditParams {
            content,
            old_text: "",
            new_text: "new",
            replace_all: false,
        };

        let result = apply_edit_cascade(&params);
        assert!(result.is_err());
    }

    #[test]
    fn test_no_match_diagnostics() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let old = "completely unrelated content that does not exist";
        let new = "something else";

        let params = EditParams {
            content,
            old_text: old,
            new_text: new,
            replace_all: false,
        };

        let result = apply_edit_cascade(&params).unwrap_err();
        assert!(!result.diagnostics.is_empty());
        // First diagnostic should be Exact
        assert_eq!(result.diagnostics[0].strategy, Strategy::Exact);
    }

    #[test]
    fn test_cascade_config_disabled_strategies() {
        let config = CascadeConfig {
            enable_whitespace_tolerant: false,
            enable_line_number_anchored: false,
            enable_prefix_suffix_anchored: false,
            enable_fuzzy: false,
            min_confidence: 0.6,
            max_levenshtein_distance: 10,
        };

        let content = "fn main() {\n\tprintln!(\"hello\");\n}\n";
        let old = "fn main() {\n    println!(\"hello\");\n}";
        let new = "fn main() {\n    println!(\"world\");\n}";

        let params = EditParams {
            content,
            old_text: old,
            new_text: new,
            replace_all: false,
        };

        // With only Exact enabled, whitespace difference should fail
        let result = apply_edit_cascade_with_config(&params, &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_line_number_anchored() {
        let content = "use std::io;\n\nfn main() {\n    let x = 42;\n    println!(\"{}\");\n}\n";
        // LLM provides line-numbered content that doesn't exactly match
        let old = "3: fn main() {\n4:     let x = 99;\n5:     println!(\"{}\");\n6: }";
        let new = "fn main() {\n    let x = 42;\n    println!(\"{}\");\n}";

        let params = EditParams {
            content,
            old_text: old,
            new_text: new,
            replace_all: false,
        };

        let result = apply_edit_cascade(&params).unwrap();
        assert!(result.confidence > 0.4);
    }

    #[test]
    fn test_confidence_threshold() {
        let config = CascadeConfig {
            min_confidence: 0.99, // Very high threshold
            ..Default::default()
        };

        let content = "fn main() {\n\tprintln!(\"hello\");\n}\n";
        let old = "fn main() {\n    println!(\"hello\");\n}";
        let new = "fn main() {\n    println!(\"world\");\n}";

        let params = EditParams {
            content,
            old_text: old,
            new_text: new,
            replace_all: false,
        };

        // Whitespace tolerant should match but confidence < 0.99
        let result = apply_edit_cascade_with_config(&params, &config);
        // Should either match exact (unlikely) or fail
        if let Ok(r) = result {
            assert_eq!(r.strategy, Strategy::Exact);
        }
    }
}
