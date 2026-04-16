//! `ail status` — show the highest pipeline stage reached and node/edge counts.

use std::path::{Path, PathBuf};

use ail_db::{EmbeddingModelStatus, SqliteGraph};
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

    // ── Embedding index health (optional — requires a .ail.db file) ──────────
    if let Some(line) = embedding_status_line(root) {
        println!("{line}");
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

// ─── Embedding index health ───────────────────────────────────────────────────

/// Locate an `.ail.db` file in `root`.
///
/// Checks `project.ail.db` first (conventional name), then scans the directory
/// for any `*.ail.db` file and returns the first match.
fn find_db(root: &Path) -> Option<PathBuf> {
    let conventional = root.join("project.ail.db");
    if conventional.exists() {
        return Some(conventional);
    }
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("db")
            && path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.ends_with(".ail"))
                .unwrap_or(false)
        {
            return Some(path);
        }
    }
    None
}

/// Build the embedding health line for `ail status`, or `None` if no DB exists.
///
/// The model string `"onnx/all-MiniLM-L6-v2"` is the default local provider
/// name (from `OnnxEmbeddingProvider::name()`). We use the literal here to avoid
/// taking a compile-time dependency on the `embeddings` feature of `ail-search`.
fn embedding_status_line(root: &Path) -> Option<String> {
    let db_path = find_db(root)?;
    let db = SqliteGraph::open(&db_path).ok()?;
    let count = db.embedding_count().ok()?;
    let model = "onnx/all-MiniLM-L6-v2";
    let status = db.check_embedding_model(model).ok()?;
    Some(match status {
        EmbeddingModelStatus::Empty => {
            "embedding: none | run 'ail search --setup' to enable".to_string()
        }
        EmbeddingModelStatus::Compatible => {
            format!("embedding: {count} vectors | model: {model} | status: ok")
        }
        EmbeddingModelStatus::Changed { stored } => {
            format!(
                "embedding: stale | stored model: {stored} | \
                 run 'ail reindex' to rebuild"
            )
        }
    })
}
