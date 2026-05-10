#![allow(
    dead_code,
    missing_docs,
    unused_variables,
    clippy::cast_precision_loss,
    clippy::clone_on_copy,
    clippy::doc_lazy_continuation,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::format_collect,
    clippy::items_after_statements,
    clippy::manual_is_multiple_of,
    clippy::manual_range_contains,
    clippy::missing_const_for_fn,
    clippy::must_use_candidate,
    clippy::needless_return,
    clippy::panic,
    clippy::redundant_clone,
    clippy::return_self_not_must_use,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    clippy::unwrap_used
)]

pub mod agentic_llm;
pub mod diff_workflow;
pub mod features;
pub mod rpc_communication;
pub mod search_workflow;
pub mod session_flow;
pub mod tool_execution;
