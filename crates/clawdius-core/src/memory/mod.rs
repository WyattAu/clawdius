//! Project Memory System
//!
//! This module implements a CLAUDE.md-style memory system that:
//! - Reads project instructions from CLAUDE.md files
//! - Auto-learns from interactions (build commands, debugging insights)
//! - Stores project-specific context persistently
//!
//! # Example
//!
//! ```ignore
//! use clawdius_core::memory::{ProjectMemory, MemoryStore};
//!
//! // Load project memory
//! let memory = ProjectMemory::load("/path/to/project")?;
//!
//! // Get instructions for the LLM
//! let instructions = memory.to_instructions();
//!
//! // Learn from an interaction
//! memory.learn("build_command", "cargo build --release");
//! memory.save()?;
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

/// Memory system errors
#[derive(Debug, Error)]
pub enum MemoryError {
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    /// Parse error
    #[error("Parse error: {0}")]
    Parse(String),
    /// Not found
    #[error("Memory not found: {0}")]
    NotFound(String),
}

/// Result type for memory operations
pub type Result<T> = std::result::Result<T, MemoryError>;

/// Memory entry types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MemoryEntry {
    /// Build command learned
    BuildCommand {
        command: String,
        description: Option<String>,
        learned_at: DateTime<Utc>,
        success_count: u32,
        failure_count: u32,
    },
    /// Debug insight learned
    DebugInsight {
        issue: String,
        solution: String,
        learned_at: DateTime<Utc>,
        relevance: f32,
    },
    /// Code pattern
    CodePattern {
        name: String,
        pattern: String,
        description: String,
        learned_at: DateTime<Utc>,
    },
    /// Test command
    TestCommand {
        command: String,
        description: Option<String>,
        learned_at: DateTime<Utc>,
    },
    /// Lint command
    LintCommand {
        command: String,
        description: Option<String>,
        learned_at: DateTime<Utc>,
    },
    /// Deployment command
    DeployCommand {
        command: String,
        environment: String,
        learned_at: DateTime<Utc>,
    },
    /// Custom instruction
    CustomInstruction {
        instruction: String,
        category: String,
        learned_at: DateTime<Utc>,
    },
    /// Project preference
    Preference {
        key: String,
        value: String,
        learned_at: DateTime<Utc>,
    },
}

impl MemoryEntry {
    /// Get the timestamp when this entry was learned
    #[must_use]
    pub fn learned_at(&self) -> &DateTime<Utc> {
        match self {
            MemoryEntry::BuildCommand { learned_at, .. } => learned_at,
            MemoryEntry::DebugInsight { learned_at, .. } => learned_at,
            MemoryEntry::CodePattern { learned_at, .. } => learned_at,
            MemoryEntry::TestCommand { learned_at, .. } => learned_at,
            MemoryEntry::LintCommand { learned_at, .. } => learned_at,
            MemoryEntry::DeployCommand { learned_at, .. } => learned_at,
            MemoryEntry::CustomInstruction { learned_at, .. } => learned_at,
            MemoryEntry::Preference { learned_at, .. } => learned_at,
        }
    }

    /// Get category for grouping
    #[must_use]
    pub fn category(&self) -> &str {
        match self {
            MemoryEntry::BuildCommand { .. } => "build",
            MemoryEntry::DebugInsight { .. } => "debug",
            MemoryEntry::CodePattern { .. } => "patterns",
            MemoryEntry::TestCommand { .. } => "test",
            MemoryEntry::LintCommand { .. } => "lint",
            MemoryEntry::DeployCommand { .. } => "deploy",
            MemoryEntry::CustomInstruction { .. } => "instructions",
            MemoryEntry::Preference { .. } => "preferences",
        }
    }
}

/// Project memory stored in CLAUDE.md
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMemory {
    /// Project root path
    #[serde(skip)]
    project_root: PathBuf,
    /// Manual instructions from CLAUDE.md
    instructions: String,
    /// Auto-learned entries
    learned: Vec<MemoryEntry>,
    /// Project metadata
    metadata: MemoryMetadata,
    /// Last updated
    updated_at: DateTime<Utc>,
}

/// Memory metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryMetadata {
    /// Project name
    pub project_name: Option<String>,
    /// Primary language
    pub primary_language: Option<String>,
    /// Framework
    pub framework: Option<String>,
    /// Custom fields
    pub custom: HashMap<String, String>,
}

impl ProjectMemory {
    /// Create a new empty project memory
    #[must_use]
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            instructions: String::new(),
            learned: Vec::new(),
            metadata: MemoryMetadata::default(),
            updated_at: Utc::now(),
        }
    }

    /// Load project memory from a project root
    pub fn load(project_root: impl Into<PathBuf>) -> Result<Self> {
        let project_root = project_root.into();
        let claude_md_path = project_root.join("CLAUDE.md");
        let memory_json_path = project_root.join(".clawdius/memory.json");

        let mut memory = Self::new(project_root);

        // Load manual instructions from CLAUDE.md
        if claude_md_path.exists() {
            let content = std::fs::read_to_string(&claude_md_path)?;
            memory.instructions = Self::parse_instructions(&content);
            memory.extract_metadata(&content);
        }

        // Load learned entries from memory.json
        if memory_json_path.exists() {
            let content = std::fs::read_to_string(&memory_json_path)?;
            let store: MemoryStore = serde_json::from_str(&content)?;
            memory.learned = store.learned;
            memory.updated_at = store.updated_at;
        }

        Ok(memory)
    }

    /// Parse instructions from CLAUDE.md content
    fn parse_instructions(content: &str) -> String {
        // Remove frontmatter if present
        let content = if let Some(stripped) = content.strip_prefix("---") {
            stripped.trim()
        } else {
            content
        };

        // Remove the auto-learned section if present
        if let Some(pos) = content.find("<!-- AUTO-LEARNED -->") {
            content[..pos].trim().to_string()
        } else {
            content.trim().to_string()
        }
    }

    /// Extract metadata from CLAUDE.md frontmatter
    fn extract_metadata(&mut self, content: &str) {
        if !content.starts_with("---") {
            return;
        }

        let end = match content[3..].find("---") {
            Some(i) => i + 3,
            None => return,
        };

        let frontmatter = &content[3..end];

        for line in frontmatter.lines() {
            let line = line.trim();
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');

                match key {
                    "project" | "name" => self.metadata.project_name = Some(value.to_string()),
                    "language" => self.metadata.primary_language = Some(value.to_string()),
                    "framework" => self.metadata.framework = Some(value.to_string()),
                    _ => {
                        self.metadata
                            .custom
                            .insert(key.to_string(), value.to_string());
                    },
                }
            }
        }
    }

    /// Save memory to disk
    pub fn save(&self) -> Result<()> {
        let memory_dir = self.project_root.join(".clawdius");
        std::fs::create_dir_all(&memory_dir)?;

        let memory_json_path = memory_dir.join("memory.json");
        let store = MemoryStore {
            learned: self.learned.clone(),
            updated_at: Utc::now(),
        };

        let content = serde_json::to_string_pretty(&store)?;
        std::fs::write(&memory_json_path, content)?;

        Ok(())
    }

    /// Get manual instructions
    #[must_use]
    pub fn instructions(&self) -> &str {
        &self.instructions
    }

    /// Set manual instructions
    pub fn set_instructions(&mut self, instructions: impl Into<String>) {
        self.instructions = instructions.into();
        self.updated_at = Utc::now();
    }

    /// Get all learned entries
    #[must_use]
    pub fn learned(&self) -> &[MemoryEntry] {
        &self.learned
    }

    /// Get learned entries by category
    #[must_use]
    pub fn learned_by_category(&self, category: &str) -> Vec<&MemoryEntry> {
        self.learned
            .iter()
            .filter(|e| e.category() == category)
            .collect()
    }

    /// Learn a new entry
    pub fn learn(&mut self, entry: MemoryEntry) {
        // Check for duplicates and update counts if applicable
        match &entry {
            MemoryEntry::BuildCommand { command, .. } => {
                if let Some(existing) = self.learned.iter_mut().find_map(|e| {
                    if let MemoryEntry::BuildCommand { command: c, .. } = e {
                        if c == command {
                            Some(e)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }) {
                    if let MemoryEntry::BuildCommand { success_count, .. } = existing {
                        *success_count += 1;
                    }
                    return;
                }
            },
            MemoryEntry::TestCommand { command, .. } => {
                if self.learned.iter().any(
                    |e| matches!(e, MemoryEntry::TestCommand { command: c, .. } if c == command),
                ) {
                    return;
                }
            },
            _ => {},
        }

        self.learned.push(entry);
        self.updated_at = Utc::now();
    }

    /// Learn a build command
    pub fn learn_build_command(&mut self, command: impl Into<String>, description: Option<String>) {
        self.learn(MemoryEntry::BuildCommand {
            command: command.into(),
            description,
            learned_at: Utc::now(),
            success_count: 1,
            failure_count: 0,
        });
    }

    /// Learn a debug insight
    pub fn learn_debug_insight(&mut self, issue: impl Into<String>, solution: impl Into<String>) {
        self.learn(MemoryEntry::DebugInsight {
            issue: issue.into(),
            solution: solution.into(),
            learned_at: Utc::now(),
            relevance: 1.0,
        });
    }

    /// Learn a test command
    pub fn learn_test_command(&mut self, command: impl Into<String>, description: Option<String>) {
        self.learn(MemoryEntry::TestCommand {
            command: command.into(),
            description,
            learned_at: Utc::now(),
        });
    }

    /// Learn a code pattern
    pub fn learn_code_pattern(
        &mut self,
        name: impl Into<String>,
        pattern: impl Into<String>,
        description: impl Into<String>,
    ) {
        self.learn(MemoryEntry::CodePattern {
            name: name.into(),
            pattern: pattern.into(),
            description: description.into(),
            learned_at: Utc::now(),
        });
    }

    /// Learn a preference
    pub fn learn_preference(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();

        // Update existing preference or add new
        if let Some(existing) = self
            .learned
            .iter_mut()
            .find(|e| matches!(e, MemoryEntry::Preference { key: k, .. } if *k == key))
        {
            if let MemoryEntry::Preference {
                value: v,
                learned_at,
                ..
            } = existing
            {
                *v = value;
                *learned_at = Utc::now();
            }
            return;
        }

        self.learn(MemoryEntry::Preference {
            key,
            value,
            learned_at: Utc::now(),
        });
    }

    /// Get a preference
    #[must_use]
    pub fn get_preference(&self, key: &str) -> Option<&str> {
        self.learned.iter().find_map(|e| {
            if let MemoryEntry::Preference { key: k, value, .. } = e {
                if k == key {
                    Some(value.as_str())
                } else {
                    None
                }
            } else {
                None
            }
        })
    }

    /// Get build commands
    #[must_use]
    pub fn build_commands(&self) -> Vec<(&str, Option<&str>)> {
        self.learned
            .iter()
            .filter_map(|e| {
                if let MemoryEntry::BuildCommand {
                    command,
                    description,
                    ..
                } = e
                {
                    Some((command.as_str(), description.as_deref()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get test commands
    #[must_use]
    pub fn test_commands(&self) -> Vec<(&str, Option<&str>)> {
        self.learned
            .iter()
            .filter_map(|e| {
                if let MemoryEntry::TestCommand {
                    command,
                    description,
                    ..
                } = e
                {
                    Some((command.as_str(), description.as_deref()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get debug insights
    #[must_use]
    pub fn debug_insights(&self) -> Vec<(&str, &str)> {
        self.learned
            .iter()
            .filter_map(|e| {
                if let MemoryEntry::DebugInsight {
                    issue, solution, ..
                } = e
                {
                    Some((issue.as_str(), solution.as_str()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Convert to instructions for LLM context
    #[must_use]
    pub fn to_instructions(&self) -> String {
        let mut output = String::new();

        // Add manual instructions
        if !self.instructions.is_empty() {
            output.push_str("# Project Instructions\n\n");
            output.push_str(&self.instructions);
            output.push_str("\n\n");
        }

        // Add metadata
        if let Some(name) = &self.metadata.project_name {
            output.push_str(&format!("**Project:** {name}\n"));
        }
        if let Some(lang) = &self.metadata.primary_language {
            output.push_str(&format!("**Language:** {lang}\n"));
        }
        if let Some(fw) = &self.metadata.framework {
            output.push_str(&format!("**Framework:** {fw}\n"));
        }

        // Add learned commands
        let build_commands = self.build_commands();
        if !build_commands.is_empty() {
            output.push_str("\n## Build Commands\n\n");
            for (cmd, desc) in &build_commands {
                if let Some(d) = desc {
                    output.push_str(&format!("- `{cmd}` - {d}\n"));
                } else {
                    output.push_str(&format!("- `{cmd}`\n"));
                }
            }
        }

        let test_commands = self.test_commands();
        if !test_commands.is_empty() {
            output.push_str("\n## Test Commands\n\n");
            for (cmd, desc) in &test_commands {
                if let Some(d) = desc {
                    output.push_str(&format!("- `{cmd}` - {d}\n"));
                } else {
                    output.push_str(&format!("- `{cmd}`\n"));
                }
            }
        }

        // Add debug insights
        let insights = self.debug_insights();
        if !insights.is_empty() {
            output.push_str("\n## Known Issues & Solutions\n\n");
            for (issue, solution) in &insights {
                output.push_str(&format!("**Issue:** {issue}\n**Solution:** {solution}\n\n"));
            }
        }

        // Add preferences
        let preferences: Vec<_> = self
            .learned
            .iter()
            .filter_map(|e| {
                if let MemoryEntry::Preference { key, value, .. } = e {
                    Some((key.as_str(), value.as_str()))
                } else {
                    None
                }
            })
            .collect();

        if !preferences.is_empty() {
            output.push_str("\n## Preferences\n\n");
            for (key, value) in &preferences {
                output.push_str(&format!("- {key}: {value}\n"));
            }
        }

        output
    }

    /// Get metadata
    #[must_use]
    pub fn metadata(&self) -> &MemoryMetadata {
        &self.metadata
    }

    /// Get mutable metadata
    pub fn metadata_mut(&mut self) -> &mut MemoryMetadata {
        &mut self.metadata
    }

    /// Clear all learned entries
    pub fn clear_learned(&mut self) {
        self.learned.clear();
        self.updated_at = Utc::now();
    }

    /// Remove entries by category
    pub fn remove_by_category(&mut self, category: &str) {
        self.learned.retain(|e| e.category() != category);
        self.updated_at = Utc::now();
    }
}

/// Memory store for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryStore {
    /// Learned entries
    learned: Vec<MemoryEntry>,
    /// Last updated
    updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_memory_creation() {
        let memory = ProjectMemory::new("/tmp/test");
        assert!(memory.instructions().is_empty());
        assert!(memory.learned().is_empty());
    }

    #[test]
    fn test_learn_build_command() {
        let mut memory = ProjectMemory::new("/tmp/test");
        memory.learn_build_command(
            "cargo build --release",
            Some("Build in release mode".into()),
        );

        let commands = memory.build_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].0, "cargo build --release");
    }

    #[test]
    fn test_learn_debug_insight() {
        let mut memory = ProjectMemory::new("/tmp/test");
        memory.learn_debug_insight("Missing dependency", "Run cargo fetch");

        let insights = memory.debug_insights();
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].0, "Missing dependency");
        assert_eq!(insights[0].1, "Run cargo fetch");
    }

    #[test]
    fn test_preferences() {
        let mut memory = ProjectMemory::new("/tmp/test");
        memory.learn_preference("style", "use_tabs");
        memory.learn_preference("max_line_length", "100");

        assert_eq!(memory.get_preference("style"), Some("use_tabs"));
        assert_eq!(memory.get_preference("max_line_length"), Some("100"));

        // Update preference
        memory.learn_preference("style", "use_spaces");
        assert_eq!(memory.get_preference("style"), Some("use_spaces"));
    }

    #[test]
    fn test_to_instructions() {
        let mut memory = ProjectMemory::new("/tmp/test");
        memory.set_instructions("Always use descriptive variable names.");
        memory.learn_build_command("cargo build", Some("Build the project".into()));

        let instructions = memory.to_instructions();
        assert!(instructions.contains("Always use descriptive variable names"));
        assert!(instructions.contains("cargo build"));
    }

    #[test]
    fn test_save_and_load() -> Result<()> {
        let dir = tempdir()?;
        let project_root = dir.path();

        // Create CLAUDE.md
        let claude_md = project_root.join("CLAUDE.md");
        std::fs::write(&claude_md, "# Test Project\n\nUse 4-space indentation.")?;

        // Load and modify
        let mut memory = ProjectMemory::load(project_root)?;
        memory.learn_build_command("cargo test", Some("Run tests".into()));
        memory.save()?;

        // Reload
        let memory2 = ProjectMemory::load(project_root)?;
        assert!(memory2.instructions().contains("4-space indentation"));
        assert_eq!(memory2.build_commands().len(), 1);

        Ok(())
    }

    #[test]
    fn test_frontmatter_parsing() {
        let content = r"---
project: my-project
language: rust
framework: axum
---
# Instructions
Use idiomatic Rust.
";

        let mut memory = ProjectMemory::new("/tmp/test");
        memory.extract_metadata(content);

        assert_eq!(memory.metadata().project_name, Some("my-project".into()));
        assert_eq!(memory.metadata().primary_language, Some("rust".into()));
        assert_eq!(memory.metadata().framework, Some("axum".into()));
    }
}
