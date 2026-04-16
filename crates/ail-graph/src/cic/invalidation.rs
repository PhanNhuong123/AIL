//! Cache invalidation scope for check-based promoted facts (Phase 8.2).
//!
//! When a `Check` node is modified, removed, or inserted, every context packet
//! that received a promoted fact from it is stale. This module computes that
//! scope by mirroring the read path in `collect_promoted_facts`
//! (`compute_backend.rs`): at each ancestor level of the check node, all
//! subsequent siblings and all descendants of those siblings.
//!
//! The read path walks the full root-to-current ancestor path and at each
//! level looks at previous siblings, recursing into `Do` bodies. The
//! invalidation must therefore walk **up** from the check through every
//! ancestor, collecting forward siblings + descendants at each level.
//!
//! ## Sequencing contract
//!
//! [`check_promotion_affected_nodes`] must be called **before** mutating the
//! graph (i.e. while the check node still exists). If the node has already been
//! removed, the function returns `Err(GraphError::NodeNotFound)`. Callers that
//! need the affected set for a post-removal scenario should snapshot it first,
//! then perform the mutation.

use std::collections::HashSet;

use crate::errors::GraphError;
use crate::graph::GraphBackend;
use crate::types::NodeId;

/// Return every [`NodeId`] whose context packet must be recomputed when
/// `check_id` is added, changed, or removed.
///
/// Starting from `check_id`, walks up through every ancestor. At each level
/// the scope includes every sibling that follows the current node in execution
/// order (via [`GraphBackend::siblings_after`]) plus every descendant of each
/// such sibling (via [`GraphBackend::all_descendants`]). This mirrors the CIC
/// read path which walks the full ancestor chain and at each level collects
/// promoted facts from previous siblings (recursing into `Do` bodies).
///
/// Nodes that precede the check in execution order are not affected because
/// promoted facts only flow forward.
///
/// ## Caller contract
///
/// - **Pattern**: `check_id` should identify a [`crate::types::Pattern::Check`]
///   node. The function does not validate the pattern; passing a non-`Check`
///   node returns the sibling descendants of that node at every ancestor level,
///   which may cause unnecessary cache invalidation but will not corrupt graph
///   state.
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
    graph
        .get_node(check_id)?
        .ok_or(GraphError::NodeNotFound(check_id))?;

    let mut affected = Vec::new();
    let mut seen = HashSet::new();

    // Walk from check_id up through every ancestor level.
    let mut cursor = check_id;
    loop {
        let siblings = graph.siblings_after(cursor)?;
        for sib in siblings {
            if seen.insert(sib) {
                affected.push(sib);
                for desc in graph.all_descendants(sib)? {
                    if seen.insert(desc) {
                        affected.push(desc);
                    }
                }
            }
        }
        match graph.parent(cursor)? {
            Some(parent_id) => cursor = parent_id,
            None => break,
        }
    }

    Ok(affected)
}
