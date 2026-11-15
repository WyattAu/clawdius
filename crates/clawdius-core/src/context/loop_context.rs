//! Loop context: repo conventions and skills for tool loops.
//!
//! The interactive TUI and opencode both hand their models the repo's
//! `AGENTS.md` and a set of skills. Tool loops (auto, pipeline,
//! maintenance) used to get neither: a maintenance cycle was told "write
//! tests for this file" with no conventions, no standards, and no
//! repo-specific rules — so it guessed at both.
//!
//! This module assembles that context synchronously at loop start. It
//! does not go through `SkillRegistry`: the registry holds executable
//! skills (async, argument-driven), while a loop needs the body text
//! inlined into its system prompt. It also cannot read a skill from
//! outside the workspace on demand — the loop's `read_file` tool rejects
//! paths that escape the workspace root — so matched bodies are inlined
//! rather than advertised as files.
//!
//! Selection is deterministic: score skill name/description tokens
//! against the goal's tokens, take the best few. No LLM, no randomness.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Cap on the inlined project instructions (`AGENTS.md` / `CLAUDE.md`).
pub const MAX_INSTRUCTIONS_CHARS: usize = 8_000;

/// Cap on each inlined skill body.
pub const MAX_SKILL_BODY_CHARS: usize = 3_000;

/// Cap on the whole assembled block.
pub const MAX_CONTEXT_CHARS: usize = 16_000;

/// Most skills inlined for one task.
pub const MAX_SKILLS: usize = 2;

/// Minimum token-overlap score before a skill is considered relevant.
/// One token is usually a coincidence ("test" appears in half the
/// catalog); two is a signal.
const MIN_SCORE: usize = 2;

/// Tokens too common to say anything about relevance.
const STOPWORDS: &[&str] = &[
    "that", "this", "with", "from", "into", "must", "each", "when", "then", "will", "have", "been",
    "them", "they", "there", "their", "also", "only", "just", "than", "over", "under", "after",
    "before", "without", "using", "used", "make", "made", "does", "doesn", "should", "could",
    "would", "which", "where", "what", "your", "yours",
];

/// Assembled loop context, ready to append to a system prompt.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoopContext {
    /// Prompt block. Empty when nothing was found.
    pub block: String,
    /// Characters of project instructions inlined (0 = none found).
    pub instructions_chars: usize,
    /// Skill names inlined, best match first.
    pub skills: Vec<String>,
}

impl LoopContext {
    /// Whether the block carries anything at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.block.is_empty()
    }

    /// One-line description for logs (`AGENTS.md 2.1k chars, skills: …`).
    #[must_use]
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if self.instructions_chars > 0 {
            parts.push(format!("AGENTS.md {} chars", self.instructions_chars));
        }
        if !self.skills.is_empty() {
            parts.push(format!("skills: {}", self.skills.join(", ")));
        }
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// A parsed `SKILL.md`: enough to judge relevance and inline the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDoc {
    /// Frontmatter `name`.
    pub name: String,
    /// Frontmatter `description`.
    pub description: String,
    /// Markdown body after the frontmatter.
    pub body: String,
}

/// Build context for `goal` from the repo and the user's skill roots.
#[must_use]
pub fn build(repo_root: &Path, goal: &str) -> LoopContext {
    build_with_roots(repo_root, goal, &skill_roots(repo_root))
}

/// Build context from explicit skill roots (tests inject these so results
/// never depend on what is installed on the host).
#[must_use]
pub fn build_with_roots(repo_root: &Path, goal: &str, roots: &[PathBuf]) -> LoopContext {
    let instructions = project_instructions(repo_root);
    let goal_tokens = tokens(goal);

    let mut docs = Vec::new();
    for root in roots {
        docs.extend(discover(root));
    }
    // First root wins on a name collision: repo skills override global.
    let mut seen = HashSet::new();
    docs.retain(|d| seen.insert(d.name.clone()));

    let mut scored: Vec<(usize, SkillDoc)> = docs
        .into_iter()
        .map(|d| (score(&d, &goal_tokens), d))
        .filter(|(s, _)| *s >= MIN_SCORE)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
    scored.truncate(MAX_SKILLS);

    let mut block = String::new();
    if let Some((file, body)) = &instructions {
        block.push_str("## Project instructions (");
        block.push_str(file);
        block.push_str(")\n");
        block.push_str(&cap_chars(body, MAX_INSTRUCTIONS_CHARS));
        block.push_str("\n\n");
    }
    if !scored.is_empty() {
        block.push_str("## Skills\nApply the guidance below when it fits the task.\n\n");
        for (_, doc) in &scored {
            block.push_str("### ");
            block.push_str(&doc.name);
            block.push('\n');
            block.push_str(&cap_chars(doc.body.trim(), MAX_SKILL_BODY_CHARS));
            block.push_str("\n\n");
        }
    }

    let block = if block.trim().is_empty() {
        String::new()
    } else {
        cap_chars(block.trim_end(), MAX_CONTEXT_CHARS)
    };

    LoopContext {
        block,
        instructions_chars: instructions
            .as_ref()
            .map_or(0, |(_, body)| body.chars().count()),
        skills: scored.into_iter().map(|(_, d)| d.name).collect(),
    }
}

/// Skill roots searched for a loop, in priority order.
#[must_use]
pub fn skill_roots(repo_root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![
        repo_root.join(".opencode/skills"),
        repo_root.join(".claude/skills"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        roots.push(home.join(".agents/skills"));
        roots.push(home.join(".config/opencode/skills"));
        roots.push(home.join(".clawdius/skills"));
    }
    roots
}

/// Repo conventions: `AGENTS.md`, else `CLAUDE.md`, else nothing.
fn project_instructions(repo_root: &Path) -> Option<(String, String)> {
    for name in ["AGENTS.md", "CLAUDE.md"] {
        let path = repo_root.join(name);
        if let Ok(body) = std::fs::read_to_string(&path) {
            if !body.trim().is_empty() {
                return Some((name.to_string(), body));
            }
        }
    }
    None
}

/// Parse every `SKILL.md` (or flat `*.md`) directly under `root`.
fn discover(root: &Path) -> Vec<SkillDoc> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut docs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let candidate = if path.is_dir() {
            path.join("SKILL.md")
        } else if path.extension().is_some_and(|e| e == "md") {
            path
        } else {
            continue;
        };
        if let Ok(raw) = std::fs::read_to_string(&candidate) {
            if let Some(doc) = parse_skill(&raw) {
                docs.push(doc);
            }
        }
    }
    docs
}

/// Parse `SKILL.md` frontmatter plus body.
#[must_use]
pub fn parse_skill(raw: &str) -> Option<SkillDoc> {
    let trimmed = raw.trim_start();
    let rest = trimmed.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let frontmatter = &rest[..end];
    let body = rest[end + 4..].trim_start().to_string();

    let mut name = None;
    let mut description = None;
    let mut lines = frontmatter.lines().peekable();
    while let Some(line) = lines.next() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key != "name" && key != "description" {
            continue;
        }
        let value = value.trim();
        let text = if matches!(value, "|" | ">" | "|-" | ">-" | "|+" | ">+") {
            // YAML block scalar: consume the indented continuation lines.
            let mut parts = Vec::new();
            while let Some(next) = lines.peek() {
                if next.trim().is_empty() {
                    let _ = lines.next();
                    continue;
                }
                if !next.starts_with(' ') && !next.starts_with('\t') {
                    break;
                }
                if let Some(text) = lines.next() {
                    parts.push(text.trim().to_string());
                }
            }
            let sep = if value.starts_with('|') { "\n" } else { " " };
            parts.join(sep)
        } else {
            value.trim_matches(|c| c == '"' || c == '\'').to_string()
        };
        if text.is_empty() {
            continue;
        }
        if key == "name" {
            name = Some(text);
        } else {
            description = Some(text);
        }
    }

    Some(SkillDoc {
        name: name?,
        description: description.unwrap_or_default(),
        body,
    })
}

/// Lowercased, de-duplicated content tokens (length >= 4, plural folded).
fn tokens(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for raw in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
        let word = raw.to_lowercase();
        let folded = word.strip_suffix('s').unwrap_or(&word).to_string();
        if folded.len() < 4 || STOPWORDS.contains(&folded.as_str()) {
            continue;
        }
        if seen.insert(folded.clone()) {
            out.push(folded);
        }
    }
    out
}

/// Name hits weigh more than description hits. Matching is token-based,
/// not substring-based: "edge" must not score against "ledger".
fn score(doc: &SkillDoc, goal_tokens: &[String]) -> usize {
    let name_tokens: HashSet<String> = tokens(&doc.name).into_iter().collect();
    let description_tokens: HashSet<String> = tokens(&doc.description).into_iter().collect();
    goal_tokens
        .iter()
        .map(|t| {
            if name_tokens.contains(t) {
                3
            } else if description_tokens.contains(t) {
                1
            } else {
                0
            }
        })
        .sum()
}

/// Truncate to `max` characters, marking the cut.
fn cap_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push_str("\n... (truncated)");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    fn rigor_skill() -> &'static str {
        "---\n\
         name: estate-rigor-loop\n\
         description: >\n\
         \x20 Evidence-first execution loop: hypothesis, cheapest falsifying\n\
         \x20 test, verification ladder, root cause. Use when fixing a bug.\n\
         allowed-tools: read, grep, bash\n\
         ---\n\
         \n\
         # estate-rigor-loop\n\
         \n\
         State a hypothesis, then the cheapest test that could falsify it.\n"
    }

    #[test]
    fn folded_descriptions_parse() {
        let doc = parse_skill(rigor_skill()).unwrap();
        assert_eq!(doc.name, "estate-rigor-loop");
        assert!(doc.description.starts_with("Evidence-first execution loop"));
        assert!(doc.description.contains("falsifying test"));
        assert!(doc.body.contains("State a hypothesis"));
    }

    #[test]
    fn literal_block_scalars_keep_line_breaks() {
        let raw = "---\nname: x\ndescription: |\n  line one\n  line two\n---\nbody\n";
        let doc = parse_skill(raw).unwrap();
        assert_eq!(doc.description, "line one\nline two");
    }

    #[test]
    fn instructions_come_from_agents_md_first() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("AGENTS.md"), "# Rules\nNo unwraps.\n");
        write(&tmp.path().join("CLAUDE.md"), "# Wrong file\n");
        let ctx = build_with_roots(tmp.path(), "do a thing", &[]);
        assert!(ctx.block.contains("No unwraps."));
        assert!(!ctx.block.contains("Wrong file"));
        assert!(ctx.instructions_chars > 0);
        assert_eq!(
            ctx.summary(),
            format!("AGENTS.md {} chars", ctx.instructions_chars)
        );
    }

    #[test]
    fn relevant_skill_is_inlined_and_noise_is_not() {
        let tmp = tempfile::tempdir().unwrap();
        let skills = tmp.path().join("skills");
        write(&skills.join("estate-rigor-loop/SKILL.md"), rigor_skill());
        write(
            &skills.join("estate-seo/SKILL.md"),
            "---\nname: estate-seo\ndescription: Technical SEO and AI-engine visibility for landing pages.\n---\nSEO body.\n",
        );
        let goal = "Add unit tests for tool dispatch edge cases; a path escaping the \
                    workspace root must be rejected";
        let ctx = build_with_roots(tmp.path(), goal, &[skills.clone()]);
        assert_eq!(ctx.skills, vec!["estate-rigor-loop"]);
        assert!(ctx.block.contains("State a hypothesis"));
        assert!(!ctx.block.contains("SEO body"));
    }

    #[test]
    fn unrelated_goal_inlines_no_skills() {
        let tmp = tempfile::tempdir().unwrap();
        let skills = tmp.path().join("skills");
        write(&skills.join("estate-rigor-loop/SKILL.md"), rigor_skill());
        let ctx = build_with_roots(
            tmp.path(),
            "bump the serde version in Cargo.toml",
            &[skills],
        );
        assert!(ctx.skills.is_empty());
        assert!(ctx.block.is_empty());
    }

    #[test]
    fn a_name_hit_outweighs_a_description_hit() {
        let doc = SkillDoc {
            name: "secret-scan".into(),
            description: "Scan repos for leaked credentials.".into(),
            body: String::new(),
        };
        let goal = tokens("scan for secrets before publishing");
        assert!(score(&doc, &goal) >= 6, "name hits weigh three each");
    }

    #[test]
    fn substring_accidents_do_not_score() {
        // Live: "edge" (goal) scored against "ledger" (skill name) because
        // plain substring matching ran the match. Tokens must match whole.
        let doc = SkillDoc {
            name: "estate-plan-ledger".into(),
            description: "Crash-proof task state for multi-session work.".into(),
            body: String::new(),
        };
        let goal = tokens("tool dispatch edge cases");
        assert_eq!(score(&doc, &goal), 0, "edge is not ledger");
    }

    #[test]
    fn oversize_blocks_are_capped_and_marked() {
        let tmp = tempfile::tempdir().unwrap();
        let skills = tmp.path().join("skills");
        let long = "x".repeat(MAX_SKILL_BODY_CHARS * 3);
        write(
            &skills.join("big/SKILL.md"),
            &format!("---\nname: big-tool-skill\ndescription: tool dispatch helper\n---\n{long}\n"),
        );
        let ctx = build_with_roots(tmp.path(), "tool dispatch helper", &[skills]);
        assert!(ctx.block.chars().count() <= MAX_CONTEXT_CHARS + 32);
        assert!(ctx.block.contains("(truncated)"));
    }

    #[test]
    fn missing_everything_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = build_with_roots(tmp.path(), "anything", &[tmp.path().join("nope")]);
        assert!(ctx.is_empty());
        assert_eq!(ctx.summary(), "none");
    }

    /// Host sanity check: the real skill roots parse and something relevant
    /// matches a maintenance-style goal. Run with `--ignored --nocapture`.
    #[test]
    #[ignore = "live: inspects the host's real skill catalog"]
    fn live_catalog_parses_and_matches() {
        let root = std::env::current_dir().unwrap();
        let goal = "Add unit tests for tool dispatch edge cases; a path escaping the \
                    workspace root must be rejected";
        let ctx = build(&root, goal);
        println!("summary: {}", ctx.summary());
        println!("block: {} chars", ctx.block.chars().count());
        assert!(
            !ctx.is_empty(),
            "host has skills and the repo has AGENTS.md"
        );
        assert!(
            !ctx.skills.is_empty(),
            "a rigor/test goal should match a skill"
        );
    }
}
