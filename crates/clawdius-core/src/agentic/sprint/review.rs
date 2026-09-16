// Unwrap purge batch 1: execution-surface module — production code must not
// unwrap/expect; propagate, use invariant-expect with a written INVARIANT
// argument, or restructure.
#![deny(clippy::unwrap_used, clippy::expect_used)]
// Test builds keep unwrap/expect for brevity (fleet convention, see lib.rs).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use super::engine::SprintEngine;
use super::{PhaseResult, PhaseStatus, SprintPhase, SprintState};
use crate::agentic::review_engine::{ReviewEngine, ReviewerConfig};
use crate::Result;

pub async fn run_multi_model_review(
    engine: &SprintEngine,
    state: &SprintState,
    llm_result: PhaseResult,
) -> Result<PhaseResult> {
    let code_to_review = &state.context_accumulator;
    let context = &state.config.task_description;

    let review_engine = ReviewEngine::new(state.config.reviewers.clone());
    let fused = review_engine.review(code_to_review, context).await?;

    let review_output = format!(
        "[Multi-Model Review — {} reviewers, avg score: {:.1}/5]\n\n\
         {}\n\n\
         {}",
        fused.reviews.len(),
        fused.average_score,
        fused.summary,
        if fused.has_critical_issues {
            "⚠️ CRITICAL issues found. Address before proceeding."
        } else {
            "No critical issues."
        }
    );

    Ok(PhaseResult {
        phase: SprintPhase::Review,
        status: if fused.has_critical_issues {
            PhaseStatus::Success
        } else {
            PhaseStatus::Success
        },
        output: review_output,
        duration_ms: fused.total_duration_ms,
        files_modified: Vec::new(),
        errors: if fused.has_critical_issues {
            vec!["Critical issues found in review".to_string()]
        } else {
            Vec::new()
        },
        tokens_used: fused.total_tokens,
    })
}
