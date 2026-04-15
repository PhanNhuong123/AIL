use crate::types::{EdgeId, NodeId};

/// Errors produced by `ail-graph` operations.
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("node not found: {0}")]
    NodeNotFound(NodeId),

    #[error("duplicate node id: {0}")]
    DuplicateNodeId(NodeId),

    #[error("edge not found: {0:?}")]
    EdgeNotFound(EdgeId),

    #[error("edge not found: {from} → {to} (kind: {kind:?})")]
    EdgeKindNotFound {
        from: NodeId,
        to: NodeId,
        kind: crate::types::EdgeKind,
    },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("name not found: '{0}' is not declared in any ancestor scope")]
    NameNotFound(String),

    #[error("ambiguous name: '{name}' found at multiple locations: {locations:?}")]
    AmbiguousName {
        name: String,
        locations: Vec<String>,
    },

    /// A storage backend (e.g. SQLite) reported an error.
    #[error("storage backend error: {0}")]
    Storage(String),

    /// A node id string could not be parsed as a UUID.
    #[error("invalid node id '{0}': {1}")]
    InvalidNodeId(String, String),
}
