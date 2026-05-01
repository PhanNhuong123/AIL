//! DTOs for the `scaffold_project` Tauri command (Phase 18 follow-up to v4.0
//! review: closes finding **N2**).
//!
//! `scaffold_project` writes a minimal `.ail` skeleton to disk so the Welcome
//! / Quick Create flow can hand a real project path to `load_project`.
//! Wire format is camelCase JSON matching the TypeScript consumer.

use serde::{Deserialize, Serialize};

/// Caller-supplied request for `scaffold_project`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectScaffoldRequest {
    /// Absolute parent directory chosen by the user. The new project folder
    /// is created as `<parent_dir>/<name>`.
    pub parent_dir: String,
    /// Quick-Create kind: `"module"`, `"function"`, `"rule"`, or `"test"`.
    /// Drives the skeleton template.
    pub kind: String,
    /// Project / node name. Must be a valid identifier (alphanumeric + `_`).
    pub name: String,
    /// Optional human description seeded into the skeleton's contract block.
    #[serde(default)]
    pub description: String,
}

/// Result of a successful scaffold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectScaffoldResult {
    /// Absolute path of the newly created project directory.
    pub project_dir: String,
    /// Absolute path of the primary `.ail` file inside the project.
    pub ail_file: String,
}
