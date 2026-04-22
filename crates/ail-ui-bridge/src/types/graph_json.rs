use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::node_detail::NodeDetail;
use super::status::Status;

/// Alias for `VerifyFailureJson` in its role as a project-level issue.
///
/// Used in `GraphJson.issues` to drive the TitleBar "⚠ N issues" pill and the
/// verify-lens banner. The `severity` and `source` fields on `VerifyFailureJson`
/// carry issue-level metadata.
pub type IssueJson = crate::types::verify_result::VerifyFailureJson;

/// Top-level JSON shape for a serialized `VerifiedGraph`.
///
/// Matches the `GraphJson` interface in `AIL-Tauri-IDE-v4.0.md`.
/// Uses `BTreeMap` for `detail` to ensure deterministic serialization order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphJson {
    pub project: ProjectJson,
    pub clusters: Vec<ClusterJson>,
    pub modules: Vec<ModuleJson>,
    pub externals: Vec<ExternalJson>,
    pub relations: Vec<RelationJson>,
    pub types: Vec<TypeRefJson>,
    pub errors: Vec<ErrorRefJson>,
    /// Project-level issues (verification failures, rule violations, etc.)
    /// that feed the TitleBar pill and verify-lens banner.
    pub issues: Vec<IssueJson>,
    pub detail: BTreeMap<String, NodeDetail>,
}

/// Summary metrics for the project as a whole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectJson {
    pub id: String,
    pub name: String,
    pub description: String,
    pub node_count: usize,
    pub module_count: usize,
    pub fn_count: usize,
    pub status: Status,
}

/// A cluster groups related modules for visual organisation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterJson {
    pub id: String,
    pub name: String,
    pub color: String,
}

/// A module corresponds to a level-1 child of the project root.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleJson {
    pub id: String,
    pub name: String,
    pub description: String,
    pub cluster: String,
    pub cluster_name: String,
    pub cluster_color: String,
    pub status: Status,
    pub node_count: usize,
    pub functions: Vec<FunctionJson>,
}

/// A function corresponds to a `Do` node inside a module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionJson {
    pub id: String,
    pub name: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<StepJson>>,
}

/// A step is a leaf child of a `Do` function node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepJson {
    pub id: String,
    pub name: String,
    pub status: Status,
    pub intent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

/// A cross-module or cross-function relation (derived from `Ed` edges).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationJson {
    pub from: String,
    pub to: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
}

/// A type definition referenced from within the project graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeRefJson {
    pub id: String,
    pub name: String,
    pub status: Status,
}

/// An error type defined in the project graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorRefJson {
    pub id: String,
    pub name: String,
    pub status: Status,
}

/// An external dependency (outside the project graph).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalJson {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
