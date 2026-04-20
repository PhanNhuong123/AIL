use serde::Serialize;
use thiserror::Error;

/// Structured error type for the UI bridge.
///
/// Derives `Serialize` with `#[serde(tag = "code", content = "detail")]` so
/// Tauri can surface structured error codes to the SvelteKit frontend.
#[derive(Error, Debug, Serialize)]
#[serde(tag = "code", content = "detail")]
pub enum BridgeError {
    /// The specified project path does not exist or is not a directory.
    #[error("project not found: {path}")]
    ProjectNotFound { path: String },

    /// A pipeline stage (`parse`, `validate`, `type_check`, or `verify`) failed.
    #[error("pipeline error at stage '{stage}': {detail}")]
    PipelineError { stage: String, detail: String },

    /// A node ID referenced in a request was not found in the graph.
    #[error("node not found: {id}")]
    NodeNotFound { id: String },

    /// Invalid input from the caller (bad path, malformed ID, etc.).
    #[error("invalid input: {reason}")]
    InvalidInput { reason: String },
}
