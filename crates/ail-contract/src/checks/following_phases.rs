use std::collections::HashSet;

use ail_graph::graph::GraphBackend;
use ail_graph::types::{Node, NodeId};

use crate::errors::ContractError;

/// Check that a `Do` node implementing a template provides all required phases.
///
/// Templates are identified by `metadata.following_template_name` — set by the
/// parser from the `following <name>` clause. Ed edges are not used here
/// because a `Do` may also emit Ed edges for type references, raised errors,
/// and function calls (see MCP auto-edge labels `uses_type`/`raises`/`calls`),
/// which would falsely trip this check if interpreted as template refs.
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

    let template_name = match &node.metadata.following_template_name {
        Some(name) => name,
        None => return,
    };

    let template_ids: Vec<NodeId> = graph.find_by_name(template_name).unwrap_or_default();
    if template_ids.is_empty() {
        return;
    }

    let implemented_phases: HashSet<String> = collect_named_children(graph, node.id);

    for template_id in template_ids {
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
