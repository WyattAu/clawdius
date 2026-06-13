//! Diagnostic analysis for `textDocument/publishDiagnostics`.
//!
//! Scans source text for common quality issues: TODO/FIXME markers
//! and `.unwrap()` usage that can panic at runtime.

use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, DiagnosticTag, NumberOrString, Position, Range,
};

const SOURCE: &str = "clawdius";

/// Analyze source text and produce a list of diagnostics.
///
/// Detects:
/// - `TODO` comments (hint, tagged deprecated)
/// - `FIXME` comments (warning)
/// - `.unwrap()` calls outside comments (warning)
#[must_use]
pub fn analyze_diagnostics(text: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for (line_num, line) in text.lines().enumerate() {
        check_todo(line, line_num, &mut diagnostics);
        check_fixme(line, line_num, &mut diagnostics);
        check_unwrap(line, line_num, &mut diagnostics);
    }

    diagnostics
}

/// Check for `TODO` markers.
#[allow(clippy::cast_possible_truncation)]
fn check_todo(line: &str, line_num: usize, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(pos) = line.find("TODO") {
        diagnostics.push(Diagnostic {
            range: Range {
                start: Position {
                    line: line_num as u32,
                    character: pos as u32,
                },
                end: Position {
                    line: line_num as u32,
                    character: (pos + 4) as u32,
                },
            },
            severity: Some(DiagnosticSeverity::HINT),
            code: None,
            source: Some(SOURCE.to_string()),
            message: "TODO found".to_string(),
            related_information: None,
            tags: Some(vec![DiagnosticTag::UNNECESSARY]),
            data: None,
            ..Default::default()
        });
    }
}

/// Check for `FIXME` markers.
#[allow(clippy::cast_possible_truncation)]
fn check_fixme(line: &str, line_num: usize, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(pos) = line.find("FIXME") {
        diagnostics.push(Diagnostic {
            range: Range {
                start: Position {
                    line: line_num as u32,
                    character: pos as u32,
                },
                end: Position {
                    line: line_num as u32,
                    character: (pos + 5) as u32,
                },
            },
            severity: Some(DiagnosticSeverity::WARNING),
            code: None,
            source: Some(SOURCE.to_string()),
            message: "FIXME found".to_string(),
            related_information: None,
            tags: None,
            data: None,
            ..Default::default()
        });
    }
}

/// Check for `.unwrap()` calls outside comments.
#[allow(clippy::cast_possible_truncation)]
fn check_unwrap(line: &str, line_num: usize, diagnostics: &mut Vec<Diagnostic>) {
    let trimmed_start = line.trim_start();
    if trimmed_start.starts_with("//") {
        return;
    }
    if let Some(pos) = line.find(".unwrap()") {
        diagnostics.push(Diagnostic {
            range: Range {
                start: Position {
                    line: line_num as u32,
                    character: pos as u32,
                },
                end: Position {
                    line: line_num as u32,
                    character: line.len() as u32,
                },
            },
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String("unwrap".to_string())),
            source: Some(SOURCE.to_string()),
            message: "Use of .unwrap() can panic".to_string(),
            related_information: None,
            tags: None,
            data: None,
            ..Default::default()
        });
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_todo_detection() {
        let code = "fn main() {\n    // TODO: implement this\n}\n";
        let diags = analyze_diagnostics(code);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::HINT));
        assert_eq!(diags[0].message, "TODO found");
    }

    #[test]
    fn test_fixme_detection() {
        let code = "// FIXME: broken\nfn main() {}\n";
        let diags = analyze_diagnostics(code);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(diags[0].message, "FIXME found");
    }

    #[test]
    fn test_unwrap_detection() {
        let code = "fn main() {\n    let x = foo.unwrap();\n}\n";
        let diags = analyze_diagnostics(code);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::WARNING));
        assert!(diags[0].message.contains("unwrap"));
    }

    #[test]
    fn test_unwrap_ignored_in_comment() {
        let code = "// let x = foo.unwrap();\nfn main() {}\n";
        let diags = analyze_diagnostics(code);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_multiple_issues() {
        let code =
            "fn main() {\n    // TODO: fix\n    let x = foo.unwrap();\n    // FIXME: urgent\n}\n";
        let diags = analyze_diagnostics(code);
        assert_eq!(diags.len(), 3);
    }

    #[test]
    fn test_clean_code() {
        let code = "fn main() {\n    println!(\"hello\");\n}\n";
        let diags = analyze_diagnostics(code);
        assert!(diags.is_empty());
    }
}
