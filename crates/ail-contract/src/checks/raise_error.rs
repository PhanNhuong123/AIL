use ail_graph::graph::GraphBackend;
use ail_graph::types::{Expression, Node, NodeId, Pattern};

use crate::errors::ContractError;

/// Check that a `Raise` node's expression names an error declared by the
/// enclosing `Do` function.
///
/// The enclosing function is found by walking up `parent_of` edges until a
/// `Pattern::Do` node is reached. The function's declared errors are the
/// `Pattern::Error` nodes reachable via outgoing `Ed` edges from that `Do` node.
pub(crate) fn check_raise_error_refs(
    graph: &dyn GraphBackend,
    node: &Node,
    errors: &mut Vec<ContractError>,
) {
    let error_name = match node.expression.as_ref().and_then(extract_error_name) {
        Some(name) => name,
        None => return, // no expression — nothing to check
    };

    let enclosing_do_id = match find_enclosing_do(graph, node.id) {
        Some(id) => id,
        None => return, // no enclosing Do — skip (structural validator would catch this)
    };

    let known_errors = collect_declared_errors(graph, enclosing_do_id);

    if !known_errors.contains(&error_name) {
        errors.push(ContractError::RaiseUnknownError {
            node_id: node.id,
            error_name,
            known_errors,
        });
    }
}

// ─── helpers ──────────────────────────────────────────────────────────────

/// Extract the error type name from a Raise expression.
///
/// A raise expression may be:
/// - `"InsufficientFunds"` — just the name
/// - `"InsufficientFunds carries current_balance = sender.balance"` — name + payload
///
/// The error name is always the first whitespace-delimited token.
fn extract_error_name(expr: &Expression) -> Option<String> {
    expr.0
        .split_whitespace()
        .next()
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// Walk up the `parent_of` chain from `start_id` until a `Do` node is found.
/// Returns `None` if no enclosing `Do` exists.
fn find_enclosing_do(graph: &dyn GraphBackend, start_id: NodeId) -> Option<NodeId> {
    let mut cursor = start_id;
    loop {
        match graph.parent(cursor) {
            Ok(Some(parent_id)) => {
                cursor = parent_id;
                // AilGraph: never errors; SqliteGraph: degrade gracefully on missing node.
                if let Some(parent) = graph.get_node(parent_id).ok().flatten() {
                    if parent.pattern == Pattern::Do {
                        return Some(parent_id);
                    }
                }
            }
            _ => return None,
        }
    }
}

/// Collect the names of all `Error` nodes reachable via outgoing `Ed` edges
/// from the `Do` node at `do_id`.
fn collect_declared_errors(graph: &dyn GraphBackend, do_id: NodeId) -> Vec<String> {
    graph
        .outgoing_diagonal_refs(do_id)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|target_id| {
            // AilGraph: never errors; SqliteGraph: degrade gracefully on missing node.
            let target = graph.get_node(target_id).ok().flatten()?;
            if target.pattern == Pattern::Error {
                target.metadata.name.clone()
            } else {
                None
            }
        })
        .collect()
}
