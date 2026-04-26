//! DTOs for Phase 17.4 sheaf analysis events.
//!
//! These types are serialized to the frontend over the `sheaf-complete` Tauri
//! event. Wire format is camelCase JSON matching the TypeScript consumer.

use serde::{Deserialize, Serialize};

/// One contradictory conflict pair from a Čech nerve overlap.
///
/// Both `node_a` and `node_b` are path-like step IDs (e.g.
/// `"wallet_service.src.transfer.s1_validate"`) matching the `step.id` values
/// in `GraphJson`. Translated from raw `NodeId` UUIDs by `IdMap::get_path` at
/// projection time so the frontend can match conflicts to graph nodes directly.
/// `conflicting_a` and `conflicting_b` are the minimized UNSAT-core constraints
/// attributed to each side (Phase 17.2 invariant 17.2-A).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheafConflictEntry {
    pub overlap_index: usize,
    pub node_a: String,
    pub node_b: String,
    pub conflicting_a: Vec<String>,
    pub conflicting_b: Vec<String>,
}

/// Payload emitted on the `sheaf-complete` Tauri event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheafCompletePayload {
    pub run_id: String,
    pub ok: bool,
    pub z3_available: bool,
    pub conflicts: Vec<SheafConflictEntry>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub cancelled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Result returned by `cancel_sheaf_analysis`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheafCancelResult {
    pub cancelled: bool,
}
