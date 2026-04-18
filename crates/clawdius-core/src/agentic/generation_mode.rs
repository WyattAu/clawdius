//! Generation Mode
//!
//! Defines the different modes for code generation.

use serde::{Deserialize, Serialize};

/// Mode of code generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationMode {
    /// Single-pass generation: Fast, one-shot generation for simple tasks.
    /// Best for: Small fixes, simple functions, documentation updates.
    SinglePass,

    /// Iterative generation: Progressive refinement with feedback loops.
    /// Best for: Medium complexity, requires refinement, test-driven.
    Iterative {
        /// Maximum number of refinement iterations
        max_iterations: u32,
    },

    /// Agent-based generation: Full autonomous workflow with planning.
    /// Best for: Complex features, multi-file changes, architectural changes.
    AgentBased {
        /// Maximum steps in the execution plan
        max_steps: u32,
        /// Whether to allow autonomous execution without confirmation
        autonomous: bool,
    },

    /// Sprint mode: Full 7-phase workflow (Think→Plan→Build→Review→Test→Ship→Reflect).
    /// Includes error recovery, multi-model review, and optional real execution.
    /// Best for: Complete features requiring thorough review and testing.
    Sprint {
        /// Maximum retry iterations when build/test fails
        max_iterations: usize,
        /// Whether to run real build/test commands
        real_execution: bool,
        /// Whether to auto-approve phase transitions
        auto_approve: bool,
    },
}

impl Default for GenerationMode {
    fn default() -> Self {
        Self::SinglePass
    }
}

impl GenerationMode {
    /// Creates a single-pass mode.
    #[must_use]
    pub const fn single_pass() -> Self {
        Self::SinglePass
    }

    /// Creates an iterative mode with default settings.
    #[must_use]
    pub fn iterative() -> Self {
        Self::Iterative { max_iterations: 3 }
    }

    /// Creates an iterative mode with custom max iterations.
    #[must_use]
    pub const fn iterative_with_max(max_iterations: u32) -> Self {
        Self::Iterative { max_iterations }
    }

    /// Creates an agent-based mode with default settings.
    #[must_use]
    pub fn agent_based() -> Self {
        Self::AgentBased {
            max_steps: 10,
            autonomous: false,
        }
    }

    /// Creates a sprint mode with default settings.
    #[must_use]
    pub fn sprint() -> Self {
        Self::Sprint {
            max_iterations: 3,
            real_execution: false,
            auto_approve: false,
        }
    }

    /// Creates a sprint mode with real execution enabled.
    #[must_use]
    pub fn sprint_with_execution(max_iterations: usize) -> Self {
        Self::Sprint {
            max_iterations,
            real_execution: true,
            auto_approve: false,
        }
    }

    /// Creates an autonomous sprint mode (auto-approve + real execution).
    #[must_use]
    pub fn autonomous_sprint(max_iterations: usize) -> Self {
        Self::Sprint {
            max_iterations,
            real_execution: true,
            auto_approve: true,
        }
    }

    /// Returns true if this is single-pass mode.
    #[must_use]
    pub const fn is_single_pass(&self) -> bool {
        matches!(self, Self::SinglePass)
    }

    /// Returns true if this is iterative mode.
    #[must_use]
    pub const fn is_iterative(&self) -> bool {
        matches!(self, Self::Iterative { .. })
    }

    /// Returns true if this is agent-based mode.
    #[must_use]
    pub const fn is_agent_based(&self) -> bool {
        matches!(self, Self::AgentBased { .. })
    }

    /// Returns true if this is sprint mode.
    #[must_use]
    pub const fn is_sprint(&self) -> bool {
        matches!(self, Self::Sprint { .. })
    }

    /// Returns the maximum iterations for iterative/sprint mode, or 1 for others.
    #[must_use]
    pub fn max_iterations(&self) -> usize {
        match self {
            Self::SinglePass => 1,
            Self::Iterative { max_iterations } => *max_iterations as usize,
            Self::AgentBased { max_steps, .. } => *max_steps as usize,
            Self::Sprint { max_iterations, .. } => *max_iterations,
        }
    }

    /// Returns a human-readable name for the mode.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::SinglePass => "Single-pass",
            Self::Iterative { .. } => "Iterative",
            Self::AgentBased {
                autonomous: false, ..
            } => "Agent-based",
            Self::AgentBased {
                autonomous: true, ..
            } => "Autonomous Agent",
            Self::Sprint { .. } => "Sprint",
        }
    }

    /// Returns a description of the mode.
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::SinglePass => "Fast, one-shot generation for simple tasks",
            Self::Iterative { .. } => "Progressive refinement with feedback loops",
            Self::AgentBased { .. } => "Full autonomous workflow with planning and verification",
            Self::Sprint { .. } => "7-phase sprint: Think→Plan→Build→Review→Test→Ship→Reflect",
        }
    }
}

/// Options for generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationOptions {
    /// The generation mode to use.
    pub mode: GenerationMode,
    /// Whether to run tests after generation.
    pub run_tests: bool,
    /// Whether to verify the generated code.
    pub verify: bool,
    /// Maximum time to spend on generation (milliseconds).
    pub timeout_ms: u64,
    /// Temperature for LLM calls.
    pub temperature: f32,
    /// Custom prompt prefix.
    pub prompt_prefix: Option<String>,
}

impl Default for GenerationOptions {
    fn default() -> Self {
        Self {
            mode: GenerationMode::default(),
            run_tests: true,
            verify: true,
            timeout_ms: 60_000, // 1 minute
            temperature: 0.7,
            prompt_prefix: None,
        }
    }
}

/// Result of code generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResult {
    /// Whether generation was successful.
    pub success: bool,
    /// The generated code (if applicable).
    pub code: Option<String>,
    /// The file path (if a file was generated).
    pub file_path: Option<String>,
    /// Number of iterations used (for iterative mode).
    pub iterations: u32,
    /// Total time spent on generation (milliseconds).
    pub duration_ms: u64,
    /// Any warnings generated during generation.
    pub warnings: Vec<String>,
    /// The mode that was used.
    pub mode: GenerationMode,
}

impl GenerationResult {
    /// Creates a successful generation result.
    #[must_use]
    pub fn success(
        code: String,
        file_path: Option<String>,
        iterations: u32,
        duration_ms: u64,
        mode: GenerationMode,
    ) -> Self {
        Self {
            success: true,
            code: Some(code),
            file_path,
            iterations,
            duration_ms,
            warnings: Vec::new(),
            mode,
        }
    }

    /// Creates a failed generation result.
    #[must_use]
    pub fn failure(duration_ms: u64, mode: GenerationMode) -> Self {
        Self {
            success: false,
            code: None,
            file_path: None,
            iterations: 0,
            duration_ms,
            warnings: Vec::new(),
            mode,
        }
    }

    /// Adds a warning to the result.
    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_constructors() {
        assert!(GenerationMode::single_pass().is_single_pass());
        assert!(GenerationMode::iterative().is_iterative());
        assert!(GenerationMode::agent_based().is_agent_based());
        assert!(GenerationMode::autonomous_agent().is_agent_based());
    }

    #[test]
    fn test_mode_serialization() {
        let mode = GenerationMode::iterative();
        let json = serde_json::to_string(&mode).unwrap();
        assert!(json.contains("iterative"));
    }

    #[test]
    fn test_max_iterations() {
        assert_eq!(GenerationMode::single_pass().max_iterations(), 1);
        assert_eq!(GenerationMode::iterative_with_max(5).max_iterations(), 5);
        assert_eq!(GenerationMode::agent_based().max_iterations(), 10);
    }

    #[test]
    fn test_generation_result() {
        let result = GenerationResult::success(
            "fn main() {}".to_string(),
            Some("src/main.rs".to_string()),
            1,
            100,
            GenerationMode::single_pass(),
        );
        assert!(result.success);
        assert_eq!(result.iterations, 1);
    }
}
