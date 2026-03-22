//! Code analysis module for Clawdius
//!
//! This module provides architecture analysis tools including:
//! - **Drift Detection**: Detects deviations from intended architecture
//! - **Technical Debt**: Quantifies and prioritizes technical debt

pub mod debt;
pub mod drift;

pub use debt::{DebtAnalyzer, DebtItem, DebtReport, DebtRule, DebtType};
pub use drift::{
    ArchitectureDrift, DriftCategory, DriftDetector, DriftReport, DriftRule, DriftSeverity,
};

/// Common result type for analysis operations.
pub type AnalysisResult<T> = std::result::Result<T, AnalysisError>;

/// Errors that can occur during analysis.
#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    /// File not found
    #[error("File not found: {0}")]
    FileNotFound(String),

    /// Parse error
    #[error("Parse error in {file}: {message}")]
    ParseError {
        /// File that failed to parse
        file: String,
        /// Error message
        message: String,
    },

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Pattern matching error
    #[error("Pattern error: {0}")]
    PatternError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_error_display() {
        let err = AnalysisError::FileNotFound("test.rs".to_string());
        assert!(err.to_string().contains("test.rs"));

        let err = AnalysisError::ParseError {
            file: "foo.rs".to_string(),
            message: "syntax error".to_string(),
        };
        assert!(err.to_string().contains("foo.rs"));
    }
}
