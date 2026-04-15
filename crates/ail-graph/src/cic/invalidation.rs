//! Cache invalidation scope for check-based promoted facts (Phase 8.2).
//!
//! When a `Check` node is modified, removed, or inserted, every context packet
//! that received a promoted fact from it is stale. This module computes that
//! scope: all subsequent siblings of the check node and all descendants of
//! those siblings.
//!
//! ## Sequencing contract
//!
//! [`check_promotion_affected_nodes`] must be called **before** mutating the
//! graph (i.e. while the check node still exists). If the node has already been
//! removed, the function returns `Err(GraphError::NodeNotFound)`. Callers that
//! need the affected set for a post-removal scenario should snapshot it first,
//! then perform the mutation.

use crate::errors::GraphError;
use crate::graph::GraphBackend;
use crate::types::NodeId;

/// Return every [`NodeId`] whose context packet must be recomputed when
/// `check_id` is added, changed, or removed.
///
/// The scope is every sibling that follows `check_id` in execution order
/// (obtained via [`GraphBackend::siblings_after`]) plus every descendant of
/// each such sibling (obtained via [`GraphBackend::all_descendants`]). Nodes
/// that precede the check in execution order are not affected because promoted
/// facts only flow forward.
///
/// ## Caller contract
///
/// - **Pattern**: `check_id` should identify a [`crate::types::Pattern::Check`]
///   node. The function does not validate the pattern; passing a non-`Check`
///   node returns the sibling descendants of that node, which may cause
///   unnecessary cache invalidation but will not corrupt graph state.
/// - **Timing**: call this *before* the mutation while the node is still in
///   the graph. See the module-level sequencing contract above.
///
/// ## Errors
///
/// Returns [`GraphError::NodeNotFound`] if `check_id` is not in the graph.
pub fn check_promotion_affected_nodes(
    graph: &dyn GraphBackend,
    check_id: NodeId,
) -> Result<Vec<NodeId>, GraphError> {
    let siblings = graph.siblings_after(check_id)?;
    let mut affected = Vec::new();
    for sib in siblings {
        affected.push(sib);
        let descendants = graph.all_descendants(sib)?;
        affected.extend(descendants);
    }
    Ok(affected)
}
