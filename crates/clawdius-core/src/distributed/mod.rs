//! # Distributed LLM Routing Module
//!
//! Multi-node LLM routing with consensus-based coordination.
//!
//! ## Architecture
//!
//! This module provides the scaffolding for distributing LLM requests across a
//! cluster of nodes. It is designed around three core components:
//!
//! **Router** ([`router`]) — Selects which node handles each request using
//! pluggable strategies: round-robin, least-connections, or latency-aware.
//!
//! **Consensus** ([`consensus`]) — Ensures cluster-wide agreement on leader
//! identity via heartbeat-based election. Uses a simplified Raft-inspired
//! protocol with `LeaderElection`, `LogEntry`, and `AppendEntries` types.
//!
//! **Node** ([`node`]) — Represents a single cluster member with health
//! tracking, load metrics, and lifecycle management.
//!
//! ## Feature Gate
//!
//! The module source is always compiled so that tests run without the feature.
//! Public re-exports in `clawdius_core` are gated behind `cfg(feature =
//! "distributed")`:
//!
//! ```toml
//! clawdius-core = { version = "...", features = ["distributed"] }
//! ```
//!
//! ## Status
//!
//! **v1.2.0 — PLANNED.** This is architectural scaffolding only.

pub mod consensus;
pub mod node;
pub mod router;
