use std::fmt::Write;

use ail_graph::{AilGraph, ContractKind, NodeId, Pattern};

use super::node_text::render_node_text;

// ── Top-level node collection ───────────────────────────────────────────────

/// Collect top-level node IDs in deterministic order.
///
/// If the graph has a root:
///   - If root is a directory container (Describe with name=None), return its children.
///   - Otherwise, return the root itself.
///
/// If no root, find all parentless nodes and order them by the Eh sibling chain
/// (deterministic: head node has no prev_sibling, follow next_sibling to end).
pub(crate) fn collect_top_level_ids(graph: &AilGraph) -> Vec<NodeId> {
    if let Some(root_id) = graph.root_id() {
        if is_container_describe(graph, root_id) {
            return ordered_children(graph, root_id);
        }
        return vec![root_id];
    }

    // No root — find parentless nodes and order by Eh chain.
    let parentless: Vec<NodeId> = graph
        .node_ids()
        .filter(|&id| graph.parent_of(id).ok().flatten().is_none())
        .collect();

    order_by_eh_chain(graph, &parentless)
}

/// Order a set of node IDs by following the Eh sibling chain.
/// Finds the head (no prev_sibling within the set), then follows next_sibling.
fn order_by_eh_chain(graph: &AilGraph, ids: &[NodeId]) -> Vec<NodeId> {
    if ids.is_empty() {
        return vec![];
    }
    if ids.len() == 1 {
        return ids.to_vec();
    }

    let id_set: std::collections::HashSet<NodeId> = ids.iter().copied().collect();

    // Find head: the node in the set with no prev_sibling in the set.
    let head = ids
        .iter()
        .copied()
        .find(|&id| {
            graph
                .prev_sibling_of(id)
                .ok()
                .flatten()
                .is_none_or(|prev| !id_set.contains(&prev))
        })
        .unwrap_or(ids[0]);

    let mut ordered = vec![head];
    let mut current = head;
    while let Ok(Some(next)) = graph.next_sibling_of(current) {
        if id_set.contains(&next) {
            ordered.push(next);
            current = next;
        } else {
            break;
        }
    }

    ordered
}

/// Get children of a node in their stored order (from node.children Vec).
fn ordered_children(graph: &AilGraph, parent_id: NodeId) -> Vec<NodeId> {
    graph
        .get_node(parent_id)
        .ok()
        .and_then(|n| n.children.clone())
        .unwrap_or_default()
}

/// Check if a node is a directory container (Describe with name=None).
fn is_container_describe(graph: &AilGraph, node_id: NodeId) -> bool {
    graph
        .get_node(node_id)
        .ok()
        .is_some_and(|n| n.pattern == Pattern::Describe && n.metadata.name.is_none())
}

// ── Recursive node rendering ────────────────────────────────────────────────

/// Render a single node and its subtree at the given indent level and depth.
///
/// - `indent`: current indentation level (0 = no indent, 1 = 2 spaces, etc.)
/// - `depth`: remaining levels to expand (0 = no children, usize::MAX = all)
pub(crate) fn render_node(
    graph: &AilGraph,
    node_id: NodeId,
    indent: usize,
    depth: usize,
    out: &mut String,
) {
    let node = match graph.get_node(node_id) {
        Ok(n) => n,
        Err(_) => return,
    };

    // Directory container: skip the node, render children at the same level.
    if node.pattern == Pattern::Describe && node.metadata.name.is_none() {
        let children = ordered_children(graph, node_id);
        for (i, &child_id) in children.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            render_node(graph, child_id, indent, depth, out);
        }
        return;
    }

    // Render the node's own text.
    let text = render_node_text(node);
    if text.is_empty() {
        return;
    }
    append_indented(out, &text, indent);
    out.push('\n');

    if depth == 0 {
        return;
    }

    let child_indent = indent + 1;
    let child_depth = depth.saturating_sub(1);

    // ── Do node: contracts + children ───────────────────────────────────
    if node.pattern == Pattern::Do {
        let has_contracts = !node.contracts.is_empty();
        let children = ordered_children(graph, node_id);

        if has_contracts {
            out.push('\n'); // blank line before contracts

            // Sort contracts: Before → After → Always for deterministic output.
            let mut sorted_contracts = node.contracts.clone();
            sorted_contracts.sort_by_key(|c| match c.kind {
                ContractKind::Before => 0,
                ContractKind::After => 1,
                ContractKind::Always => 2,
            });

            for contract in &sorted_contracts {
                let kind = match contract.kind {
                    ContractKind::Before => "before",
                    ContractKind::After => "after",
                    ContractKind::Always => "always",
                };
                let line = format!("promise {kind}: {}", contract.expression.0);
                append_indented(out, &line, child_indent);
                out.push('\n');
            }
        }

        if !children.is_empty() {
            if has_contracts {
                out.push('\n'); // blank line between contracts and children
            }
            render_children(graph, &children, child_indent, child_depth, out);
        }
        return;
    }

    // ── Together/Retry: inline children ─────────────────────────────────
    if node.pattern == Pattern::Together || node.pattern == Pattern::Retry {
        let children = ordered_children(graph, node_id);
        if !children.is_empty() {
            render_children(graph, &children, child_indent, child_depth, out);
        }
        return;
    }

    // ── Generic children (ForEach, etc.) ────────────────────────────────
    let children = ordered_children(graph, node_id);
    if !children.is_empty() {
        render_children(graph, &children, child_indent, child_depth, out);
    }
}

/// Render a list of child nodes sequentially.
fn render_children(
    graph: &AilGraph,
    children: &[NodeId],
    indent: usize,
    depth: usize,
    out: &mut String,
) {
    for &child_id in children {
        render_node(graph, child_id, indent, depth, out);
    }
}

// ── Indentation helper ──────────────────────────────────────────────────────

/// Append `text` to `out`, prepending `indent * 2` spaces to each line.
fn append_indented(out: &mut String, text: &str, indent: usize) {
    let prefix = "  ".repeat(indent);
    for (i, line) in text.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let _ = write!(out, "{prefix}{line}");
    }
}
