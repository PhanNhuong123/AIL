use serde::{Deserialize, Serialize};

use crate::types::{ContractKind, Expression, NodeId};

/// A single constraint carried inside a [`super::ContextPacket`].
///
/// This is a thin wrapper over the graph's [`crate::types::Contract`] with one
/// additional field, [`PacketConstraint::origin_node`], recording where the
/// constraint was originally declared. Provenance is kept because a single
/// packet merges constraints from many sources (ancestors, sibling types,
/// called functions) and downstream phases — especially Phase 3 contract
/// verification — need to know which node each obligation came from.
///
/// The constraint body is stored as raw [`Expression`] text. The AST form
/// lives in the Phase 2 `ail-types` crate and is intentionally not referenced
/// here, so `ail-graph` stays constraint-AST-agnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacketConstraint {
    /// Node that originally declared the underlying contract.
    pub origin_node: NodeId,
    /// Whether the constraint is a precondition, postcondition, or invariant.
    pub kind: ContractKind,
    /// Raw expression text — parsed and evaluated in later phases.
    pub expression: Expression,
}

impl PacketConstraint {
    /// Build a constraint from a graph [`crate::types::Contract`] plus the
    /// id of the node on which that contract was declared.
    pub fn from_contract(origin_node: NodeId, contract: &crate::types::Contract) -> Self {
        Self {
            origin_node,
            kind: contract.kind.clone(),
            expression: contract.expression.clone(),
        }
    }
}
