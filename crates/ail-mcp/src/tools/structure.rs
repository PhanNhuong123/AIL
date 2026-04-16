//! Handlers for `ail.move` and `ail.delete` MCP tools (Phase 11.2).
//!
//! These tools restructure or remove nodes in the graph through validated
//! operations. Like `ail.write`/`ail.patch`, both demote `ProjectContext` to
//! `Raw` and clear search caches at the dispatch boundary.

use std::collections::HashSet;

use ail_graph::{AilGraph, EdgeKind, GraphBackend, NodeId};

use crate::tools::write::{insert_at_position, parse_node_id};
use crate::types::tool_io::{DeleteInput, DeleteOutput, MoveInput, MoveOutput};

// ── Shared topology helpers ──────────────────────────────────────────────────

/// Splice `node_id` out of its current Eh sibling chain.
///
/// If `node_id` has both a previous and next sibling, link them directly.
/// Removes the existing `prev → node` and `node → next` Eh edges.
/// Returns `Ok(())` even when the node has no Eh neighbors.
fn detach_eh_chain(graph: &mut AilGraph, node_id: NodeId) -> Result<(), String> {
    let prev = GraphBackend::siblings_before(graph, node_id)
        .map_err(|e| format!("failed to read siblings_before: {e}"))?
        .last()
        .copied();
    let next = GraphBackend::siblings_after(graph, node_id)
        .map_err(|e| format!("failed to read siblings_after: {e}"))?
        .first()
        .copied();

    if let Some(p) = prev {
        // Best-effort — ignore if the edge was already gone.
        let _ = GraphBackend::remove_edge_by_kind(graph, p, node_id, EdgeKind::Eh);
    }
    if let Some(n) = next {
        let _ = GraphBackend::remove_edge_by_kind(graph, node_id, n, EdgeKind::Eh);
    }
    if let (Some(p), Some(n)) = (prev, next) {
        graph
            .add_edge(p, n, EdgeKind::Eh)
            .map_err(|e| format!("failed to relink Eh chain: {e}"))?;
    }
    Ok(())
}

/// Count distinct Ed edges incident to any node in `targets` (incoming or
/// outgoing). Each `(source, target)` pair is counted once.
fn count_incident_ed_edges(graph: &AilGraph, targets: &[NodeId]) -> usize {
    let target_set: HashSet<NodeId> = targets.iter().copied().collect();
    let mut counted: HashSet<(NodeId, NodeId)> = HashSet::new();

    // Outgoing Ed edges from any target.
    for &id in targets {
        if let Ok(out) = GraphBackend::outgoing_diagonal_refs(graph, id) {
            for dst in out {
                counted.insert((id, dst));
            }
        }
    }

    // Incoming Ed edges from sources outside the target set.
    if let Ok(all) = GraphBackend::all_node_ids(graph) {
        for src in all {
            if target_set.contains(&src) {
                continue;
            }
            if let Ok(out) = GraphBackend::outgoing_diagonal_refs(graph, src) {
                for dst in out {
                    if target_set.contains(&dst) {
                        counted.insert((src, dst));
                    }
                }
            }
        }
    }

    counted.len()
}

// ── ail.move ─────────────────────────────────────────────────────────────────

/// Move a node to a new parent and optional sibling position.
///
/// Validates: the node and new parent both exist, the node is not the root,
/// and the new parent is not the node itself or any of its descendants
/// (would create a cycle).
pub(crate) fn run_move(graph: &mut AilGraph, input: &MoveInput) -> Result<MoveOutput, String> {
    let mut warnings = Vec::new();

    // 1. Parse and verify both node and new parent exist.
    let node_id = parse_node_id(&input.node_id)?;
    let new_parent_id = parse_node_id(&input.new_parent_id)?;

    GraphBackend::get_node(graph, node_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("node not found: {}", input.node_id))?;
    GraphBackend::get_node(graph, new_parent_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("new parent not found: {}", input.new_parent_id))?;

    // 2. Reject moving a root (no Ev parent edge to remove from).
    let old_parent_id = GraphBackend::parent(graph, node_id).map_err(|e| e.to_string())?;
    let old_parent =
        old_parent_id.ok_or_else(|| "cannot move a root node — it has no parent".to_string())?;

    // 3. Reject self-move and cyclic moves.
    if new_parent_id == node_id {
        return Err("cannot move a node under itself".into());
    }
    let descendants = GraphBackend::all_descendants(graph, node_id).map_err(|e| e.to_string())?;
    if descendants.contains(&new_parent_id) {
        return Err("cannot move a node under one of its own descendants".into());
    }

    // 4. Capture pre-move depth.
    let old_depth = GraphBackend::depth(graph, node_id).map_err(|e| e.to_string())?;

    // 5. Splice the node out of its current Eh sibling chain.
    detach_eh_chain(graph, node_id)?;

    // 6. Re-wire Ev: remove old, add new.
    GraphBackend::remove_edge_by_kind(graph, old_parent, node_id, EdgeKind::Ev)
        .map_err(|e| format!("failed to remove old Ev edge: {e}"))?;
    graph
        .add_edge(new_parent_id, node_id, EdgeKind::Ev)
        .map_err(|e| format!("failed to add new Ev edge: {e}"))?;

    // 7. Wire into new parent's Eh chain at requested position.
    if let Err(e) = insert_at_position(graph, new_parent_id, node_id, input.position) {
        warnings.push(format!("Eh repositioning skipped: {e}"));
    }

    // 8. Depth is computed from the parent chain — recompute lazily.
    let new_depth = GraphBackend::depth(graph, node_id).map_err(|e| e.to_string())?;

    Ok(MoveOutput {
        status: "moved".into(),
        node_id: node_id.to_string(),
        old_parent_id: Some(old_parent.to_string()),
        new_parent_id: new_parent_id.to_string(),
        old_depth,
        new_depth,
        descendants_moved: descendants.len(),
        cic_invalidated: 0,
        warnings,
    })
}

// ── ail.delete ───────────────────────────────────────────────────────────────

/// Delete a node from the graph using the requested mutating strategy
/// (`cascade` or `orphan`). For `dry_run`, use [`run_delete_dry_run`] which
/// only borrows the graph immutably and does not demote the pipeline.
pub(crate) fn run_delete(
    graph: &mut AilGraph,
    input: &DeleteInput,
) -> Result<DeleteOutput, String> {
    let strategy = input.strategy.as_deref().unwrap_or("cascade");
    let node_id = parse_node_id(&input.node_id)?;

    GraphBackend::get_node(graph, node_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("node not found: {}", input.node_id))?;

    match strategy {
        "cascade" => delete_cascade(graph, node_id),
        "orphan" => delete_orphan(graph, node_id),
        "dry_run" => {
            Err("dry_run must be dispatched through run_delete_dry_run (immutable borrow)".into())
        }
        other => Err(format!(
            "invalid strategy: \"{other}\" (expected cascade, orphan, or dry_run)"
        )),
    }
}

/// Compute what a cascade delete *would* remove without mutating the graph.
pub(crate) fn run_delete_dry_run(
    graph: &AilGraph,
    input: &DeleteInput,
) -> Result<DeleteOutput, String> {
    let node_id = parse_node_id(&input.node_id)?;
    GraphBackend::get_node(graph, node_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("node not found: {}", input.node_id))?;
    delete_dry_run(graph, node_id)
}

fn delete_cascade(graph: &mut AilGraph, node_id: NodeId) -> Result<DeleteOutput, String> {
    let descendants = GraphBackend::all_descendants(graph, node_id).map_err(|e| e.to_string())?;
    let mut all_targets: Vec<NodeId> = Vec::with_capacity(descendants.len() + 1);
    all_targets.push(node_id);
    all_targets.extend(descendants.iter().copied());

    let affected_ed_edges = count_incident_ed_edges(graph, &all_targets);

    // Splice the target out of its sibling chain before removing.
    detach_eh_chain(graph, node_id)?;

    // Remove descendants first (deepest-last in BFS order means we reverse it).
    let mut deleted_node_ids = Vec::with_capacity(all_targets.len());
    for descendant in descendants.iter().rev() {
        if let Err(e) = GraphBackend::remove_node(graph, *descendant) {
            return Err(format!("failed to remove descendant {descendant}: {e}"));
        }
        deleted_node_ids.push(descendant.to_string());
    }
    GraphBackend::remove_node(graph, node_id)
        .map_err(|e| format!("failed to remove node {node_id}: {e}"))?;
    deleted_node_ids.push(node_id.to_string());

    Ok(DeleteOutput {
        status: "deleted".into(),
        deleted_nodes: deleted_node_ids.len(),
        deleted_node_ids,
        would_delete: 0,
        would_delete_ids: Vec::new(),
        reparented_children: 0,
        affected_ed_edges,
        cic_invalidated: 0,
        warnings: Vec::new(),
    })
}

fn delete_orphan(graph: &mut AilGraph, node_id: NodeId) -> Result<DeleteOutput, String> {
    let mut warnings = Vec::new();

    let parent_id = GraphBackend::parent(graph, node_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "cannot orphan a root node — it has no parent".to_string())?;

    // Children that will be lifted to parent.
    let children = GraphBackend::children(graph, node_id).map_err(|e| e.to_string())?;

    // Pre-compute affected Ed edges that the target itself participates in.
    let affected_ed_edges = count_incident_ed_edges(graph, &[node_id]);

    // Splice the target out of its parent's Eh sibling chain.
    detach_eh_chain(graph, node_id)?;

    // Reparent each child: drop Ev(target → child), add Ev(parent → child),
    // and append to parent's Eh chain. Any pre-existing Eh edges children
    // shared at the old level [11.2-A] are removed by `remove_node(target)`
    // when they involve the target; cross-level Eh edges are an upstream
    // graph defect and stay for graph validation to surface.
    for &child in &children {
        GraphBackend::remove_edge_by_kind(graph, node_id, child, EdgeKind::Ev)
            .map_err(|e| format!("failed to remove Ev to child {child}: {e}"))?;
        graph
            .add_edge(parent_id, child, EdgeKind::Ev)
            .map_err(|e| format!("failed to reparent {child}: {e}"))?;
        if let Err(e) = insert_at_position(graph, parent_id, child, None) {
            warnings.push(format!("Eh repositioning for {child} skipped: {e}"));
        }
    }
    if !children.is_empty() {
        warnings.push(format!(
            "orphan: {} child link(s) reparented to {}",
            children.len(),
            parent_id
        ));
    }

    // Remove the target. `remove_node` strips remaining incident edges.
    GraphBackend::remove_node(graph, node_id)
        .map_err(|e| format!("failed to remove node {node_id}: {e}"))?;

    Ok(DeleteOutput {
        status: "orphaned".into(),
        deleted_nodes: 1,
        deleted_node_ids: vec![node_id.to_string()],
        would_delete: 0,
        would_delete_ids: Vec::new(),
        reparented_children: children.len(),
        affected_ed_edges,
        cic_invalidated: 0,
        warnings,
    })
}

fn delete_dry_run(graph: &AilGraph, node_id: NodeId) -> Result<DeleteOutput, String> {
    let descendants = GraphBackend::all_descendants(graph, node_id).map_err(|e| e.to_string())?;
    let mut all_targets: Vec<NodeId> = Vec::with_capacity(descendants.len() + 1);
    all_targets.push(node_id);
    all_targets.extend(descendants.iter().copied());

    let affected_ed_edges = count_incident_ed_edges(graph, &all_targets);
    let would_delete_ids: Vec<String> = all_targets.iter().map(|id| id.to_string()).collect();

    Ok(DeleteOutput {
        status: "dry_run".into(),
        deleted_nodes: 0,
        deleted_node_ids: Vec::new(),
        would_delete: would_delete_ids.len(),
        would_delete_ids,
        reparented_children: 0,
        affected_ed_edges,
        cic_invalidated: 0,
        warnings: Vec::new(),
    })
}
