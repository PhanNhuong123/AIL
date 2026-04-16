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

    #[error("AIL-E011: Do node {node_id} has using_pattern_name but no outgoing Ed edge (assembler must resolve the reference before emission)")]
    UsingDoMissingEdge { node_id: NodeId },

    #[error("AIL-E012: Do node {node_id} references using pattern '{pattern_name}' which cannot be found in the graph")]
    UsingDoUnresolvedPattern {
        node_id: NodeId,
        pattern_name: String,
    },

    #[error("AIL-E013: Do node {node_id} is missing required template phase '{phase}' at emit time (should have been caught by v008 validation)")]
    MissingTemplatePhase { node_id: NodeId, phase: String },

    #[error("AIL-E014: old() reference in non-after contract on node {node_id}: old() is only valid in promise-after expressions")]
    OldRefInNonAfterContract { node_id: NodeId },

    // ── TypeScript emitter errors (AIL-E015–E019) ─────────────────────────────
    #[error("AIL-E015: TS Define node {node_id} has no name in metadata")]
    TsDefineNodeMissingName { node_id: NodeId },

    #[error("AIL-E016: TS Define node {node_id} has no base_type in metadata")]
    TsDefineNodeMissingBaseType { node_id: NodeId },

    #[error("AIL-E017: TS Describe node {node_id} has no name in metadata")]
    TsDescribeNodeMissingName { node_id: NodeId },

    #[error("AIL-E018: TS Error node {node_id} has no name in metadata")]
    TsErrorNodeMissingName { node_id: NodeId },

    #[error("AIL-E019: TS constraint parse error on node {node_id}: {message}")]
    TsConstraintParseError {
        node_id: NodeId,
        expression: String,
        message: String,
    },

    // ── TypeScript function emitter errors (AIL-E020–E024) ────────────────────
    #[error("AIL-E020: TS Do node {node_id} has no name in metadata")]
    TsDoNodeMissingName { node_id: NodeId },

    #[error("AIL-E021: TS Fetch node {node_id} has no variable name in metadata")]
    TsFetchNodeMissingName { node_id: NodeId },

    #[error("AIL-E022: TS Save node {node_id} has no name in metadata")]
    TsSaveNodeMissingName { node_id: NodeId },

    #[error("AIL-E023: TS Return node {node_id} has no type name in metadata")]
    TsReturnNodeMissingName { node_id: NodeId },

    #[error("AIL-E024: TS Raise node {node_id} has no error name in metadata")]
    TsRaiseNodeMissingName { node_id: NodeId },
}
