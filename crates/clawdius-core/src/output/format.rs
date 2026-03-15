//! Output format implementations

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Output format configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    /// Human-readable text (default)
    #[default]
    Text,
    /// Single JSON object
    Json,
    /// Newline-delimited JSON events
    StreamJson,
}

impl OutputFormat {
    /// Parse from string
    #[must_use]
    pub fn parse_format(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "text" => Some(Self::Text),
            "json" => Some(Self::Json),
            "stream-json" | "stream_json" | "streamjson" => Some(Self::StreamJson),
            _ => None,
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text => write!(f, "text"),
            Self::Json => write!(f, "json"),
            Self::StreamJson => write!(f, "stream-json"),
        }
    }
}

/// Output options
#[derive(Debug, Clone)]
pub struct OutputOptions {
    /// Output format
    pub format: OutputFormat,
    /// Show progress indicators
    pub show_progress: bool,
    /// Quiet mode (no output except results)
    pub quiet: bool,
    /// Include metadata in output
    pub include_metadata: bool,
}

impl Default for OutputOptions {
    fn default() -> Self {
        Self {
            format: OutputFormat::Text,
            show_progress: true,
            quiet: false,
            include_metadata: true,
        }
    }
}

/// JSON output structure for structured responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonOutput {
    /// Response content
    pub content: String,
    /// Session ID
    pub session_id: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Tool calls made during response
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCallInfo>,
    /// Files modified during response
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_changed: Vec<FileChange>,
    /// Token usage
    pub usage: TokenUsageInfo,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Success status
    pub success: bool,
    /// Error message (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl JsonOutput {
    /// Create a successful output
    pub fn success(content: impl Into<String>, session_id: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            session_id: session_id.into(),
            timestamp: Utc::now(),
            tool_calls: Vec::new(),
            files_changed: Vec::new(),
            usage: TokenUsageInfo::default(),
            duration_ms: 0,
            success: true,
            error: None,
        }
    }

    /// Create an error output
    pub fn error(error: impl Into<String>, session_id: impl Into<String>) -> Self {
        Self {
            content: String::new(),
            session_id: session_id.into(),
            timestamp: Utc::now(),
            tool_calls: Vec::new(),
            files_changed: Vec::new(),
            usage: TokenUsageInfo::default(),
            duration_ms: 0,
            success: false,
            error: Some(error.into()),
        }
    }

    /// Add a tool call
    #[must_use]
    pub fn with_tool_call(mut self, tool_call: ToolCallInfo) -> Self {
        self.tool_calls.push(tool_call);
        self
    }

    /// Add a file change
    #[must_use]
    pub fn with_file_change(mut self, change: FileChange) -> Self {
        self.files_changed.push(change);
        self
    }

    /// Set token usage
    #[must_use]
    pub fn with_usage(mut self, usage: TokenUsageInfo) -> Self {
        self.usage = usage;
        self
    }

    /// Set duration
    #[must_use]
    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> crate::Result<String> {
        serde_json::to_string_pretty(self).map_err(crate::Error::Serialization)
    }

    /// Convert to compact JSON string
    pub fn to_json_compact(&self) -> crate::Result<String> {
        serde_json::to_string(self).map_err(crate::Error::Serialization)
    }
}

/// Information about a tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallInfo {
    /// Tool name
    pub name: String,
    /// Tool arguments
    pub arguments: serde_json::Value,
    /// Tool result (if completed)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Whether the call succeeded
    pub success: bool,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Information about a file change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    /// File path
    pub path: String,
    /// Change type
    pub change_type: ChangeType,
    /// Lines added
    pub lines_added: usize,
    /// Lines removed
    pub lines_removed: usize,
}

/// Type of file change
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    /// File created
    Created,
    /// File modified
    Modified,
    /// File deleted
    Deleted,
    /// File renamed
    Renamed,
}

/// Token usage information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsageInfo {
    /// Input tokens
    pub input: usize,
    /// Output tokens
    pub output: usize,
    /// Total tokens
    pub total: usize,
    /// Cached tokens (if applicable)
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cached: usize,
}

fn is_zero(val: &usize) -> bool {
    *val == 0
}

impl TokenUsageInfo {
    /// Create new token usage info
    #[must_use]
    pub fn new(input: usize, output: usize) -> Self {
        Self {
            input,
            output,
            total: input + output,
            cached: 0,
        }
    }
}

/// JSON output for init command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitResult {
    /// Success status
    pub success: bool,
    /// Path where Clawdius was initialized
    pub path: String,
    /// Created configuration file path
    pub config_path: String,
    /// Onboarding status
    pub onboarding_complete: bool,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Error message (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl InitResult {
    /// Create a successful init result
    pub fn success(
        path: impl Into<String>,
        config_path: impl Into<String>,
        onboarding_complete: bool,
    ) -> Self {
        Self {
            success: true,
            path: path.into(),
            config_path: config_path.into(),
            onboarding_complete,
            timestamp: Utc::now(),
            error: None,
        }
    }

    /// Create an error result
    pub fn error(error: impl Into<String>) -> Self {
        Self {
            success: false,
            path: String::new(),
            config_path: String::new(),
            onboarding_complete: false,
            timestamp: Utc::now(),
            error: Some(error.into()),
        }
    }
}

/// JSON output for config command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResult {
    /// Success status
    pub success: bool,
    /// Configuration data
    pub config: serde_json::Value,
    /// Configuration file path
    pub config_path: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Error message (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ConfigResult {
    /// Create a successful config result
    pub fn success(config: serde_json::Value, config_path: impl Into<String>) -> Self {
        Self {
            success: true,
            config,
            config_path: config_path.into(),
            timestamp: Utc::now(),
            error: None,
        }
    }

    /// Create an error result
    pub fn error(error: impl Into<String>) -> Self {
        Self {
            success: false,
            config: serde_json::json!(null),
            config_path: String::new(),
            timestamp: Utc::now(),
            error: Some(error.into()),
        }
    }
}

/// JSON output for metrics command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsResult {
    /// Total requests
    pub requests_total: u64,
    /// Total errors
    pub requests_errors: u64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// Total tokens used
    pub tokens_used: usize,
    /// Error rate
    pub error_rate: f64,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl MetricsResult {
    /// Create from metrics snapshot
    #[must_use]
    pub fn new(
        requests_total: u64,
        requests_errors: u64,
        avg_latency_ms: f64,
        tokens_used: usize,
        error_rate: f64,
    ) -> Self {
        Self {
            requests_total,
            requests_errors,
            avg_latency_ms,
            tokens_used,
            error_rate,
            timestamp: Utc::now(),
        }
    }
}

/// JSON output for verify command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResult {
    /// Success status
    pub success: bool,
    /// Proof file path
    pub proof_path: String,
    /// Verification duration in milliseconds
    pub duration_ms: u64,
    /// Errors found
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ProofError>,
    /// Warnings found
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl VerifyResult {
    /// Create a successful verification result
    pub fn success(proof_path: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            success: true,
            proof_path: proof_path.into(),
            duration_ms,
            errors: Vec::new(),
            warnings: Vec::new(),
            timestamp: Utc::now(),
        }
    }

    /// Create a failed verification result
    pub fn failure(
        proof_path: impl Into<String>,
        duration_ms: u64,
        errors: Vec<ProofError>,
        warnings: Vec<String>,
    ) -> Self {
        Self {
            success: false,
            proof_path: proof_path.into(),
            duration_ms,
            errors,
            warnings,
            timestamp: Utc::now(),
        }
    }
}

/// Proof error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofError {
    /// Line number
    pub line: usize,
    /// Column number
    pub column: usize,
    /// Error message
    pub message: String,
}

/// JSON output for refactor command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorResult {
    /// Success status
    pub success: bool,
    /// Source language
    pub from_language: String,
    /// Target language
    pub to_language: String,
    /// Input path
    pub input_path: String,
    /// Whether this was a dry run
    pub dry_run: bool,
    /// Files that would be/were changed
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_changed: Vec<RefactorFileChange>,
    /// Summary message
    pub message: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Error message (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl RefactorResult {
    /// Create a successful refactor result
    pub fn success(
        from: impl Into<String>,
        to: impl Into<String>,
        path: impl Into<String>,
        dry_run: bool,
        files_changed: Vec<RefactorFileChange>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            success: true,
            from_language: from.into(),
            to_language: to.into(),
            input_path: path.into(),
            dry_run,
            files_changed,
            message: message.into(),
            timestamp: Utc::now(),
            error: None,
        }
    }

    /// Create an error result
    pub fn error(error: impl Into<String>) -> Self {
        Self {
            success: false,
            from_language: String::new(),
            to_language: String::new(),
            input_path: String::new(),
            dry_run: false,
            files_changed: Vec::new(),
            message: String::new(),
            timestamp: Utc::now(),
            error: Some(error.into()),
        }
    }
}

/// File change information for refactor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefactorFileChange {
    /// Original file path
    pub original_path: String,
    /// New file path (if renamed)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_path: Option<String>,
    /// Change type
    pub change_type: String,
    /// Lines added
    pub lines_added: usize,
    /// Lines removed
    pub lines_removed: usize,
}

/// JSON output for broker command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerResult {
    /// Success status
    pub success: bool,
    /// Whether paper trading is enabled
    pub paper_trade: bool,
    /// Broker status
    pub status: String,
    /// Configuration path
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
    /// Message
    pub message: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Error message (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl BrokerResult {
    /// Create a successful broker result
    pub fn success(
        paper_trade: bool,
        status: impl Into<String>,
        config_path: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            success: true,
            paper_trade,
            status: status.into(),
            config_path,
            message: message.into(),
            timestamp: Utc::now(),
            error: None,
        }
    }

    /// Create an error result
    pub fn error(error: impl Into<String>) -> Self {
        Self {
            success: false,
            paper_trade: false,
            status: String::new(),
            config_path: None,
            message: String::new(),
            timestamp: Utc::now(),
            error: Some(error.into()),
        }
    }
}

/// JSON output for compliance command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceResult {
    /// Success status
    pub success: bool,
    /// Standards included
    pub standards: Vec<String>,
    /// Project path
    pub project_path: String,
    /// Output format
    pub output_format: String,
    /// Output path (if written to file)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    /// Compliance matrix data
    pub matrix: serde_json::Value,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Error message (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ComplianceResult {
    /// Create a successful compliance result
    pub fn success(
        standards: Vec<String>,
        project_path: impl Into<String>,
        output_format: impl Into<String>,
        output_path: Option<String>,
        matrix: serde_json::Value,
    ) -> Self {
        Self {
            success: true,
            standards,
            project_path: project_path.into(),
            output_format: output_format.into(),
            output_path,
            matrix,
            timestamp: Utc::now(),
            error: None,
        }
    }

    /// Create an error result
    pub fn error(error: impl Into<String>) -> Self {
        Self {
            success: false,
            standards: Vec::new(),
            project_path: String::new(),
            output_format: String::new(),
            output_path: None,
            matrix: serde_json::json!(null),
            timestamp: Utc::now(),
            error: Some(error.into()),
        }
    }
}

/// JSON output for research command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchResult {
    /// Success status
    pub success: bool,
    /// Query string
    pub query: String,
    /// Languages covered
    pub languages_covered: Vec<String>,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Concepts found
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concepts: Vec<ResearchConcept>,
    /// Relationships found
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationships: Vec<ResearchRelationship>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Error message (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ResearchResult {
    /// Create a successful research result
    pub fn success(
        query: impl Into<String>,
        languages_covered: Vec<String>,
        confidence: f64,
        concepts: Vec<ResearchConcept>,
        relationships: Vec<ResearchRelationship>,
    ) -> Self {
        Self {
            success: true,
            query: query.into(),
            languages_covered,
            confidence,
            concepts,
            relationships,
            timestamp: Utc::now(),
            error: None,
        }
    }

    /// Create an error result
    pub fn error(error: impl Into<String>) -> Self {
        Self {
            success: false,
            query: String::new(),
            languages_covered: Vec::new(),
            confidence: 0.0,
            concepts: Vec::new(),
            relationships: Vec::new(),
            timestamp: Utc::now(),
            error: Some(error.into()),
        }
    }
}

/// Research concept information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchConcept {
    /// Language
    pub language: String,
    /// Concept name
    pub name: String,
    /// Concept definition
    pub definition: String,
}

/// Research relationship information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchRelationship {
    /// Source concept
    pub from: String,
    /// Relationship type
    pub relationship: String,
    /// Target concept
    pub to: String,
}

/// Timeline result for JSON output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineResult {
    /// Checkpoint ID
    pub checkpoint_id: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Number of files changed
    pub files_changed: usize,
    /// Description
    pub description: String,
}

impl TimelineResult {
    /// Create a timeline result
    #[must_use]
    pub fn new(
        checkpoint_id: String,
        timestamp: DateTime<Utc>,
        files_changed: usize,
        description: String,
    ) -> Self {
        Self {
            checkpoint_id,
            timestamp,
            files_changed,
            description,
        }
    }
}

/// File version information for JSON output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileVersionInfo {
    /// File path
    pub path: String,
    /// Version number
    pub version: u64,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Content checksum
    pub checksum: String,
    /// File size in bytes
    pub size: usize,
}

impl FileVersionInfo {
    /// Create a file version info
    #[must_use]
    pub fn new(
        path: String,
        version: u64,
        timestamp: DateTime<Utc>,
        checksum: String,
        size: usize,
    ) -> Self {
        Self {
            path,
            version,
            timestamp,
            checksum,
            size,
        }
    }
}

/// JSON output for action command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    /// Success status
    pub success: bool,
    /// Action type
    pub action: String,
    /// File path
    pub file: String,
    /// Action title
    pub title: String,
    /// Action kind
    pub kind: String,
    /// Edits to apply
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edits: Vec<ActionEdit>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Error message (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ActionResult {
    /// Create a successful action result
    pub fn success(
        action: impl Into<String>,
        file: impl Into<String>,
        title: impl Into<String>,
        kind: impl Into<String>,
        edits: Vec<ActionEdit>,
    ) -> Self {
        Self {
            success: true,
            action: action.into(),
            file: file.into(),
            title: title.into(),
            kind: kind.into(),
            edits,
            timestamp: Utc::now(),
            error: None,
        }
    }

    /// Create an error result
    pub fn error(
        action: impl Into<String>,
        file: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            success: false,
            action: action.into(),
            file: file.into(),
            title: String::new(),
            kind: String::new(),
            edits: Vec::new(),
            timestamp: Utc::now(),
            error: Some(error.into()),
        }
    }
}

/// Edit information for actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionEdit {
    /// Start position
    pub start_line: usize,
    pub start_column: usize,
    /// End position
    pub end_line: usize,
    pub end_column: usize,
    /// New text
    pub new_text: String,
}

/// JSON output for test command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Success status
    pub success: bool,
    /// File path
    pub file: String,
    /// Function name (if specified)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    /// Language
    pub language: String,
    /// Generated test cases
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test_cases: Vec<TestCaseInfo>,
    /// Output file path (if written)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Error message (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl TestResult {
    /// Create a successful test result
    pub fn success(
        file: impl Into<String>,
        function: Option<String>,
        language: impl Into<String>,
        test_cases: Vec<TestCaseInfo>,
        output_path: Option<String>,
    ) -> Self {
        Self {
            success: true,
            file: file.into(),
            function,
            language: language.into(),
            test_cases,
            output_path,
            timestamp: Utc::now(),
            error: None,
        }
    }

    /// Create an error result
    pub fn error(file: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            success: false,
            file: file.into(),
            function: None,
            language: String::new(),
            test_cases: Vec::new(),
            output_path: None,
            timestamp: Utc::now(),
            error: Some(error.into()),
        }
    }
}

/// Test case information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseInfo {
    /// Test name
    pub name: String,
    /// Test description
    pub description: String,
    /// Test code
    pub code: String,
}

/// JSON output for index command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexResult {
    /// Success status
    pub success: bool,
    /// Workspace path
    pub workspace_path: String,
    /// Files indexed
    pub files_indexed: usize,
    /// Symbols found
    pub symbols_found: usize,
    /// References found
    pub references_found: usize,
    /// Embeddings created
    pub embeddings_created: usize,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Errors encountered
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Error message (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl IndexResult {
    /// Create a successful index result
    pub fn success(
        workspace_path: impl Into<String>,
        files_indexed: usize,
        symbols_found: usize,
        references_found: usize,
        embeddings_created: usize,
        duration_ms: u64,
        errors: Vec<String>,
    ) -> Self {
        Self {
            success: true,
            workspace_path: workspace_path.into(),
            files_indexed,
            symbols_found,
            references_found,
            embeddings_created,
            duration_ms,
            errors,
            timestamp: Utc::now(),
            error: None,
        }
    }

    /// Create an error result
    pub fn error(workspace_path: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            success: false,
            workspace_path: workspace_path.into(),
            files_indexed: 0,
            symbols_found: 0,
            references_found: 0,
            embeddings_created: 0,
            duration_ms: 0,
            errors: Vec::new(),
            timestamp: Utc::now(),
            error: Some(error.into()),
        }
    }
}

/// JSON output for context command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextResult {
    /// Success status
    pub success: bool,
    /// Query string
    pub query: String,
    /// Max tokens requested
    pub max_tokens: usize,
    /// Total tokens gathered
    pub total_tokens: usize,
    /// Files included
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<ContextFile>,
    /// Symbols included
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbols: Vec<ContextSymbol>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Error message (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ContextResult {
    /// Create a successful context result
    pub fn success(
        query: impl Into<String>,
        max_tokens: usize,
        total_tokens: usize,
        files: Vec<ContextFile>,
        symbols: Vec<ContextSymbol>,
    ) -> Self {
        Self {
            success: true,
            query: query.into(),
            max_tokens,
            total_tokens,
            files,
            symbols,
            timestamp: Utc::now(),
            error: None,
        }
    }

    /// Create an error result
    pub fn error(query: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            success: false,
            query: query.into(),
            max_tokens: 0,
            total_tokens: 0,
            files: Vec::new(),
            symbols: Vec::new(),
            timestamp: Utc::now(),
            error: Some(error.into()),
        }
    }
}

/// File information for context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextFile {
    /// File path
    pub path: String,
    /// Token count
    pub token_count: usize,
    /// Symbols in file
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbols: Vec<String>,
}

/// Symbol information for context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSymbol {
    /// Symbol name
    pub name: String,
    /// Symbol kind
    pub kind: String,
    /// Location
    pub location: String,
    /// Token count
    pub token_count: usize,
}

/// JSON output for checkpoint command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointResult {
    /// Success status
    pub success: bool,
    /// Operation performed
    pub operation: String,
    /// Checkpoint ID (if applicable)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_id: Option<String>,
    /// Session ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// File count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_count: Option<usize>,
    /// Checkpoints (for list operation)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checkpoints: Vec<CheckpointInfo>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Error message (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl CheckpointResult {
    /// Create a successful checkpoint result
    pub fn success(operation: impl Into<String>) -> Self {
        Self {
            success: true,
            operation: operation.into(),
            checkpoint_id: None,
            session_id: None,
            description: None,
            file_count: None,
            checkpoints: Vec::new(),
            timestamp: Utc::now(),
            error: None,
        }
    }

    /// Create an error result
    pub fn error(operation: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            success: false,
            operation: operation.into(),
            checkpoint_id: None,
            session_id: None,
            description: None,
            file_count: None,
            checkpoints: Vec::new(),
            timestamp: Utc::now(),
            error: Some(error.into()),
        }
    }

    /// Add checkpoint ID
    pub fn with_checkpoint_id(mut self, id: impl Into<String>) -> Self {
        self.checkpoint_id = Some(id.into());
        self
    }

    /// Add session ID
    pub fn with_session_id(mut self, id: impl Into<String>) -> Self {
        self.session_id = Some(id.into());
        self
    }

    /// Add description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Add file count
    #[must_use]
    pub fn with_file_count(mut self, count: usize) -> Self {
        self.file_count = Some(count);
        self
    }

    /// Add checkpoints list
    #[must_use]
    pub fn with_checkpoints(mut self, checkpoints: Vec<CheckpointInfo>) -> Self {
        self.checkpoints = checkpoints;
        self
    }
}

/// Checkpoint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointInfo {
    /// Checkpoint ID
    pub id: String,
    /// Description
    pub description: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// File count
    pub file_count: usize,
}

/// JSON output for modes command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModesResult {
    /// Success status
    pub success: bool,
    /// Operation performed
    pub operation: String,
    /// Mode name (if applicable)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_name: Option<String>,
    /// Available modes (for list operation)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modes: Vec<ModeInfo>,
    /// Mode details (for show operation)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_details: Option<ModeDetails>,
    /// Created file path (for create operation)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_path: Option<String>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Error message (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ModesResult {
    /// Create a successful modes result
    pub fn success(operation: impl Into<String>) -> Self {
        Self {
            success: true,
            operation: operation.into(),
            mode_name: None,
            modes: Vec::new(),
            mode_details: None,
            created_path: None,
            timestamp: Utc::now(),
            error: None,
        }
    }

    /// Create an error result
    pub fn error(operation: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            success: false,
            operation: operation.into(),
            mode_name: None,
            modes: Vec::new(),
            mode_details: None,
            created_path: None,
            timestamp: Utc::now(),
            error: Some(error.into()),
        }
    }

    /// Add mode name
    pub fn with_mode_name(mut self, name: impl Into<String>) -> Self {
        self.mode_name = Some(name.into());
        self
    }

    /// Add modes list
    #[must_use]
    pub fn with_modes(mut self, modes: Vec<ModeInfo>) -> Self {
        self.modes = modes;
        self
    }

    /// Add mode details
    #[must_use]
    pub fn with_mode_details(mut self, details: ModeDetails) -> Self {
        self.mode_details = Some(details);
        self
    }

    /// Add created path
    pub fn with_created_path(mut self, path: impl Into<String>) -> Self {
        self.created_path = Some(path.into());
        self
    }
}

/// Mode information for list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeInfo {
    /// Mode name
    pub name: String,
    /// Mode description
    pub description: String,
}

/// Mode details for show
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeDetails {
    /// Mode name
    pub name: String,
    /// Mode description
    pub description: String,
    /// System prompt
    pub system_prompt: String,
    /// Temperature
    pub temperature: f32,
    /// Available tools
    pub tools: Vec<String>,
}

/// JSON output for telemetry command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryResult {
    /// Success status
    pub success: bool,
    /// Metrics enabled
    pub metrics_enabled: bool,
    /// Crash reporting enabled
    pub crash_reporting_enabled: bool,
    /// Performance monitoring enabled
    pub performance_monitoring_enabled: bool,
    /// Config path
    pub config_path: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Error message (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl TelemetryResult {
    /// Create a successful telemetry result
    pub fn success(
        metrics_enabled: bool,
        crash_reporting_enabled: bool,
        performance_monitoring_enabled: bool,
        config_path: impl Into<String>,
    ) -> Self {
        Self {
            success: true,
            metrics_enabled,
            crash_reporting_enabled,
            performance_monitoring_enabled,
            config_path: config_path.into(),
            timestamp: Utc::now(),
            error: None,
        }
    }

    /// Create an error result
    pub fn error(error: impl Into<String>) -> Self {
        Self {
            success: false,
            metrics_enabled: false,
            crash_reporting_enabled: false,
            performance_monitoring_enabled: false,
            config_path: String::new(),
            timestamp: Utc::now(),
            error: Some(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_output_success() {
        let output = JsonOutput::success("Hello, world!", "session-123");
        assert!(output.success);
        assert!(output.error.is_none());
        assert_eq!(output.content, "Hello, world!");
    }

    #[test]
    fn test_json_output_error() {
        let output = JsonOutput::error("Something went wrong", "session-123");
        assert!(!output.success);
        assert_eq!(output.error, Some("Something went wrong".to_string()));
    }

    #[test]
    fn test_json_output_serialization() {
        let output = JsonOutput::success("Test", "session-123")
            .with_usage(TokenUsageInfo::new(100, 50))
            .with_duration(1500);

        let json = output.to_json().unwrap();
        assert!(json.contains("\"content\": \"Test\""));
        assert!(json.contains("\"duration_ms\": 1500"));
    }

    #[test]
    fn test_action_result_success() {
        let result = ActionResult::success(
            "extract-function",
            "src/main.rs",
            "Extract function",
            "refactor.extract.function",
            vec![],
        );
        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_action_result_error() {
        let result = ActionResult::error("extract-function", "src/main.rs", "Invalid selection");
        assert!(!result.success);
        assert_eq!(result.error, Some("Invalid selection".to_string()));
    }

    #[test]
    fn test_index_result_success() {
        let result = IndexResult::success("/workspace", 10, 50, 100, 25, 1500, vec![]);
        assert!(result.success);
        assert_eq!(result.files_indexed, 10);
        assert_eq!(result.symbols_found, 50);
    }

    #[test]
    fn test_checkpoint_result_builder() {
        let result = CheckpointResult::success("create")
            .with_checkpoint_id("cp-123")
            .with_session_id("session-456")
            .with_file_count(5);
        assert!(result.success);
        assert_eq!(result.checkpoint_id, Some("cp-123".to_string()));
        assert_eq!(result.file_count, Some(5));
    }

    #[test]
    fn test_modes_result_builder() {
        let result = ModesResult::success("list").with_modes(vec![ModeInfo {
            name: "code".to_string(),
            description: "Code mode".to_string(),
        }]);
        assert!(result.success);
        assert_eq!(result.modes.len(), 1);
    }

    #[test]
    fn test_telemetry_result() {
        let result = TelemetryResult::success(true, false, true, "/config.toml");
        assert!(result.success);
        assert!(result.metrics_enabled);
        assert!(!result.crash_reporting_enabled);
    }
}
