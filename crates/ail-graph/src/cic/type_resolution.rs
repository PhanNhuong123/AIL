use std::collections::HashSet;

use crate::graph::AilGraph;
use crate::types::{NodeId, Pattern};

use super::PacketConstraint;

/// Scan the graph for a type-defining node whose `metadata.name` matches.
///
/// Searches `Define`, `Describe`, and `Error` patterns. Returns the first
/// match in graph iteration order, or `None` if no such node exists.
///
/// This is a **flat** lookup — no scoping. Phase 1 does not yet implement
/// local → parent → root name resolution (that is task 1.5).
pub(super) fn find_type_node_by_name(graph: &AilGraph, name: &str) -> Option<NodeId> {
    // The inner() accessor is crate-private; we use it here since this file
    // lives in the same crate as `ail_graph.rs`.
    for node_index in graph.inner().node_indices() {
        let node = graph.inner().node_weight(node_index)?;
        let is_type_node = matches!(
            node.pattern,
            Pattern::Define | Pattern::Describe | Pattern::Error
        );
        if !is_type_node {
            continue;
        }
        if node.metadata.name.as_deref() == Some(name) {
            return Some(node.id);
        }
    }
    None
}

/// Walk a type node and collect every constraint that applies to values of
/// that type, recursively unfolding record fields.
///
/// Semantics:
/// - A `Define` node contributes all of its own contracts (e.g. the `where`
///   clause on a scalar type).
/// - A `Describe` node contributes its own contracts **and** recursively
///   unfolds each field's type. This is how chains like
///   `User.balance → WalletBalance → value >= 0` are materialised.
/// - An `Error` node contributes its own contracts.
///
/// Cycle guard: `visited` tracks the set of type-node ids already walked so
/// self-referential types (a `User` with a `friends: List[User]` field, for
/// example) do not loop forever. The guard is keyed on the type node id, so
/// each type is walked at most once per top-level call.
///
/// Unresolvable field types are silently skipped — validation and type
/// checking are owned by later phases.
pub(super) fn unfold_type_constraints(
    graph: &AilGraph,
    type_node_id: NodeId,
    visited: &mut HashSet<NodeId>,
) -> Vec<PacketConstraint> {
    if !visited.insert(type_node_id) {
        // already walked this type on the current stack
        return Vec::new();
    }

    let Ok(type_node) = graph.get_node(type_node_id) else {
        return Vec::new();
    };

    let mut out: Vec<PacketConstraint> = type_node
        .contracts
        .iter()
        .map(|c| PacketConstraint::from_contract(type_node_id, c))
        .collect();

    if matches!(type_node.pattern, Pattern::Describe) {
        for field in &type_node.metadata.fields {
            if let Some(field_type_id) = find_type_node_by_name(graph, &field.type_ref) {
                let nested = unfold_type_constraints(graph, field_type_id, visited);
                out.extend(nested);
            }
        }
    }

    out
}
