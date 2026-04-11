use serde::{Deserialize, Serialize};

use crate::types::{Contract, Expression, NodeId, NodeMetadata, Pattern};

/// A single node in the PSSD graph.
///
/// Invariants enforced by `ValidGraph` (not here):
/// - `intent` must be non-empty.
/// - Structural nodes (`children.is_some()`) must have `expression == None`.
/// - Leaf nodes (`children.is_none()`) may carry an `expression`.
/// - `Do` nodes must have at least one `Before` and one `After` contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    /// Human-readable English description of what this node does.
    pub intent: String,
    /// Which of the 17 AIL patterns this node represents.
    pub pattern: Pattern,
    /// Child node IDs connected by Ev edges. `None` means this is a leaf.
    pub children: Option<Vec<NodeId>>,
    /// Raw expression text — present only on leaf nodes.
    pub expression: Option<Expression>,
    /// Promises attached to this node (`promise before / after / always`).
    pub contracts: Vec<Contract>,
    /// Pattern-specific symbolic metadata (name, params, fields, etc.).
    pub metadata: NodeMetadata,
}

impl Node {
    /// Create a minimal node with a fresh ID and empty contracts/metadata.
    /// Set `children = Some(vec![])` for a structural node or `None` for a leaf.
    pub fn new(id: NodeId, intent: impl Into<String>, pattern: Pattern) -> Self {
        Self {
            id,
            intent: intent.into(),
            pattern,
            children: None,
            expression: None,
            contracts: vec![],
            metadata: NodeMetadata::default(),
        }
    }
}
