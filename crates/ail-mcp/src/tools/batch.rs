//! Handler for the `ail.batch` MCP tool (Phase 11.3).
//!
//! Runs an ordered list of graph mutations (`write`, `patch`, `move`,
//! `delete`) as a single atomic operation. A snapshot of the in-memory
//! `AilGraph` is taken before the first op; if any op fails, the snapshot is
//! restored and the batch is reported as rolled back. After the whole batch
//! succeeds, auto-edge detection is re-run across every affected node so Ed
//! references reflect the final graph state.

use std::collections::HashSet;

use ail_graph::{AilGraph, EdgeKind, GraphBackend, NodeId};

use crate::tools::structure::{run_delete, run_move};
use crate::tools::write::{detect_auto_edges, parse_node_id, run_patch, run_write};
use crate::types::tool_io::{
    BatchInput, BatchOperation, BatchOperationResult, BatchOutput, DeleteInput,
};

/// Execute a batch of mutating operations on `graph`.
///
/// On success, returns a [`BatchOutput`] with per-operation results and a
/// count of Ed edges added/removed by the post-batch refresh pass. On the
/// first operation failure, the graph is restored from its pre-batch snapshot
/// and the batch status becomes `"rolled_back"`. The `results` field still
/// carries the successful ops up to the failure point plus an error entry for
/// the failing op; subsequent ops are dropped because they never ran.
pub(crate) fn run_batch(graph: &mut AilGraph, input: &BatchInput) -> BatchOutput {
    // Empty batch: no-op, no snapshot.
    if input.operations.is_empty() {
        return BatchOutput {
            status: "completed".into(),
            results: Vec::new(),
            auto_edges_refreshed: 0,
            total_cic_invalidated: 0,
            error: None,
        };
    }

    // Snapshot before the first mutation — used to restore the graph verbatim
    // if any operation fails.
    let snapshot = graph.clone();

    let mut results: Vec<BatchOperationResult> = Vec::with_capacity(input.operations.len());
    let mut affected: Vec<NodeId> = Vec::new();
    let mut deleted: HashSet<NodeId> = HashSet::new();

    for (idx, operation) in input.operations.iter().enumerate() {
        let outcome = apply_operation(graph, operation, &mut affected, &mut deleted);
        match outcome {
            Ok(result) => results.push(result),
            Err((op_name, err)) => {
                results.push(BatchOperationResult {
                    op: op_name.clone(),
                    status: "error".into(),
                    output: None,
                    error: Some(err.clone()),
                });
                // Restore the pre-batch graph state.
                *graph = snapshot;
                return BatchOutput {
                    status: "rolled_back".into(),
                    results,
                    auto_edges_refreshed: 0,
                    total_cic_invalidated: 0,
                    error: Some(format!("op #{idx} ({op_name}): {err}")),
                };
            }
        }
    }

    // Post-batch: refresh auto-edges for every node that was written, patched,
    // or moved and still exists in the graph.
    let auto_edges_refreshed = refresh_auto_edges_for(graph, &affected, &deleted);

    BatchOutput {
        status: "completed".into(),
        results,
        auto_edges_refreshed,
        total_cic_invalidated: 0,
        error: None,
    }
}

/// Dispatch a single operation to the matching tool handler.
///
/// Tracks affected node IDs for the post-batch auto-edge refresh pass:
/// writes/patches/moves record the touched node; deletes record every
/// removed id so auto-edge refresh skips them.
fn apply_operation(
    graph: &mut AilGraph,
    op: &BatchOperation,
    affected: &mut Vec<NodeId>,
    deleted: &mut HashSet<NodeId>,
) -> Result<BatchOperationResult, (String, String)> {
    match op {
        BatchOperation::Write(input) => {
            let out = run_write(graph, input).map_err(|e| ("write".into(), e))?;
            if let Ok(id) = parse_node_id(&out.node_id) {
                affected.push(id);
            }
            let output = serde_json::to_value(&out)
                .map_err(|e| ("write".into(), format!("serialize write output: {e}")))?;
            Ok(BatchOperationResult {
                op: "write".into(),
                status: "ok".into(),
                output: Some(output),
                error: None,
            })
        }
        BatchOperation::Patch(input) => {
            let out = run_patch(graph, input).map_err(|e| ("patch".into(), e))?;
            if let Ok(id) = parse_node_id(&out.node_id) {
                affected.push(id);
            }
            let output = serde_json::to_value(&out)
                .map_err(|e| ("patch".into(), format!("serialize patch output: {e}")))?;
            Ok(BatchOperationResult {
                op: "patch".into(),
                status: "ok".into(),
                output: Some(output),
                error: None,
            })
        }
        BatchOperation::Move(input) => {
            let out = run_move(graph, input).map_err(|e| ("move".into(), e))?;
            if let Ok(id) = parse_node_id(&out.node_id) {
                affected.push(id);
            }
            let output = serde_json::to_value(&out)
                .map_err(|e| ("move".into(), format!("serialize move output: {e}")))?;
            Ok(BatchOperationResult {
                op: "move".into(),
                status: "ok".into(),
                output: Some(output),
                error: None,
            })
        }
        BatchOperation::Delete(input) => {
            reject_dry_run(input).map_err(|e| ("delete".into(), e))?;
            let out = run_delete(graph, input).map_err(|e| ("delete".into(), e))?;
            for id_str in &out.deleted_node_ids {
                if let Ok(id) = parse_node_id(id_str) {
                    deleted.insert(id);
                }
            }
            let output = serde_json::to_value(&out)
                .map_err(|e| ("delete".into(), format!("serialize delete output: {e}")))?;
            Ok(BatchOperationResult {
                op: "delete".into(),
                status: "ok".into(),
                output: Some(output),
                error: None,
            })
        }
    }
}

/// Refuse `dry_run` inside a batch: a batch must be mutating. Dry-run is a
/// standalone preview and does not belong in an atomic edit sequence.
fn reject_dry_run(input: &DeleteInput) -> Result<(), String> {
    if input.strategy.as_deref() == Some("dry_run") {
        Err("dry_run is not allowed inside ail.batch; use ail.delete directly".into())
    } else {
        Ok(())
    }
}

/// Re-detect auto edges for every affected node and reconcile the graph's
/// outgoing Ed edges against the detection result. Returns the number of Ed
/// edges added or removed across all nodes.
///
/// Skips any node that was deleted during the batch. De-duplicates the
/// affected list so a node referenced by multiple ops is refreshed once.
fn refresh_auto_edges_for(
    graph: &mut AilGraph,
    affected: &[NodeId],
    deleted: &HashSet<NodeId>,
) -> usize {
    let mut seen: HashSet<NodeId> = HashSet::new();
    let mut changes = 0usize;

    for node_id in affected {
        if deleted.contains(node_id) || !seen.insert(*node_id) {
            continue;
        }
        // Confirm the node still exists in the graph — a later delete without
        // matching bookkeeping shouldn't leave a dangling refresh.
        match GraphBackend::get_node(graph, *node_id) {
            Ok(Some(_)) => {}
            _ => continue,
        }

        changes += reconcile_node_edges(graph, *node_id);
    }

    changes
}

/// Bring `node_id`'s outgoing Ed edges in sync with `detect_auto_edges`.
///
/// Removes currently-outgoing Ed edges that the detector no longer proposes
/// and adds any newly-detected edges. Returns the number of edges changed
/// (removed + added).
fn reconcile_node_edges(graph: &mut AilGraph, node_id: NodeId) -> usize {
    let existing: HashSet<NodeId> = GraphBackend::outgoing_diagonal_refs(graph, node_id)
        .unwrap_or_default()
        .into_iter()
        .collect();

    let detected = detect_auto_edges(graph, node_id);
    let mut desired: HashSet<NodeId> = HashSet::new();
    for edge in &detected {
        if let Ok(target) = parse_node_id(&edge.target) {
            desired.insert(target);
        }
    }

    let mut changes = 0usize;

    // Remove edges that no longer apply.
    for target in existing.difference(&desired) {
        if GraphBackend::remove_edge_by_kind(graph, node_id, *target, EdgeKind::Ed).is_ok() {
            changes += 1;
        }
    }
    // Add edges that the detector now proposes.
    for target in desired.difference(&existing) {
        if graph.add_edge(node_id, *target, EdgeKind::Ed).is_ok() {
            changes += 1;
        }
    }

    changes
}
