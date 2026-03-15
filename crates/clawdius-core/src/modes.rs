//! Agent modes

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Agent mode configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum AgentMode {
    /// Everyday coding, file edits, quick fixes
    #[default]
    Code,
    /// System design, migrations, architecture
    Architect,
    /// Quick answers, explanations, documentation
    Ask,
    /// Debugging, logging, root cause analysis
    Debug,
    /// Code review and analysis
    Review,
    /// Code refactoring and improvement
    Refactor,
    /// Test generation
    Test,
    /// Autonomous mode for CI/CD pipelines
    Auto,
    /// Custom mode with user-defined behavior
    Custom(CustomMode),
}

impl AgentMode {
    /// Get system prompt for this mode
    #[must_use]
    pub fn system_prompt(&self) -> &str {
        match self {
            Self::Code => CODE_PROMPT,
            Self::Architect => ARCHITECT_PROMPT,
            Self::Ask => ASK_PROMPT,
            Self::Debug => DEBUG_PROMPT,
            Self::Review => REVIEW_PROMPT,
            Self::Refactor => REFACTOR_PROMPT,
            Self::Test => TEST_PROMPT,
            Self::Auto => AUTO_PROMPT,
            Self::Custom(custom) => &custom.system_prompt,
        }
    }

    /// Get temperature for this mode
    #[must_use]
    pub fn temperature(&self) -> f32 {
        match self {
            Self::Code => 0.7,
            Self::Architect => 0.5,
            Self::Ask => 0.8,
            Self::Debug => 0.6,
            Self::Review => 0.5,
            Self::Refactor => 0.6,
            Self::Test => 0.7,
            Self::Auto => 0.3, // Lower temperature for more deterministic CI/CD behavior
            Self::Custom(custom) => custom.temperature.unwrap_or(0.7),
        }
    }

    /// Get available tools for this mode
    #[must_use]
    pub fn tools(&self) -> Vec<String> {
        match self {
            Self::Code => vec!["file".to_string(), "shell".to_string(), "git".to_string()],
            Self::Architect => vec!["file".to_string(), "git".to_string()],
            Self::Ask => vec![],
            Self::Debug => vec!["file".to_string(), "shell".to_string(), "git".to_string()],
            Self::Review => vec!["file".to_string(), "git".to_string()],
            Self::Refactor => vec!["file".to_string(), "shell".to_string(), "git".to_string()],
            Self::Test => vec!["file".to_string(), "shell".to_string()],
            Self::Auto => vec!["file".to_string(), "shell".to_string(), "git".to_string()],
            Self::Custom(custom) => custom.tools.clone(),
        }
    }

    /// Get mode name
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Code => "code",
            Self::Architect => "architect",
            Self::Ask => "ask",
            Self::Debug => "debug",
            Self::Review => "review",
            Self::Refactor => "refactor",
            Self::Test => "test",
            Self::Auto => "auto",
            Self::Custom(custom) => &custom.name,
        }
    }

    /// Get mode description
    #[must_use]
    pub fn description(&self) -> &str {
        match self {
            Self::Code => "Code generation and editing",
            Self::Architect => "Design and structure planning",
            Self::Ask => "Quick answers and explanations",
            Self::Debug => "Troubleshooting and diagnostics",
            Self::Review => "Code review and analysis",
            Self::Refactor => "Code improvement and refactoring",
            Self::Test => "Test generation",
            Self::Auto => "Autonomous CI/CD mode",
            Self::Custom(custom) => custom.description.as_deref().unwrap_or("Custom mode"),
        }
    }

    /// Check if this mode requires user approval for tool execution
    #[must_use]
    pub fn requires_approval(&self) -> bool {
        match self {
            Self::Code => true,
            Self::Architect => true,
            Self::Ask => false, // No tools to approve
            Self::Debug => true,
            Self::Review => true,
            Self::Refactor => true,
            Self::Test => true,
            Self::Auto => false, // Autonomous mode - no approval needed
            Self::Custom(custom) => custom.requires_approval.unwrap_or(true),
        }
    }

    /// Get max tokens for this mode
    #[must_use]
    pub fn max_tokens(&self) -> Option<usize> {
        match self {
            Self::Code => Some(4096),
            Self::Architect => Some(8192), // Architecture docs can be longer
            Self::Ask => Some(2048),
            Self::Debug => Some(4096),
            Self::Review => Some(4096),
            Self::Refactor => Some(4096),
            Self::Test => Some(4096),
            Self::Auto => Some(8192), // CI/CD may need longer responses
            Self::Custom(custom) => custom.max_tokens,
        }
    }

    /// Check if streaming should be enabled for this mode
    #[must_use]
    pub fn enable_streaming(&self) -> bool {
        match self {
            Self::Code => true,
            Self::Architect => true,
            Self::Ask => true,
            Self::Debug => true,
            Self::Review => true,
            Self::Refactor => true,
            Self::Test => true,
            Self::Auto => false, // CI/CD often needs full response for parsing
            Self::Custom(custom) => custom.enable_streaming.unwrap_or(true),
        }
    }

    /// Parse from string
    #[must_use]
    pub fn parse_mode(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "code" => Some(Self::Code),
            "architect" => Some(Self::Architect),
            "ask" => Some(Self::Ask),
            "debug" => Some(Self::Debug),
            "review" => Some(Self::Review),
            "refactor" => Some(Self::Refactor),
            "test" => Some(Self::Test),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }

    /// Load mode from TOML file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read mode file: {}", path.display()))?;

        let config: ModeConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse mode file: {}", path.display()))?;

        Ok(Self::Custom(CustomMode {
            name: config.name,
            description: config.description,
            system_prompt: config.system_prompt,
            temperature: config.temperature,
            tools: config.tools,
            max_tokens: config.max_tokens,
            requires_approval: config.requires_approval,
            enable_streaming: config.enable_streaming,
            model: config.model,
        }))
    }

    /// Load mode by name from default modes directory
    pub fn load_by_name(name: &str, modes_dir: &Path) -> Result<Self> {
        // First check built-in modes
        if let Some(mode) = Self::parse_mode(name) {
            return Ok(mode);
        }

        // Then check custom modes
        let mode_file = modes_dir.join(format!("{name}.toml"));
        if mode_file.exists() {
            Self::load_from_file(&mode_file)
        } else {
            anyhow::bail!(
                "Mode '{}' not found in built-in modes or {}",
                name,
                modes_dir.display()
            );
        }
    }

    /// List all available modes
    pub fn list_all(modes_dir: &Path) -> Result<Vec<(String, String)>> {
        let mut modes = vec![
            (
                "code".to_string(),
                "Code generation and editing".to_string(),
            ),
            (
                "architect".to_string(),
                "Design and structure planning".to_string(),
            ),
            (
                "ask".to_string(),
                "Quick answers and explanations".to_string(),
            ),
            (
                "debug".to_string(),
                "Troubleshooting and diagnostics".to_string(),
            ),
            ("review".to_string(), "Code review and analysis".to_string()),
            (
                "refactor".to_string(),
                "Code improvement and refactoring".to_string(),
            ),
            ("test".to_string(), "Test generation".to_string()),
            ("auto".to_string(), "Autonomous CI/CD mode".to_string()),
        ];

        // Add custom modes from directory
        if modes_dir.exists() {
            for entry in fs::read_dir(modes_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "toml") {
                    if let Ok(Self::Custom(custom)) = Self::load_from_file(&path) {
                        modes.push((custom.name.clone(), custom.description.unwrap_or_default()));
                    }
                }
            }
        }

        Ok(modes)
    }
}

const CODE_PROMPT: &str = r"
You are Clawdius, an expert programmer and coding assistant. You help with:
- Writing clean, efficient, and maintainable code
- Debugging issues and fixing bugs
- Refactoring code for better performance or readability
- Implementing new features
- Code review and best practices

Always follow the project's coding standards and conventions.
";

const ARCHITECT_PROMPT: &str = r"
You are Clawdius, a software architect. You help with:
- System design and architecture decisions
- Planning migrations and refactoring
- Designing APIs and interfaces
- Evaluating trade-offs between approaches
- Creating technical documentation

Focus on long-term maintainability, scalability, and best practices.
";

const ASK_PROMPT: &str = r"
You are Clawdius, a helpful assistant. You help with:
- Answering questions about code and concepts
- Explaining how things work
- Providing documentation
- Quick tips and tricks

Be concise and clear in your explanations.
";

const DEBUG_PROMPT: &str = r"
You are Clawdius, a debugging specialist. You help with:
- Analyzing error messages and stack traces
- Finding root causes of issues
- Suggesting debugging strategies
- Adding logging and diagnostics
- Fixing bugs

Think systematically and methodically.
";

const REVIEW_PROMPT: &str = r"
You are Clawdius, a code reviewer. You help with:
- Reviewing code for quality and best practices
- Identifying potential bugs and issues
- Suggesting improvements and optimizations
- Checking for security vulnerabilities
- Ensuring code follows conventions

Provide constructive feedback and actionable suggestions.
Focus on the code quality, not the coder.
";

const REFACTOR_PROMPT: &str = r"
You are Clawdius, a refactoring specialist. You help with:
- Improving code structure and readability
- Reducing complexity and duplication
- Enhancing performance
- Modernizing legacy code
- Applying design patterns

Always preserve existing behavior and ensure tests pass.
Make incremental, safe changes.
";

const TEST_PROMPT: &str = r"
You are Clawdius, a test generation specialist. You help with:
- Writing unit tests
- Writing integration tests
- Creating test fixtures and mocks
- Ensuring code coverage
- Testing edge cases

Focus on meaningful tests that verify behavior, not just coverage.
";

const AUTO_PROMPT: &str = r"
You are Clawdius, running in autonomous CI/CD mode. You help with:
- Automated code fixes and improvements
- Running tests and fixing failures
- Implementing features from specifications
- Refactoring and code quality improvements
- Generating documentation

You operate autonomously without user interaction. Always:
1. Make safe, incremental changes
2. Run tests to verify changes
3. Commit changes with clear messages
4. Report progress and results clearly
5. Roll back changes if tests fail

Be conservative and prioritize stability over speed.
";

/// Mode configuration from TOML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeConfig {
    /// Mode name
    pub name: String,
    /// Mode description
    #[serde(default)]
    pub description: Option<String>,
    /// System prompt
    pub system_prompt: String,
    /// Temperature (0.0-1.0)
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Available tools
    #[serde(default)]
    pub tools: Vec<String>,
    /// Maximum tokens for response
    #[serde(default)]
    pub max_tokens: Option<usize>,
    /// Whether tool execution requires approval
    #[serde(default)]
    pub requires_approval: Option<bool>,
    /// Whether to enable streaming
    #[serde(default)]
    pub enable_streaming: Option<bool>,
    /// Preferred model for this mode
    #[serde(default)]
    pub model: Option<String>,
}

/// Custom mode configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomMode {
    /// Mode name
    pub name: String,
    /// System prompt
    pub system_prompt: String,
    /// Mode description
    #[serde(default)]
    pub description: Option<String>,
    /// Temperature (0.0-1.0)
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Available tools
    #[serde(default)]
    pub tools: Vec<String>,
    /// Maximum tokens for response
    #[serde(default)]
    pub max_tokens: Option<usize>,
    /// Whether tool execution requires approval
    #[serde(default)]
    pub requires_approval: Option<bool>,
    /// Whether to enable streaming
    #[serde(default)]
    pub enable_streaming: Option<bool>,
    /// Preferred model for this mode
    #[serde(default)]
    pub model: Option<String>,
}

impl std::fmt::Display for AgentMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}
