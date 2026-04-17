use ail_graph::{GraphError, NodeId};
use ail_search::SearchError;

/// Errors produced by `ail-coverage` operations.
///
/// Error codes follow the `AIL-C0xx` range (cross-phase convention [X-4]).
#[derive(Debug, thiserror::Error)]
pub enum CoverageError {
    /// AIL-C001 — The requested node does not exist in the graph.
    #[error("AIL-C001: node not found: {0}")]
    NodeNotFound(NodeId),

    /// AIL-C002 — Graph backend error propagated from `ail-graph`.
    #[error("AIL-C002: graph backend error: {0}")]
    Graph(#[from] GraphError),

    /// AIL-C003 — Embedding provider error propagated from `ail-search`.
    #[error("AIL-C003: embedding provider error: {0}")]
    Embedding(#[from] SearchError),

    /// AIL-C004 — The embedding provider reported dimension 0.
    #[error("AIL-C004: embedding provider returned dimension 0")]
    ZeroDimension,

    /// AIL-C005 — The provider returned a vector of unexpected length.
    #[error(
        "AIL-C005: provider returned vector of unexpected length (expected {expected}, got {actual})"
    )]
    DimensionMismatch { expected: usize, actual: usize },
}
