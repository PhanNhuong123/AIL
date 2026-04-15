use std::collections::HashSet;

use ail_graph::graph::GraphBackend;
use ail_graph::types::{Node, NodeId, Pattern};

use crate::errors::ContractError;

/// Check that a `Do` node implementing a template provides all required phases.
///
/// **Template vs function call distinction**: Ed edges FROM a `Do` node itself
/// (not its children) identify template following. Ed edges from child action
/// nodes (Let, Fetch, etc.) to another `Do` identify function calls made
/// within the body. Using `outgoing_diagonal_refs_of(do_node.id)` here
/// correctly isolates template references from in-body function calls.
///
/// A template's required phases are the named children of the template `Do`
/// node. The implementing `Do` must have a named child for each one.
pub(crate) fn check_following_template_phases(
    graph: &dyn GraphBackend,
    node: &Node,
    errors: &mut Vec<ContractError>,
) {
    // using-Do nodes reference a shared pattern via Ed for CIC propagation but
    // do NOT implement template phases — skip them to avoid false positives.
    if node.metadata.using_pattern_name.is_some() {
        return;
    }

    let template_refs: Vec<NodeId> = graph
        .outgoing_diagonal_refs(node.id)
        .unwrap_or_default()
        .into_iter()
        .filter(|&target_id| {
            // AilGraph: never errors; SqliteGraph: degrade gracefully on missing node.
            graph
                .get_node(target_id)
                .ok()
                .flatten()
                .map(|t| t.pattern == Pattern::Do)
                .unwrap_or(false)
        })
        .collect();

    if template_refs.is_empty() {
        return;
    }

    let implemented_phases: HashSet<String> = collect_named_children(graph, node.id);

    for template_id in template_refs {
        // AilGraph: never errors; SqliteGraph: degrade gracefully on missing node.
        let Some(template_node) = graph.get_node(template_id).ok().flatten() else {
            continue;
        };

        let template_name = template_node
            .metadata
            .name
            .clone()
            .unwrap_or_else(|| template_id.to_string());

        let required_phases = collect_named_children(graph, template_id);

        for phase in required_phases {
            if !implemented_phases.contains(&phase) {
                errors.push(ContractError::FollowingMissingPhase {
                    node_id: node.id,
                    template_name: template_name.clone(),
                    missing_phase: phase,
                });
            }
        }
    }
}

// ─── helper ───────────────────────────────────────────────────────────────

/// Return the `metadata.name` values of all named children of `node_id`.
fn collect_named_children(graph: &dyn GraphBackend, node_id: NodeId) -> HashSet<String> {
    graph
        .children(node_id)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|child_id| {
            // AilGraph: never errors; SqliteGraph: degrade gracefully on missing node.
            graph
                .get_node(child_id)
                .ok()
                .flatten()
                .and_then(|c| c.metadata.name.clone())
        })
        .collect()
}
