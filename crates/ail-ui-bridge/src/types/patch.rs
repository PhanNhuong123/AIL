use serde::{Deserialize, Serialize};

use super::graph_json::{FunctionJson, ModuleJson, StepJson};

/// An entry for an added or modified function, carrying the parent module id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionPatchEntry {
    /// The id of the module that contains this function.
    pub module_id: String,
    /// The full function payload.
    pub function: FunctionJson,
}

/// A removal record for a function, carrying both the parent module id and the
/// function's own id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionRemoval {
    /// The id of the module that contained this function.
    pub module_id: String,
    /// The id of the removed function.
    pub function_id: String,
}

/// An entry for an added or modified step, carrying the parent function id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepPatchEntry {
    /// The id of the function that contains this step.
    pub function_id: String,
    /// The full step payload.
    pub step: StepJson,
}

/// A removal record for a step, carrying both the parent function id and the
/// step's own id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepRemoval {
    /// The id of the function that contained this step.
    pub function_id: String,
    /// The id of the removed step.
    pub step_id: String,
}

/// Fine-grained incremental graph patch — carries only deltas per constraint
/// 16.1-C.
///
/// The nine delta arrays cover module, function, and step granularity.
/// Each added/modified entry carries its parent id so the frontend can place
/// it without a full re-scan. Removed entries carry both parent and self ids.
/// There is no `Full` variant — patches must never trigger a full reload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphPatchJson {
    pub modules_added: Vec<ModuleJson>,
    pub modules_modified: Vec<ModuleJson>,
    /// Path IDs of removed modules.
    pub modules_removed: Vec<String>,
    pub functions_added: Vec<FunctionPatchEntry>,
    pub functions_modified: Vec<FunctionPatchEntry>,
    pub functions_removed: Vec<FunctionRemoval>,
    pub steps_added: Vec<StepPatchEntry>,
    pub steps_modified: Vec<StepPatchEntry>,
    pub steps_removed: Vec<StepRemoval>,
    pub timestamp: u64,
}
