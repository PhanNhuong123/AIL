/// Errors produced by `ail-graph` operations.
///
/// This enum grows with each phase 1 subtask (1.2 CRUD, 1.6 validation, etc.).
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
