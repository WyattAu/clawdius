use clawdius_core::diff::{DiffLine, DiffRenderer, DiffTheme, FileDiff};
use std::path::PathBuf;

#[test]
fn test_diff_compute_render_terminal() {
    let old = r#"fn main() {
    println!("Hello");
}
"#;
    let new = r#"fn main() {
    println!("Hello, World!");
    println!("Welcome!");
}
"#;

    let diff = FileDiff::compute(PathBuf::from("src/main.rs"), Some(old), new);

    assert!(diff.old_content.is_some());
    assert!(!diff.hunks.is_empty());

    let stats = diff.stats();
    assert!(stats.additions > 0);
    assert!(stats.deletions > 0);
    assert_eq!(stats.files_changed, 1);

    let renderer = DiffRenderer::default();
    let terminal = renderer.render_terminal(&diff);

    assert!(terminal.contains("src/main.rs"));
    assert!(terminal.contains('\x1b'));

    let has_ansi_green = terminal.contains("\x1b[38;2;");
    assert!(has_ansi_green);
}

#[test]
fn test_diff_compute_render_html() {
    let old = r"<!DOCTYPE html>
<html>
<body>
    <h1>Old Title</h1>
</body>
</html>";

    let new = r"<!DOCTYPE html>
<html>
<body>
    <h1>New Title</h1>
    <p>Added paragraph</p>
</body>
</html>";

    let diff = FileDiff::compute(PathBuf::from("index.html"), Some(old), new);

    let renderer = DiffRenderer::default();
    let html = renderer.render_html(&diff);

    assert!(html.contains("<div class=\"diff-container\">"));
    assert!(html.contains("<div class=\"diff-header\">index.html</div>"));
    assert!(html.contains("<div class=\"diff-hunk\">"));
    assert!(html.contains("<div class=\"diff-line added\">"));
    assert!(html.contains("<div class=\"diff-line removed\">"));

    assert!(html.contains("&lt;h1&gt;"));
    assert!(html.contains("&lt;/h1&gt;"));
}

#[test]
fn test_diff_new_file() {
    let new = "This is a brand new file\nWith multiple lines\n";

    let diff = FileDiff::compute(PathBuf::from("new_file.txt"), None, new);

    assert!(diff.old_content.is_none());
    assert_eq!(diff.new_content, new);

    let stats = diff.stats();
    assert!(stats.additions > 0);
    assert_eq!(stats.deletions, 0);
}

#[test]
fn test_diff_empty_changes() {
    let content = "Line 1\nLine 2\nLine 3\n";

    let diff = FileDiff::compute(PathBuf::from("unchanged.txt"), Some(content), content);

    let stats = diff.stats();
    assert_eq!(stats.additions, 0);
    assert_eq!(stats.deletions, 0);
}

#[test]
fn test_diff_unified_format() {
    let old = "line1\nline2\nline3\n";
    let new = "line1\nmodified\nline3\n";

    let diff = FileDiff::compute(PathBuf::from("test.txt"), Some(old), new);
    let unified = diff.to_unified();

    assert!(unified.contains("--- test.txt"));
    assert!(unified.contains("+++ test.txt"));
    assert!(unified.contains("@@"));
}

#[test]
fn test_diff_theme_dark() {
    let theme = DiffTheme::dark();

    assert_ne!(theme.addition_color.r, theme.deletion_color.r);

    let ansi = theme.addition_color.to_ansi();
    assert!(ansi.starts_with("\x1b[38;2;"));
}

#[test]
fn test_diff_theme_light() {
    let theme = DiffTheme::light();

    let hex = theme.addition_color.to_hex();
    assert!(hex.starts_with('#'));
    assert_eq!(hex.len(), 7);
}

#[test]
fn test_diff_renderer_css() {
    let renderer = DiffRenderer::default();
    let css = renderer.get_css();

    assert!(css.contains(".diff-container"));
    assert!(css.contains(".diff-header"));
    assert!(css.contains(".diff-line"));
    assert!(css.contains(".added"));
    assert!(css.contains(".removed"));
}

#[test]
fn test_diff_multiple_hunks() {
    let old = r"header
section1
middle
section2
footer";

    let new = r"header
modified_section1
middle
modified_section2
footer";

    let diff = FileDiff::compute(PathBuf::from("multi.txt"), Some(old), new);

    assert!(!diff.hunks.is_empty());

    let stats = diff.stats();
    assert!(stats.additions >= 2);
    assert!(stats.deletions >= 2);
}

#[test]
fn test_diff_line_types() {
    let old = "line1\nline2\nline3\nline4\n";
    let new = "line1\nmodified\nline3\nline4\n";

    let diff = FileDiff::compute(PathBuf::from("types.txt"), Some(old), new);

    let mut has_added = false;
    let mut has_removed = false;

    for hunk in &diff.hunks {
        for line in &hunk.lines {
            match line {
                DiffLine::Context(_) => {}
                DiffLine::Added(_) => has_added = true,
                DiffLine::Removed(_) => has_removed = true,
            }
        }
    }

    assert!(has_added, "Should have added lines");
    assert!(has_removed, "Should have removed lines");
}

#[test]
fn test_diff_large_file() {
    let old: String = (0..1000).map(|i| format!("Line {i}\n")).collect();
    let new: String = (0..1000)
        .map(|i| {
            if i == 500 {
                format!("Modified Line {i}\n")
            } else {
                format!("Line {i}\n")
            }
        })
        .collect();

    let diff = FileDiff::compute(PathBuf::from("large.txt"), Some(&old), &new);

    assert!(!diff.hunks.is_empty());

    let stats = diff.stats();
    assert_eq!(stats.additions, 1);
    assert_eq!(stats.deletions, 1);
}
