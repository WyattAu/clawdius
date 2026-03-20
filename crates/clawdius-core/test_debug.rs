fn test_framework_parsing() {
    let content = r#"---
project: my-project
language: rust
framework: axum
---
# Instructions
Use idiomatic Rust.
"#;

    let mut memory = ProjectMemory::new("/tmp/test");
    memory.extract_metadata(content);

    println!("DEBUG project_name: {:?}", memory.metadata().project_name);
    println!("DEBUG primary_language: {:?}", memory.metadata().primary_language);
    println!("DEBUG framework: {:?}", memory.metadata().framework);

    // Check frontmatter parsing
    let fm_start = content.find("---");
    let fm_end = content[3..].find("---");
    println!("DEBUG fm_start: {:?}", fm_start);
    println!("DEBUG fm_end: {:?}", fm_end);
    
    let fm_content = &content[3..(fm_end.unwrap() + 3)];
    println!("DEBUG frontmatter content:");
    for line in fm_content.lines() {
        if let Some((k, v)) = line.split_once(':') {
            println!("  '{}' => '{}'", k.trim(), v.trim());
        }
    }
}
