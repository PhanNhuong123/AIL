//! Tauri event name constants for backend → frontend notifications.
//!
//! Each constant names a Tauri event. The expected payload type is noted in
//! the doc comment. Payload structs live in [`crate::types`].

/// Emitted when the graph changes incrementally.
///
/// Payload: [`crate::types::GraphPatchJson`]
pub const GRAPH_UPDATED: &str = "graph-updated";

/// Emitted when a full verification pass completes.
///
/// Payload: [`crate::types::VerifyResultJson`]
pub const VERIFY_COMPLETE: &str = "verify-complete";

/// Emitted when a coverage computation pass completes.
///
/// Payload: coverage result (Phase 17+).
pub const COVERAGE_COMPLETE: &str = "coverage-complete";

/// Emitted for each step of an AI agent run.
///
/// Payload: agent step (Phase 17+).
pub const AGENT_STEP: &str = "agent-step";
