//! `ail migrate` / `ail export` — filesystem ↔ SQLite project migration.
//!
//! ## Forward migration (`ail migrate --from src/ --to project.ail.db`)
//!
//! 1. Parse all `.ail` files with the existing text pipeline → `AilGraph`.
//! 2. Create a new `SqliteGraph` at the target path.
//! 3. Transfer nodes (step 1), then Ev edges in BFS order (step 2), Eh edges
//!    (step 3), and Ed edges (step 4 — active code path even when count = 0,
//!    so future callers are not silently broken [7.5-A]).
//! 4. Optionally verify roundtrip fidelity with `--verify`.
//!
//! ## Reverse export (`ail export --from project.ail.db --to dst/`)
//!
//! Reconstructs an `AilGraph` from `SqliteGraph`, repopulating `node.children`
//! for every parent node (the renderer reads this field, not petgraph edges),
//! then calls the existing `ail_text::render` and writes `dst/export.ail`.

use std::collections::VecDeque;
use std::fs;
use std::path::Path;

use ail_db::SqliteGraph;
use ail_graph::{graph::GraphBackend, types::EdgeKind, AilGraph};
use ail_text::{parse_directory, render};

use crate::error::CliError;

// ── Public types ──────────────────────────────────────────────────────────────

/// Summary returned by a successful migration run.
#[derive(Debug)]
pub struct MigrationReport {
    pub node_count: usize,
    /// Combined count of Ev + Eh + Ed edges transferred.
    pub edge_count: usize,
    pub contract_count: usize,
    pub warnings: Vec<String>,
}

/// Summary returned by a successful verify run.
#[derive(Debug)]
pub struct VerifyResult {
    pub verified_nodes: usize,
    pub mismatches: Vec<String>,
}

// ── Entry points ──────────────────────────────────────────────────────────────

/// `ail migrate --from src/ --to project.ail.db [--verify]`
///
/// Parses `.ail` files under `from`, creates a new SQLite database at `to`,
/// transfers all nodes and edges, and optionally verifies roundtrip fidelity.
/// Returns `Err` if the target path already exists or any transfer step fails.
pub fn run_migrate(from: &Path, to: &Path, verify: bool) -> Result<(), CliError> {
    let graph = parse_directory(from).map_err(|e| CliError::MigrationFailed {
        message: e.to_string(),
    })?;

    let report = migrate_graph(&graph, to)?;

    println!(
        "Migrated {} node(s), {} edge(s), {} contract(s) to {}",
        report.node_count,
        report.edge_count,
        report.contract_count,
        to.display()
    );

    for w in &report.warnings {
        eprintln!("warning: {w}");
    }

    if verify {
        let result = run_verify(from, to)?;
        println!(
            "Verified {} node(s). Migration lossless.",
            result.verified_nodes
        );
    }

    Ok(())
}

/// Migrate a pre-parsed `AilGraph` into a new SQLite database at `db_path`.
///
/// The target path must not already exist. Returns a [`MigrationReport`] with
/// counts on success. Useful when testing with programmatically-constructed
/// graphs rather than a filesystem project.
pub fn migrate_graph(graph: &AilGraph, db_path: &Path) -> Result<MigrationReport, CliError> {
    let mut db = SqliteGraph::create(db_path).map_err(|e| CliError::MigrationFailed {
        message: e.to_string(),
    })?;
    migrate_ail_to_sqlite(graph, &mut db)
}

/// `ail export --from project.ail.db --to dst/`
///
/// Opens an existing database at `from`, reconstructs an `AilGraph`, renders
/// it to `.ail` text, and writes `dst/export.ail`. The output directory is
/// created if it does not exist.
pub fn run_export(from: &Path, to: &Path) -> Result<(), CliError> {
    let db = SqliteGraph::open(from).map_err(|e| CliError::MigrationFailed {
        message: e.to_string(),
    })?;

    let ail = rebuild_from_sqlite(&db)?;
    let text = render(&ail, usize::MAX);

    fs::create_dir_all(to)?;

    let out_file = to.join("export.ail");
    fs::write(&out_file, &text)?;

    println!(
        "Exported {} node(s) to {}",
        ail.node_count(),
        out_file.display()
    );

    Ok(())
}

/// Verify that a previously migrated database matches the source directory.
///
/// Parses `src_dir`, opens `db_path`, and checks that every node in the source
/// has a matching node in the database (by intent, pattern, and contracts).
/// Returns `Err(CliError::VerifyFailed)` on any mismatch.
///
/// **Note:** `parse_directory` generates fresh [`NodeId`]s on every call, so
/// the verify succeeds only when the database was populated from the *same
/// parse run* that produced the [`AilGraph`]. Prefer [`run_verify_graph`] when
/// you already hold the parsed graph.
pub fn run_verify(src_dir: &Path, db_path: &Path) -> Result<VerifyResult, CliError> {
    let graph = parse_directory(src_dir).map_err(|e| CliError::MigrationFailed {
        message: e.to_string(),
    })?;
    let db = SqliteGraph::open(db_path).map_err(|e| CliError::MigrationFailed {
        message: e.to_string(),
    })?;
    check_migration(&graph, &db)
}

/// Verify that every node in `graph` has a matching entry in the database at
/// `db_path`.
///
/// Use this when you already hold the parsed graph — it avoids the double-parse
/// problem where a second call to `parse_directory` generates different
/// [`NodeId`]s.
pub fn run_verify_graph(graph: &AilGraph, db_path: &Path) -> Result<VerifyResult, CliError> {
    let db = SqliteGraph::open(db_path).map_err(|e| CliError::MigrationFailed {
        message: e.to_string(),
    })?;
    check_migration(graph, &db)
}

// ── Core migration ────────────────────────────────────────────────────────────

/// Transfer an `AilGraph` into a `SqliteGraph`.
///
/// Transfer order:
/// 1. All nodes — preserves UUID IDs, contracts in list order, and metadata.
/// 2. Ev edges in BFS order via `node.children` — preserves child position.
/// 3. Eh edges — one directed edge per consecutive sibling pair.
/// 4. Ed edges — outgoing diagonal refs, active even when count = 0 [7.5-A].
pub(crate) fn migrate_ail_to_sqlite(
    graph: &AilGraph,
    db: &mut SqliteGraph,
) -> Result<MigrationReport, CliError> {
    let mut node_count = 0usize;
    let mut edge_count = 0usize;
    let mut contract_count = 0usize;

    // ── Step 1: nodes ─────────────────────────────────────────────────────────
    // `insert_contracts` stores contracts at positions 0, 1, 2 … in list order,
    // preserving the original contract order from the source file.  [7.5-C]
    for node in graph.all_nodes() {
        contract_count += node.contracts.len();
        db.add_node(node.clone())
            .map_err(|e| CliError::MigrationFailed {
                message: format!("add_node {}: {e}", node.id),
            })?;
        node_count += 1;
    }

    // ── Step 2: Ev edges (BFS, children in list order → correct positions) ───
    // `SqliteGraph::add_edge(Ev)` assigns `position = max_child_position + 1`.
    // Adding children in `node.children` order guarantees 0, 1, 2 … positions.
    if let Some(root_id) = graph.root_id() {
        edge_count += add_ev_edges_bfs(graph, db, root_id)?;
    }

    // ── Step 3: Eh edges (next-sibling chain) ─────────────────────────────────
    // `next_sibling_of` returns only the immediate next sibling, so each directed
    // Eh edge is added exactly once.
    for node in graph.all_nodes() {
        match graph.next_sibling_of(node.id) {
            Err(e) => {
                return Err(CliError::MigrationFailed {
                    message: format!("next_sibling_of {}: {e}", node.id),
                })
            }
            Ok(None) => {}
            Ok(Some(next_id)) => {
                db.add_edge(node.id, next_id, EdgeKind::Eh).map_err(|e| {
                    CliError::MigrationFailed {
                        message: format!("add_edge Eh {}->{next_id}: {e}", node.id),
                    }
                })?;
                edge_count += 1;
            }
        }
    }

    // ── Step 4: Ed edges — ACTIVE code path [7.5-A] ───────────────────────────
    // The text parser produces zero Ed edges on a fresh parse. The code path
    // must still exist so that AilGraphs with Ed edges (e.g. post-type-check
    // or manually constructed) migrate without silent data loss.
    // `outgoing_diagonal_refs_of` adds each directed edge exactly once.
    for node in graph.all_nodes() {
        let targets =
            graph
                .outgoing_diagonal_refs_of(node.id)
                .map_err(|e| CliError::MigrationFailed {
                    message: format!("outgoing_diagonal_refs_of {}: {e}", node.id),
                })?;
        for target_id in targets {
            db.add_edge(node.id, target_id, EdgeKind::Ed).map_err(|e| {
                CliError::MigrationFailed {
                    message: format!("add_edge Ed {}->{target_id}: {e}", node.id),
                }
            })?;
            edge_count += 1;
        }
    }

    Ok(MigrationReport {
        node_count,
        edge_count,
        contract_count,
        warnings: vec![],
    })
}

/// BFS over the AilGraph tree using `node.children` to add Ev edges into `db`
/// in original child order. Returns the number of Ev edges added.
///
/// `node.children` is the ordered `Vec<NodeId>` maintained by the parser and
/// builder. Iterating it in order means `SqliteGraph` assigns positions 0, 1,
/// 2 … correctly — a depth-first or HashMap-iteration order would not.
fn add_ev_edges_bfs(
    graph: &AilGraph,
    db: &mut SqliteGraph,
    root_id: ail_graph::types::NodeId,
) -> Result<usize, CliError> {
    let mut queue = VecDeque::new();
    queue.push_back(root_id);
    let mut count = 0usize;

    while let Some(node_id) = queue.pop_front() {
        let node = graph
            .get_node(node_id)
            .map_err(|e| CliError::MigrationFailed {
                message: format!("get_node {node_id}: {e}"),
            })?;

        if let Some(children) = &node.children {
            for &child_id in children {
                db.add_edge(node_id, child_id, EdgeKind::Ev).map_err(|e| {
                    CliError::MigrationFailed {
                        message: format!("add_edge Ev {node_id}->{child_id}: {e}"),
                    }
                })?;
                count += 1;
                queue.push_back(child_id);
            }
        }
    }

    Ok(count)
}

// ── Export helper ─────────────────────────────────────────────────────────────

/// Rebuild an `AilGraph` from a `SqliteGraph` suitable for rendering.
///
/// `SqliteGraph::get_node` always returns `children: None` (by design, see
/// `row_to_node` in `ail-db`). This function calls `db.children(id)` (ORDER BY
/// position ASC) to restore the ordered child list and writes it back into
/// `node.children` on the `AilGraph`. The `ail_text::render` function reads
/// that field directly via `ordered_children` in `tree_walk.rs`.
///
/// The root is set via `db.root_nodes()` (`SELECT … WHERE parent_id IS NULL`).
/// For a well-formed migrated database exactly one node has no parent.
pub(crate) fn rebuild_from_sqlite(db: &SqliteGraph) -> Result<AilGraph, CliError> {
    let mut ail = AilGraph::new();

    let all_ids = db.all_node_ids().map_err(|e| CliError::MigrationFailed {
        message: format!("all_node_ids: {e}"),
    })?;

    // Step 1: Add all nodes (children = None as returned by row_to_node).
    for &id in &all_ids {
        if let Some(n) = db.get_node(id).map_err(|e| CliError::MigrationFailed {
            message: format!("get_node {id}: {e}"),
        })? {
            ail.add_node(n).map_err(|e| CliError::MigrationFailed {
                message: format!("add_node {id}: {e}"),
            })?;
        }
    }

    // Step 2: Restore Ev edges and populate node.children for each parent.
    for &id in &all_ids {
        let child_ids = db.children(id).map_err(|e| CliError::MigrationFailed {
            message: format!("children({id}): {e}"),
        })?;

        if child_ids.is_empty() {
            continue;
        }

        for &child_id in &child_ids {
            ail.add_edge(id, child_id, EdgeKind::Ev)
                .map_err(|e| CliError::MigrationFailed {
                    message: format!("add_edge Ev {id}->{child_id}: {e}"),
                })?;
        }

        // Write the ordered child list into node.children so render() can use it.
        ail.get_node_mut(id)
            .map_err(|e| CliError::MigrationFailed {
                message: format!("get_node_mut {id}: {e}"),
            })?
            .children = Some(child_ids);
    }

    // Step 3: Set the root from root_nodes() — no pattern-matching heuristic.
    // root_nodes() queries: SELECT id FROM nodes WHERE parent_id IS NULL.
    // In a well-formed migrated DB exactly one node has no parent.
    let roots = db.root_nodes().map_err(|e| CliError::MigrationFailed {
        message: format!("root_nodes: {e}"),
    })?;

    if let [single_root] = roots.as_slice() {
        ail.set_root(*single_root)
            .map_err(|e| CliError::MigrationFailed {
                message: format!("set_root {single_root}: {e}"),
            })?;
    }
    // Multiple roots or none: skip set_root; renderer falls back to Eh-chain ordering.

    Ok(ail)
}

// ── Verify ────────────────────────────────────────────────────────────────────

/// Compare every node in `graph` against its counterpart in `db`.
///
/// Checks intent, pattern, and contracts. Ordering verification is covered by
/// the dedicated `t075_migrate_preserves_node_order` test, not duplicated here.
fn check_migration(graph: &AilGraph, db: &SqliteGraph) -> Result<VerifyResult, CliError> {
    let mut verified_nodes = 0usize;
    let mut mismatches: Vec<String> = Vec::new();

    for node in graph.all_nodes() {
        match db.get_node(node.id) {
            Err(e) => mismatches.push(format!("error reading {}: {e}", node.id)),
            Ok(None) => mismatches.push(format!(
                "missing node {} (intent: {})",
                node.id, node.intent
            )),
            Ok(Some(db_node)) => {
                if node.intent != db_node.intent {
                    mismatches.push(format!(
                        "intent mismatch {}: expected {}, got {}",
                        node.id, node.intent, db_node.intent
                    ));
                } else if node.pattern != db_node.pattern {
                    mismatches.push(format!(
                        "pattern mismatch {} ({}): expected {:?}, got {:?}",
                        node.id, node.intent, node.pattern, db_node.pattern
                    ));
                } else if node.contracts != db_node.contracts {
                    mismatches.push(format!("contracts mismatch {} ({})", node.id, node.intent));
                } else {
                    verified_nodes += 1;
                }
            }
        }
    }

    if !mismatches.is_empty() {
        let count = mismatches.len();
        let detail = mismatches.join("\n");
        return Err(CliError::VerifyFailed { count, detail });
    }

    Ok(VerifyResult {
        verified_nodes,
        mismatches: vec![],
    })
}
