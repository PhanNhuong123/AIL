use crate::errors::GraphError;
use crate::graph::AilGraph;
use crate::types::NodeId;

use super::compute_backend::compute_context_packet_for_backend;
use super::ContextPacket;

impl AilGraph {
    /// Compute the [`ContextPacket`] for a node.
    ///
    /// Delegates to [`compute_context_packet_for_backend`], which is the
    /// canonical backend-agnostic implementation. Both in-memory and SQLite
    /// backends therefore share the same algorithm, preventing drift.
    ///
    /// The function is **pure** (borrows `&self` only), **deterministic**
    /// (same graph + same node id always produce the same packet), and
    /// therefore safely **cacheable** by callers. Callers who need memoisation
    /// should wrap the graph in an external cache keyed by `NodeId`.
    ///
    /// Errors:
    /// - [`GraphError::NodeNotFound`] if `node_id` is unknown.
    ///
    /// Unresolved type references and dangling Ed targets are silently
    /// ignored — those are validation / type-check concerns, not CIC
    /// concerns.
    pub fn compute_context_packet(&self, node_id: NodeId) -> Result<ContextPacket, GraphError> {
        compute_context_packet_for_backend(self, node_id)
    }
}
