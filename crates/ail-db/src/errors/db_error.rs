use ail_graph::{errors::GraphError, types::NodeId};

/// Errors produced by `ail-db` internal operations.
///
/// Every `DbError` converts to a [`GraphError`] so that `impl GraphBackend`
/// methods can use `?` throughout without exposing rusqlite to callers.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// A UUID string read from the database could not be parsed as a `NodeId`.
    #[error("invalid UUID: {0}")]
    InvalidUuid(String),

    /// A node ID was referenced but the row is absent from the `nodes` table.
    #[error("node not found: {0}")]
    NodeNotFound(NodeId),

    /// Catch-all for logic errors that don't fit a specific variant.
    #[error("database error: {0}")]
    Other(String),
}

impl From<DbError> for GraphError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::Sqlite(e) => GraphError::Storage(e.to_string()),
            DbError::Serialization(e) => GraphError::Serialization(e),
            DbError::InvalidUuid(msg) => GraphError::InvalidNodeId("(from DB)".to_string(), msg),
            DbError::NodeNotFound(id) => GraphError::NodeNotFound(id),
            DbError::Other(msg) => GraphError::Storage(msg),
        }
    }
}
