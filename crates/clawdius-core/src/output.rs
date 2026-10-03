//! Output formatting for Clawdius
//!
//! Supports text, JSON, and streaming JSON output formats.

// Unwrap purge batch 4: output module — clean-root deny guard (zero production unwrap/expect in the batch-4 survey).
// Production code must not unwrap/expect; propagate, document a true invariant with an INVARIANT comment, or restructure.
// Lints inherit into all child modules and tests.
#![deny(clippy::unwrap_used, clippy::expect_used)]
// Test builds keep unwrap/expect for brevity (fleet convention, see lib.rs).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

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
