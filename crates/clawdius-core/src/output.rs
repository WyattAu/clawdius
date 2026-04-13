//! Output formatting for Clawdius
//!
//! Supports text, JSON, and streaming JSON output formats.

pub mod format;
pub mod formatter;
#[cfg(test)]
mod json_tests;
pub mod stream;

pub use format::{
    ActionEdit, ActionResult, ChangeType, CheckpointInfo, CheckpointResult, ConfigResult,
    ContextFile, ContextResult, ContextSymbol, FileChange, FileVersionInfo, IndexResult,
    InitResult, JsonOutput, MetricsResult, ModeDetails, ModeInfo, ModesResult, OutputFormat,
    OutputOptions, ProofError, RefactorFileChange, RefactorResult, TelemetryResult, TestCaseInfo,
    TestResult, TimelineResult, TokenUsageInfo, ToolCallInfo, VerifyResult,
};
pub use formatter::{OutputFormatter, SessionInfo};
pub use stream::{StreamEvent, StreamWriter};
