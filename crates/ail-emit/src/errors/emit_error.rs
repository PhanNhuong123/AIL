use ail_graph::NodeId;

/// Errors that can occur during Python code emission.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EmitError {
    #[error("AIL-E001: Define node {node_id} has no name in metadata")]
    DefineNodeMissingName { node_id: NodeId },

    #[error("AIL-E002: Define node {node_id} has no base_type in metadata")]
    DefineNodeMissingBaseType { node_id: NodeId },

    #[error("AIL-E003: Describe node {node_id} has no name in metadata")]
    DescribeNodeMissingName { node_id: NodeId },

    #[error("AIL-E004: Error node {node_id} has no name in metadata")]
    ErrorNodeMissingName { node_id: NodeId },

    #[error("AIL-E005: constraint parse error on node {node_id}: {message}")]
    ConstraintParseError {
        node_id: NodeId,
        expression: String,
        message: String,
    },

    #[error("AIL-E006: Do node {node_id} has no name in metadata")]
    DoNodeMissingName { node_id: NodeId },

    #[error("AIL-E007: Fetch node {node_id} has no variable name in metadata")]
    FetchNodeMissingName { node_id: NodeId },

    #[error("AIL-E008: Save node {node_id} has no name in metadata")]
    SaveNodeMissingName { node_id: NodeId },

    #[error("AIL-E009: Return node {node_id} has no type name in metadata")]
    ReturnNodeMissingName { node_id: NodeId },

    #[error("AIL-E010: Raise node {node_id} has no error name in metadata")]
    RaiseNodeMissingName { node_id: NodeId },
}
