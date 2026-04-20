use serde::{Deserialize, Serialize};

use super::graph_json::{FunctionJson, ModuleJson};

/// An incremental graph patch — carries only deltas per constraint 16.1-C.
/// No `Full` variant exists; patches must never trigger a full reload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphPatchJson {
    pub added: Vec<PatchItem>,
    pub modified: Vec<PatchItem>,
    /// Path IDs of removed nodes.
    pub removed: Vec<String>,
    pub timestamp: u64,
}

/// A single item inside an incremental patch.
///
/// Internally tagged with `"kind"` so the frontend can dispatch by type.
/// Variants correspond to the module and function granularities exposed in
/// `GraphJson`. There is deliberately no `Full` variant (constraint 16.1-C)
/// and no `Step` variant — step-level patching is out of scope for Phase 16.1.
/// Step-level diffing will be added in a later task once the MCP write path
/// exposes step-level mutations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum PatchItem {
    /// A module-level node was added or modified.
    Module(ModuleJson),
    /// A function-level node was added or modified.
    Function(FunctionJson),
}
