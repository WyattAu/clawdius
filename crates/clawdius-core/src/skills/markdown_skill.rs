use super::{Skill, SkillArgument, SkillContext, SkillError, SkillMeta, SkillResult};
use crate::llm::providers::LlmClient;
use crate::llm::{ChatMessage, ChatRole};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct MarkdownSkill {
    meta: SkillMeta,
    instructions: String,
    constraints: String,
    source_file: PathBuf,
}

impl MarkdownSkill {
    pub fn from_file(path: &Path) -> super::Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            SkillError::ParseError(format!("Failed to read {}: {e}", path.display()))
        })?;

        let (frontmatter, body) = extract_frontmatter(&content)?;

        let name = extract_scalar(&frontmatter, "name")
            .ok_or_else(|| SkillError::ParseError("Missing 'name' in frontmatter".into()))?;
        let description = extract_scalar(&frontmatter, "description")
            .ok_or_else(|| SkillError::ParseError("Missing 'description' in frontmatter".into()))?;
        let version = extract_scalar(&frontmatter, "version").unwrap_or("1.0.0".into());
        let tags = extract_string_list(&frontmatter, "tags").unwrap_or_default();
        let examples = extract_string_list(&frontmatter, "examples").unwrap_or_default();
        let arguments = extract_arguments(&frontmatter);

        let (instructions, constraints) = extract_constraints(&body);

        Ok(Self {
            meta: SkillMeta {
                name,
                description,
                version,
                author: None,
                tags,
                arguments,
                examples,
            },
            instructions: instructions.trim().to_string(),
            constraints: constraints.trim().to_string(),
            source_file: path.to_path_buf(),
        })
    }

    pub fn source_file(&self) -> &Path {
        &self.source_file
    }

    fn build_system_prompt(&self) -> String {
        let mut prompt = self.instructions.clone();
        if !self.constraints.is_empty() {
            prompt.push_str("\n\n## Constraints\n\n");
            prompt.push_str(&self.constraints);
        }
        prompt
    }
}

fn extract_frontmatter(content: &str) -> super::Result<(Vec<FrontmatterLine>, String)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Err(SkillError::ParseError(
            "Markdown skill must start with YAML frontmatter (---)".into(),
        ));
    }

    let after_first_delim = &trimmed[3..];
    let end = after_first_delim
        .find("\n---")
        .or_else(|| {
            if after_first_delim.starts_with("---") {
                Some(0)
            } else {
                None
            }
        })
        .ok_or_else(|| SkillError::ParseError("Missing closing --- for frontmatter".into()))?;

    let raw_fm = after_first_delim[..end].trim();
    let body = after_first_delim[end + 3..].trim().to_string();

    let lines = parse_frontmatter_lines(raw_fm);
    Ok((lines, body))
}

#[derive(Debug)]
enum FrontmatterLine {
    Scalar {
        key: String,
        value: String,
    },
    ListItem {
        index: usize,
        value: String,
    },
    NestedListItem {
        index: usize,
        item_index: usize,
        key: String,
        value: String,
    },
}

fn parse_frontmatter_lines(raw: &str) -> Vec<FrontmatterLine> {
    let mut lines = Vec::new();
    let mut current_list_index: Option<usize> = None;
    let mut current_item_index: usize = 0;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Track indentation depth to distinguish nested list items
        let indent = line.len() - line.trim_start().len();

        if let Some(rest) = trimmed.strip_prefix("- ") {
            if let Some((key, value)) = rest.split_once(':') {
                // Only treat as key:value if the key looks like a YAML key (no spaces, not quoted)
                let key_trimmed = key.trim();
                if !key_trimmed.contains(' ')
                    && !key_trimmed.starts_with('"')
                    && !key_trimmed.starts_with('\'')
                {
                    let key = key_trimmed.to_string();
                    let value = value
                        .trim()
                        .trim_start_matches('"')
                        .trim_end_matches('"')
                        .to_string();

                    if indent >= 2 && current_list_index.is_some() {
                        lines.push(FrontmatterLine::NestedListItem {
                            index: current_list_index.unwrap(),
                            item_index: current_item_index,
                            key,
                            value,
                        });
                    } else {
                        current_list_index = Some(lines.len());
                        current_item_index = 0;
                        lines.push(FrontmatterLine::Scalar { key, value });
                    }
                } else {
                    // It's a plain list item that happens to contain a colon
                    let value = rest.trim().trim_start_matches('[').trim_end_matches(']');
                    let value = value
                        .trim()
                        .trim_start_matches('"')
                        .trim_end_matches('"')
                        .to_string();
                    current_list_index = Some(lines.len());
                    current_item_index += 1;
                    lines.push(FrontmatterLine::ListItem {
                        index: lines.len(),
                        value,
                    });
                }
            } else {
                // Plain list item (e.g., `- "item"`, `- tag`)
                let value = rest.trim().trim_start_matches('[').trim_end_matches(']');
                let value = value
                    .trim()
                    .trim_start_matches('"')
                    .trim_end_matches('"')
                    .to_string();
                current_list_index = Some(lines.len());
                current_item_index += 1;
                lines.push(FrontmatterLine::ListItem {
                    index: lines.len(),
                    value,
                });
            }
        } else if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().to_string();
            let value = value
                .trim()
                .trim_start_matches('"')
                .trim_end_matches('"')
                .to_string();
            // Only reset list context if this is a top-level key (not indented)
            if indent < 2 {
                current_list_index = None;
            }
            lines.push(FrontmatterLine::Scalar { key, value });
        }
    }

    lines
}

fn extract_scalar(lines: &[FrontmatterLine], key: &str) -> Option<String> {
    for line in lines {
        if let FrontmatterLine::Scalar { key: k, value: v } = line {
            if k == key {
                return Some(v.clone());
            }
        }
    }
    None
}

fn extract_string_list(lines: &[FrontmatterLine], key: &str) -> Option<Vec<String>> {
    let mut in_list = false;
    let mut result = Vec::new();

    for line in lines {
        match line {
            FrontmatterLine::Scalar { key: k, value: v } => {
                if k == key {
                    if v.starts_with('[') && v.ends_with(']') {
                        let inner = &v[1..v.len() - 1];
                        let items: Vec<String> = inner
                            .split(',')
                            .map(|s| s.trim().trim_matches('"').to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        return Some(items);
                    }
                    in_list = true;
                } else if in_list {
                    break;
                }
            },
            FrontmatterLine::ListItem { value, .. } => {
                if in_list {
                    let mut v = value.clone();
                    if v.starts_with('[') && v.ends_with(']') {
                        v = v[1..v.len() - 1].to_string();
                    }
                    for item in v.split(',') {
                        let item = item.trim().trim_matches('"').to_string();
                        if !item.is_empty() {
                            result.push(item);
                        }
                    }
                }
            },
            _ => {},
        }
    }

    if in_list || !result.is_empty() {
        Some(result)
    } else {
        None
    }
}

fn extract_arguments(lines: &[FrontmatterLine]) -> Vec<SkillArgument> {
    let mut in_arguments = false;
    let mut args = Vec::new();
    let mut current_arg: Option<ArgBuilder> = None;

    // Known top-level keys that end the arguments section
    let top_level_keys = [
        "name",
        "description",
        "version",
        "author",
        "tags",
        "examples",
    ];

    for line in lines {
        match line {
            FrontmatterLine::Scalar { key: k, value: v } => {
                if k == "arguments" {
                    in_arguments = true;
                    continue;
                }
                if in_arguments {
                    // Check if we've hit a new top-level key
                    if top_level_keys.contains(&k.as_str()) && current_arg.is_none() && v.is_empty()
                    {
                        in_arguments = false;
                        continue;
                    }
                    // Flush previous arg when we see a new "name" key
                    if k == "name" {
                        if let Some(arg) = current_arg.take() {
                            args.push(arg.build());
                        }
                        current_arg = Some(ArgBuilder::new(v.clone()));
                    } else if let Some(arg) = current_arg.as_mut() {
                        match k.as_str() {
                            "description" => arg.description = v.clone(),
                            "required" => arg.required = v == "true",
                            "default" => arg.default = Some(v.clone()),
                            "options" => {
                                arg.options =
                                    Some(v.split(',').map(|s| s.trim().to_string()).collect())
                            },
                            _ => {},
                        }
                    }
                }
            },
            FrontmatterLine::NestedListItem {
                key: k, value: v, ..
            } => {
                if in_arguments {
                    if k == "name" {
                        if let Some(arg) = current_arg.take() {
                            args.push(arg.build());
                        }
                        current_arg = Some(ArgBuilder::new(v.clone()));
                    } else if let Some(arg) = current_arg.as_mut() {
                        match k.as_str() {
                            "description" => arg.description = v.clone(),
                            "required" => arg.required = v == "true",
                            "default" => arg.default = Some(v.clone()),
                            "options" => {
                                arg.options =
                                    Some(v.split(',').map(|s| s.trim().to_string()).collect())
                            },
                            _ => {},
                        }
                    }
                }
            },
            FrontmatterLine::ListItem { .. } => {
                // List items under arguments are ignored (they are the argument list itself)
            },
        }
    }

    if let Some(arg) = current_arg.take() {
        args.push(arg.build());
    }

    args
}

struct ArgBuilder {
    name: String,
    description: String,
    required: bool,
    default: Option<String>,
    options: Option<Vec<String>>,
}

impl ArgBuilder {
    fn new(name: String) -> Self {
        Self {
            name,
            description: String::new(),
            required: false,
            default: None,
            options: None,
        }
    }

    fn build(self) -> SkillArgument {
        SkillArgument {
            name: self.name,
            description: self.description,
            required: self.required,
            default: self.default,
            options: self.options,
        }
    }
}

fn extract_constraints(body: &str) -> (String, String) {
    let mut instructions = String::new();
    let mut constraints = String::new();
    let mut in_constraints = false;

    for line in body.lines() {
        if line.trim() == "## Constraints" {
            in_constraints = true;
            continue;
        }
        // Stop constraints section at the next ## heading
        if in_constraints && line.starts_with("## ") {
            instructions.push_str(line);
            instructions.push('\n');
            in_constraints = false;
            continue;
        }
        if in_constraints {
            constraints.push_str(line);
            constraints.push('\n');
        } else {
            instructions.push_str(line);
            instructions.push('\n');
        }
    }

    (instructions, constraints)
}

#[async_trait::async_trait]
impl Skill for MarkdownSkill {
    fn meta(&self) -> &SkillMeta {
        &self.meta
    }

    async fn execute(&self, context: SkillContext) -> super::Result<SkillResult> {
        if let Some(ref llm) = context.llm {
            let system_prompt = self.build_system_prompt();

            let mut user_parts = Vec::new();

            if let Some(ref file) = context.current_file {
                user_parts.push(format!("Current file: {}", file.display()));
            }
            if let Some(ref sel) = context.selection {
                user_parts.push(format!("Selected code:\n{sel}"));
            }
            if !context.arguments.is_empty() {
                let args: Vec<String> = context
                    .arguments
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect();
                user_parts.push(format!("Arguments: {}", args.join(", ")));
            }

            let user_prompt = if user_parts.is_empty() {
                "Execute the skill workflow.".to_string()
            } else {
                user_parts.join("\n\n")
            };

            let messages = vec![
                ChatMessage {
                    role: ChatRole::System,
                    content: system_prompt,
                },
                ChatMessage {
                    role: ChatRole::User,
                    content: user_prompt,
                },
            ];

            match llm.chat(messages).await {
                Ok(response) => return Ok(SkillResult::success(response)),
                Err(e) => {
                    tracing::warn!(
                        "LLM execution failed for markdown skill '{}': {e}",
                        self.meta.name
                    );
                },
            }
        }

        let mut output = format!("Skill: {}\n\n{}\n", self.meta.name, self.instructions);
        if !self.constraints.is_empty() {
            output.push_str("\nConstraints:\n");
            output.push_str(&self.constraints);
        }
        Ok(SkillResult::success(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_temp_skill(content: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        write!(file, "{content}").unwrap();
        file
    }

    #[test]
    fn test_parse_valid_markdown_skill() {
        let content = r#"---
name: ship
description: Stage, commit, and push changes
version: 1.0.0
tags: [git, deploy]
arguments:
  - name: message
    description: Optional commit message override
    required: false
examples:
  - "/ship"
  - "/ship message=fix: resolve login bug"
---

# Ship Skill

Stage all changes and push them.

## Instructions

1. Run `git status`
2. Run `git add -A`
3. Run `git commit -m "msg"`
4. Run `git push`

## Constraints

- Never push to protected branches
- Always show what will be committed
"#;
        let file = make_temp_skill(content);
        let skill = MarkdownSkill::from_file(file.path()).unwrap();

        assert_eq!(skill.meta().name, "ship");
        assert_eq!(skill.meta().description, "Stage, commit, and push changes");
        assert_eq!(skill.meta().version, "1.0.0");
        assert_eq!(skill.meta().tags, vec!["git", "deploy"]);
        assert_eq!(skill.meta().examples.len(), 2);
        assert_eq!(skill.meta().arguments.len(), 1);
        assert_eq!(skill.meta().arguments[0].name, "message");
        assert!(!skill.meta().arguments[0].required);
        assert!(!skill.instructions.is_empty());
        assert!(skill.constraints.contains("protected branches"));
    }

    #[test]
    fn test_parse_minimal_skill() {
        let content = "---\nname: minimal\ndescription: A minimal skill\n---\n\n# Minimal\n\nSome instructions.\n";
        let file = make_temp_skill(content);
        let skill = MarkdownSkill::from_file(file.path()).unwrap();

        assert_eq!(skill.meta().name, "minimal");
        assert_eq!(skill.meta().description, "A minimal skill");
        assert_eq!(skill.meta().version, "1.0.0");
        assert!(skill.meta().tags.is_empty());
        assert!(skill.meta().arguments.is_empty());
        assert!(skill.constraints.is_empty());
    }

    #[test]
    fn test_parse_missing_frontmatter_delimiter() {
        let content = "name: test\ndescription: broken\n\n# No frontmatter\n";
        let file = make_temp_skill(content);
        let result = MarkdownSkill::from_file(file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_closing_delimiter() {
        let content = "---\nname: test\ndescription: no close\n\n# Body\n";
        let file = make_temp_skill(content);
        let result = MarkdownSkill::from_file(file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_name() {
        let content = "---\ndescription: no name\n---\n\n# No name\n";
        let file = make_temp_skill(content);
        let result = MarkdownSkill::from_file(file.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("name"));
    }

    #[test]
    fn test_parse_missing_description() {
        let content = "---\nname: nodesc\n---\n\n# No desc\n";
        let file = make_temp_skill(content);
        let result = MarkdownSkill::from_file(file.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("description"));
    }

    #[test]
    fn test_frontmatter_inline_list() {
        let content =
            "---\nname: inline\ndescription: Skill with inline list\n---\n\n# Inline list\n\n## Constraints\n- constraint one\n";
        let file = make_temp_skill(content);
        let skill = MarkdownSkill::from_file(file.path()).unwrap();

        assert_eq!(skill.meta().name, "inline");
        assert!(skill.constraints.contains("constraint one"));
    }

    #[test]
    fn test_extract_constraints_section() {
        let body = "# Title\n\nSome intro text.\n\n## Instructions\n1. Do thing\n\n## Constraints\n- Never do X\n- Always do Y\n\n## Notes\nSome notes.\n";

        let (instructions, constraints) = extract_constraints(body);
        assert!(instructions.contains("# Title"));
        assert!(instructions.contains("Some intro text"));
        assert!(instructions.contains("## Instructions"));
        assert!(instructions.contains("Do thing"));
        assert!(!instructions.contains("## Constraints"));
        assert!(constraints.contains("Never do X"));
        assert!(constraints.contains("Always do Y"));
        assert!(!constraints.contains("## Notes"));
    }

    #[test]
    fn test_extract_constraints_none() {
        let body = "# Title\n\nNo constraints here.\n";
        let (instructions, constraints) = extract_constraints(body);
        assert!(instructions.contains("# Title"));
        assert!(constraints.is_empty());
    }

    #[tokio::test]
    async fn test_markdown_skill_execute_without_llm() {
        let content = "---\nname: test-skill\ndescription: A test skill\n---\n\n# Test\n\nDo the thing.\n\n## Constraints\n- Be careful\n";
        let file = make_temp_skill(content);
        let skill = MarkdownSkill::from_file(file.path()).unwrap();

        let ctx = SkillContext::new(PathBuf::from("/project"));
        let result = skill.execute(ctx).await.unwrap();

        assert!(result.success);
        assert!(result.output.contains("test-skill"));
        assert!(result.output.contains("Do the thing"));
        assert!(result.output.contains("Be careful"));
    }

    #[tokio::test]
    async fn test_load_skills_from_dir() {
        let dir = tempfile::tempdir().unwrap();

        let ship_content =
            "---\nname: ship\ndescription: Ship changes\n---\n\n# Ship\n\nPush code.\n";
        let qa_content =
            "---\nname: qa\ndescription: Run QA checks\n---\n\n# QA\n\nCheck quality.\n";

        std::fs::write(dir.path().join("ship.md"), ship_content).unwrap();
        std::fs::write(dir.path().join("qa.md"), qa_content).unwrap();
        std::fs::write(dir.path().join("readme.txt"), "not a skill").unwrap();

        let registry = super::SkillRegistry::new();
        let loaded = registry.load_skills_from_dir(dir.path()).await.unwrap();

        assert_eq!(loaded.len(), 2);
        assert!(loaded.contains(&"ship".to_string()));
        assert!(loaded.contains(&"qa".to_string()));

        let list = registry.list().await;
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_markdown_skill_registry_integration() {
        let dir = tempfile::tempdir().unwrap();

        let content = "---\nname: myskill\ndescription: My custom skill\nversion: 2.0.0\ntags: [custom]\nexamples:\n  - \"/myskill\"\n---\n\n# My Skill\n\nCustom instructions.\n\n## Constraints\n- Must be custom\n";
        std::fs::write(dir.path().join("myskill.md"), content).unwrap();

        let registry = super::SkillRegistry::new();
        registry.load_skills_from_dir(dir.path()).await.unwrap();

        let skill = registry.find("myskill").await;
        assert!(skill.is_some());

        let skill = skill.unwrap();
        let meta = skill.meta();
        assert_eq!(meta.name, "myskill");
        assert_eq!(meta.version, "2.0.0");
        assert_eq!(meta.tags, vec!["custom"]);

        let ctx = SkillContext::new(PathBuf::from("/project"));
        let result = registry.execute("myskill", ctx).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Custom instructions"));
    }

    #[tokio::test]
    async fn test_load_from_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let registry = super::SkillRegistry::new();
        let loaded = registry.load_skills_from_dir(dir.path()).await.unwrap();
        assert!(loaded.is_empty());
    }

    #[tokio::test]
    async fn test_load_from_nonexistent_dir() {
        let registry = super::SkillRegistry::new();
        let result = registry
            .load_skills_from_dir(Path::new("/nonexistent/path/skills"))
            .await;
        // Nonexistent dirs should return Ok with empty list (graceful degradation)
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
