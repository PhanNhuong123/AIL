//! `ail context` — print the CIC context packet for a task or named node.
//!
//! Second and later calls for the same node hit the SQLite `cic_cache` table,
//! which is the primary acceptance signal for Phase 12 Task 12.1.
//!
//! Target selection:
//! - `--node <name>`: look up a node whose `metadata.name` or `intent` matches.
//!   Takes precedence over `--task`.
//! - `--task <text>`: BM25 search over the project, pick the top `Do` hit.

use std::path::Path;

use ail_db::SqliteGraph;
use ail_graph::graph::GraphBackend;
use ail_graph::types::{NodeId, Pattern};

use crate::commands::project::{resolve_backend, ProjectBackend};
use crate::error::CliError;

/// Entry point for `ail context`.
pub fn run_context(
    root: &Path,
    task: Option<&str>,
    node: Option<&str>,
    from_db: Option<&Path>,
) -> Result<(), CliError> {
    let backend = resolve_backend(root, from_db)?;
    let db_path = match backend {
        ProjectBackend::Sqlite { db_path } => db_path,
        ProjectBackend::Filesystem { .. } => {
            return Err(CliError::Pipeline {
                errors: "`ail context` requires the SQLite backend. Run \
                    `ail migrate --from src/ --to project.ail.db --verify` first, \
                    or pass --from-db <path>."
                    .to_owned(),
            });
        }
    };

    let db = SqliteGraph::open(&db_path).map_err(|e| CliError::Pipeline {
        errors: format!("open {}: {e}", db_path.display()),
    })?;

    let target_id = resolve_target(&db, task, node)?;

    let cache_was_valid = db.cic_cache_valid(target_id).unwrap_or(false);
    let packet = db
        .get_context_packet(target_id)
        .map_err(|e| CliError::Pipeline {
            errors: format!("get_context_packet: {e}"),
        })?;

    let node_row = db
        .get_node(target_id)
        .map_err(|e| CliError::Pipeline {
            errors: format!("get_node: {e}"),
        })?
        .ok_or_else(|| CliError::Pipeline {
            errors: format!("node {target_id} not found"),
        })?;

    let label = node_row
        .metadata
        .name
        .clone()
        .unwrap_or_else(|| node_row.intent.clone());

    println!(
        "Context packet for `{label}` ({pattern:?}) [{status}]",
        pattern = node_row.pattern,
        status = if cache_was_valid {
            "cache hit"
        } else {
            "cache miss (stored)"
        }
    );

    let json = serde_json::to_string_pretty(&packet).map_err(|e| CliError::Pipeline {
        errors: format!("serialize packet: {e}"),
    })?;
    println!("{json}");

    Ok(())
}

/// Resolve the target node for `ail context`.
///
/// `--node` is tried first. If neither flag is given, returns an error.
fn resolve_target(
    db: &SqliteGraph,
    task: Option<&str>,
    node_name: Option<&str>,
) -> Result<NodeId, CliError> {
    if let Some(name) = node_name {
        return find_by_name(db, name);
    }
    if let Some(q) = task {
        return find_by_task(db, q);
    }
    Err(CliError::Pipeline {
        errors: "pass either --node <name> or --task <text>".to_owned(),
    })
}

fn find_by_name(db: &SqliteGraph, name: &str) -> Result<NodeId, CliError> {
    let ids = db.all_node_ids().map_err(|e| CliError::Pipeline {
        errors: format!("all_node_ids: {e}"),
    })?;

    let mut best: Option<NodeId> = None;
    let mut best_is_do = false;

    for id in ids {
        let node = match db.get_node(id).map_err(|e| CliError::Pipeline {
            errors: format!("get_node: {e}"),
        })? {
            Some(n) => n,
            None => continue,
        };

        let matches_name = node
            .metadata
            .name
            .as_deref()
            .map(|n| n.eq_ignore_ascii_case(name))
            .unwrap_or(false);
        let matches_intent = node.intent.eq_ignore_ascii_case(name);

        if !matches_name && !matches_intent {
            continue;
        }

        let is_do = node.pattern == Pattern::Do;
        if best.is_none() || (!best_is_do && is_do) {
            best = Some(id);
            best_is_do = is_do;
        }
    }

    best.ok_or_else(|| CliError::Pipeline {
        errors: format!("no node matches `{name}`"),
    })
}

fn find_by_task(db: &SqliteGraph, query: &str) -> Result<NodeId, CliError> {
    let hits = db.search(query, 32).map_err(|e| CliError::Pipeline {
        errors: format!("search `{query}`: {e}"),
    })?;

    if let Some(r) = hits.iter().find(|r| r.pattern == Pattern::Do) {
        return Ok(r.node_id);
    }
    if let Some(r) = hits.first() {
        return Ok(r.node_id);
    }
    Err(CliError::Pipeline {
        errors: format!("no search hits for `{query}`"),
    })
}
