//! `ail status` — show the highest pipeline stage reached and node/edge counts.

use std::path::Path;

use ail_graph::{validation::validate_graph, Pattern};
use ail_text::parse_directory;
use ail_types::type_check;

use crate::error::CliError;

/// Entry point for `ail status`.
///
/// Runs each pipeline stage in order and stops at the first failure. Prints
/// the highest stage reached along with graph statistics.
pub fn run_status(root: &Path) -> Result<(), CliError> {
    // ── Stage 1: parse ───────────────────────────────────────────────────────
    let graph = match parse_directory(root) {
        Ok(g) => g,
        Err(e) => {
            println!("Stage: parse-failed | {e}");
            return Ok(());
        }
    };

    let node_count = graph.node_count();
    let edge_count = graph.edge_count();
    let do_count = count_do_nodes(&graph);

    // ── Stage 2: validate ────────────────────────────────────────────────────
    let valid = match validate_graph(graph) {
        Ok(v) => v,
        Err(errs) => {
            println!(
                "Stage: invalid | nodes: {node_count} | edges: {edge_count} | \
                 do-nodes: {do_count} | first error: {}",
                errs.first().map(|e| e.to_string()).unwrap_or_default()
            );
            return Ok(());
        }
    };

    // ── Stage 3: type-check ──────────────────────────────────────────────────
    let typed = match type_check(valid, &[]) {
        Ok(t) => t,
        Err(errs) => {
            println!(
                "Stage: type-error | nodes: {node_count} | edges: {edge_count} | \
                 do-nodes: {do_count} | first error: {}",
                errs.first().map(|e| e.to_string()).unwrap_or_default()
            );
            return Ok(());
        }
    };

    // ── Stage 4: contract verification ──────────────────────────────────────
    match ail_contract::verify(typed) {
        Ok(_) => {
            println!(
                "Stage: verified | nodes: {node_count} | edges: {edge_count} | \
                 do-nodes: {do_count}"
            );
        }
        Err(errs) => {
            println!(
                "Stage: contract-error | nodes: {node_count} | edges: {edge_count} | \
                 do-nodes: {do_count} | first error: {}",
                errs.first().map(|e| e.to_string()).unwrap_or_default()
            );
        }
    }

    Ok(())
}

/// Count `Do`-pattern nodes in the graph.
fn count_do_nodes(graph: &ail_graph::AilGraph) -> usize {
    graph
        .all_nodes()
        .filter(|n| n.pattern == Pattern::Do)
        .count()
}
