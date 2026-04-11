use crate::types::NodeId;

/// Errors produced by `validate_graph`. Each variant corresponds to one validation rule.
/// `validate_graph` accumulates all errors rather than stopping at the first one.
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    // ─── Structural prerequisites ──────────────────────────────────────────
    #[error("graph has no root set")]
    MissingRoot,

    // ─── v001: non-empty intent ────────────────────────────────────────────
    #[error("node {node_id} has an empty intent")]
    EmptyIntent { node_id: NodeId },

    // ─── v002: Ev edges form a tree ────────────────────────────────────────
    #[error("Ev edges form a cycle involving nodes: {cycle:?}")]
    EvCycleDetected { cycle: Vec<NodeId> },

    #[error("node {node_id} has multiple Ev parents: {parents:?}")]
    EvMultipleParents {
        node_id: NodeId,
        parents: Vec<NodeId>,
    },

    // ─── v003: all nodes reachable from root ───────────────────────────────
    #[error("node {node_id} is unreachable from root")]
    UnreachableNode { node_id: NodeId },

    // ─── v004: only leaves carry expressions ──────────────────────────────
    #[error("structural node {node_id} (children.is_some()) carries an expression")]
    ExpressionOnStructuralNode { node_id: NodeId },

    // ─── v005: top-level Do has pre + post contracts ───────────────────────
    #[error("top-level Do node {node_id} is missing a 'before' contract")]
    MissingPreContract { node_id: NodeId },

    #[error("top-level Do node {node_id} is missing an 'after' contract")]
    MissingPostContract { node_id: NodeId },

    // ─── v006: type references resolve ────────────────────────────────────
    #[error("node {node_id} references unresolved type '{type_ref}'")]
    UnresolvedTypeReference { node_id: NodeId, type_ref: String },

    // ─── v007: no duplicate names in scope ────────────────────────────────
    #[error("duplicate name '{name}' in scope under {scope_id}: used by {node_ids:?}")]
    DuplicateNameInScope {
        name: String,
        scope_id: NodeId,
        node_ids: Vec<NodeId>,
    },

    // ─── v008: following template phases ──────────────────────────────────
    #[error("Do node {node_id} is missing required template phase '{phase}'")]
    MissingTemplatePhase { node_id: NodeId, phase: String },
}
