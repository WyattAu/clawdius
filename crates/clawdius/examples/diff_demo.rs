use clawdius_core::diff::{DiffPreview, DiffRenderer, FileChange, FileDiff};
use std::path::PathBuf;

fn main() {
    let old_code = r#"fn main() {
    println!("Hello");
}
"#;

    let new_code = r#"fn main() {
    println!("Hello, World!");
    println!("Welcome!");
}
"#;

    let diff = FileDiff::compute(PathBuf::from("src/main.rs"), Some(old_code), new_code);

    println!("=== Unified Diff ===");
    println!("{}", diff.to_unified());

    println!("\n=== Statistics ===");
    let stats = diff.stats();
    println!("Additions: {}", stats.additions);
    println!("Deletions: {}", stats.deletions);
    println!("Files changed: {}", stats.files_changed);

    println!("\n=== Terminal Rendering ===");
    let renderer = DiffRenderer::default();
    println!("{}", renderer.render_terminal(&diff));

    println!("\n=== HTML Rendering ===");
    println!("{}", renderer.render_html(&diff));

    println!("\n=== Multiple File Preview ===");
    let changes = vec![
        FileChange {
            path: PathBuf::from("src/main.rs"),
            old_content: Some(old_code.to_string()),
            new_content: new_code.to_string(),
        },
        FileChange {
            path: PathBuf::from("src/lib.rs"),
            old_content: None,
            new_content: "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n".to_string(),
        },
    ];

    let preview = DiffPreview::from_changes(&changes);
    println!("Summary: {}", preview.summary);
    println!("\nMarkdown:\n{}", preview.to_markdown());
}
