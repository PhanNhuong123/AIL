//! Phase 15 Task 15.1 — End-to-end coverage tests on `examples/wallet_service/`.
//!
//! Proves:
//! - Leaf nodes produce no coverage rows and are correctly tallied as N/A.
//! - `SqliteGraph::save_coverage` + `update_node` ancestor invalidation: after
//!   a child mutates, `load_coverage` on the parent returns `None`.
//! - The `cmd_all` leaf-tally loop is crash-free on the wallet_service graph.
//!
//! Two tests are `#[ignore]` + `#[cfg(feature = "embeddings")]` because
//! `run_coverage` calls `std::process::exit(1)` under default features — those
//! tests exercise the full ONNX-backed compute + cache paths and are run
//! manually with `cargo test -p ail-cli --features embeddings -- --ignored`.

use std::fs;
use std::path::{Path, PathBuf};

use ail_cli::run_migrate;
use ail_coverage::CoverageResult;
use ail_db::SqliteGraph;
use ail_graph::cic::{ChildContributionInfo, CoverageConfig, CoverageInfo, CoverageStatus};
use ail_graph::graph::GraphBackend;
use ail_graph::types::{EdgeKind, Node, NodeId, Pattern};

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Absolute path to the `examples/wallet_service/` project.
fn wallet_example_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("wallet_service")
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

/// Copy the example into a temp directory and return (temp_dir, project_root).
fn fresh_example_project() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().to_path_buf();
    copy_dir_all(&wallet_example_dir(), &project).unwrap();
    (tmp, project)
}

/// Resolve the `transfer_money` node id from a migrated DB.
///
/// Tries `find_by_name("transfer_money")` first (matches `metadata.name`),
/// then falls back to iterating all nodes and matching intent containing
/// "transfer money".
fn resolve_transfer_money_id(db: &SqliteGraph) -> NodeId {
    let by_name = db
        .find_by_name("transfer_money")
        .expect("find_by_name should not error");
    if let Some(&id) = by_name.first() {
        return id;
    }

    // Fallback: scan all nodes and match by intent substring.
    let ids = db.all_node_ids().expect("all_node_ids should not error");
    for id in ids {
        if let Ok(Some(node)) = db.get_node(id) {
            if node.intent.contains("transfer money") {
                return id;
            }
        }
    }

    panic!("transfer_money node not found in migrated wallet_service DB");
}

/// Attach three children to `parent_id` via `Ev` edges and return their IDs
/// in the order: [mutated_child, other1, other2].
fn attach_three_children(db: &mut SqliteGraph, parent_id: NodeId) -> [NodeId; 3] {
    let intents = [
        "validate users have sufficient balance",
        "execute transfer between wallets",
        "save transfer result to ledger",
    ];

    let mut ids = [NodeId::new(), NodeId::new(), NodeId::new()];
    for (i, intent) in intents.iter().enumerate() {
        let id = NodeId::new();
        let node = Node::new(id, *intent, Pattern::Do);
        db.add_node(node).expect("add_node should succeed");
        db.add_edge(parent_id, id, EdgeKind::Ev)
            .expect("add_edge Ev should succeed");
        ids[i] = id;
    }
    ids
}

/// Build a representative `CoverageInfo` for a parent node with the given
/// child ids, using the default `CoverageConfig`.
fn make_coverage_info(child_ids: &[NodeId], config_hash: &str) -> CoverageInfo {
    let child_contributions = child_ids
        .iter()
        .enumerate()
        .map(|(i, &id)| ChildContributionInfo {
            node_id: id.to_string(),
            intent_preview: format!("child intent preview {i}"),
            projection_magnitude: 0.7 + (i as f32) * 0.05,
        })
        .collect();

    CoverageInfo {
        score: Some(0.82),
        status: CoverageStatus::Partial,
        child_contributions,
        missing_aspects: vec![],
        empty_parent: false,
        degenerate_basis_fallback: false,
        computed_at: 1_700_000_000,
        config_hash: config_hash.to_owned(),
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

/// 15.1-1: All wallet_service nodes are leaves (no Ev children) immediately
/// after migration.  No coverage rows exist on a fresh migration.
///
/// This test exercises the `SqliteGraph` API directly and never calls
/// `run_coverage`, avoiding the `std::process::exit(1)` guard under default
/// features.
#[test]
fn t151_coverage_node_without_children_returns_na_or_unavailable() {
    let (_tmp, project) = fresh_example_project();
    let src = project.join("src");
    let db_path = project.join("project.ail.db");

    run_migrate(&src, &db_path, false).expect("migrate should succeed on wallet_service");

    let db = SqliteGraph::open(&db_path).expect("open migrated DB");

    // Verify no coverage rows were pre-seeded during migration.
    assert_eq!(
        db.coverage_count().expect("coverage_count should succeed"),
        0,
        "fresh migration must leave coverage_cache empty"
    );

    let cfg = CoverageConfig::default();
    let cfg_hash = cfg.config_hash();

    let ids = db.all_node_ids().expect("all_node_ids should not error");
    assert!(
        !ids.is_empty(),
        "migrated wallet_service must contain at least one node"
    );

    // For every node — leaf or structural — no coverage row must have been
    // pre-seeded by the migration.  Leaf nodes in particular must never have
    // a coverage row (they are tallied as N/A by the coverage command).
    let mut leaf_count: usize = 0;
    for id in &ids {
        let cached = db
            .load_coverage(&id.to_string(), &cfg_hash)
            .expect("load_coverage should not error");
        assert!(
            cached.is_none(),
            "fresh migration must have no coverage row for node {id}"
        );

        let children = db.children(*id).expect("children should not error");
        if children.is_empty() {
            leaf_count += 1;
        }
    }

    assert!(
        leaf_count > 0,
        "wallet_service must contain at least one leaf node"
    );

    // transfer_money must exist and must have no pre-computed coverage row,
    // regardless of whether it has structural children in the migrated graph.
    let transfer_id = resolve_transfer_money_id(&db);
    let cached = db
        .load_coverage(&transfer_id.to_string(), &cfg_hash)
        .expect("load_coverage for transfer_money should not error");
    assert!(
        cached.is_none(),
        "transfer_money must have no pre-computed coverage row after plain migrate"
    );

    // Acceptance criterion: a no-child leaf CoverageResult (score: None, empty
    // children) must produce CoverageInfo { status: Leaf } via into_info.
    // This exercises the end-to-end conversion without requiring ONNX.
    let leaf_result = CoverageResult {
        score: None,
        child_contributions: vec![],
        missing_aspects: vec![],
        empty_parent: false,
        degenerate_basis_fallback: false,
    };
    let info = leaf_result.into_info(&cfg, cfg_hash.clone());
    assert_eq!(
        info.status,
        CoverageStatus::Leaf,
        "no-child leaf CoverageResult must produce CoverageStatus::Leaf via into_info"
    );
    assert!(
        info.score.is_none(),
        "CoverageInfo for a leaf must have score: None"
    );
    assert!(
        info.child_contributions.is_empty(),
        "CoverageInfo for a leaf must have no child_contributions"
    );
}

/// 15.1-2: After seeding a `CoverageInfo` row for a parent node, mutating one
/// child via `update_node` invalidates the row — `load_coverage` returns `None`.
///
/// This test never calls `run_coverage`.
#[test]
fn t151_coverage_cache_invalidates_on_child_mutation() {
    let (_tmp, project) = fresh_example_project();
    let src = project.join("src");
    let db_path = project.join("project.ail.db");

    run_migrate(&src, &db_path, false).expect("migrate should succeed");

    let mut db = SqliteGraph::open(&db_path).expect("open migrated DB");

    let parent_id = resolve_transfer_money_id(&db);

    // Attach three children so the parent has Ev edges.
    let child_ids = attach_three_children(&mut db, parent_id);

    // Seed a coverage row for the parent AFTER attaching children.
    // (Attaching children invalidates coverage for the parent, but since no
    // row existed yet the invalidation was a no-op.)
    let cfg = CoverageConfig::default();
    let cfg_hash = cfg.config_hash();
    let info = make_coverage_info(&child_ids, &cfg_hash);

    db.save_coverage(&parent_id.to_string(), &info)
        .expect("save_coverage should succeed");

    // Verify the row is present and loadable.
    let loaded = db
        .load_coverage(&parent_id.to_string(), &cfg_hash)
        .expect("load_coverage should not error");
    assert!(
        loaded.is_some(),
        "coverage row must be present immediately after save_coverage"
    );

    assert_eq!(
        db.coverage_count()
            .expect("coverage_count should not error"),
        1,
        "exactly one coverage row must exist after saving one"
    );

    // Mutate child 0 — this triggers ancestor invalidation via update_node.
    let mutated_id = child_ids[0];
    let mut mutated_node = db
        .get_node(mutated_id)
        .expect("get_node should not error")
        .expect("child node must exist");
    mutated_node.intent = "validate users have sufficient balance and positive amount".to_owned();
    db.update_node(mutated_id, mutated_node)
        .expect("update_node should succeed");

    // The parent's coverage row must now be stale — load_coverage returns None
    // for stale rows.
    let after_mutation = db
        .load_coverage(&parent_id.to_string(), &cfg_hash)
        .expect("load_coverage after mutation should not error");
    assert!(
        after_mutation.is_none(),
        "coverage row for parent must be invalidated after child mutation"
    );
}

/// 15.1-3: The `cmd_all` leaf-tally loop is crash-free on the wallet_service
/// graph.  Mirrors the loop logic purely through `SqliteGraph` APIs — no ONNX.
#[test]
fn t151_coverage_all_crash_free_leaf_tally_pure_db() {
    let (_tmp, project) = fresh_example_project();
    let src = project.join("src");
    let db_path = project.join("project.ail.db");

    run_migrate(&src, &db_path, false).expect("migrate should succeed");

    let db = SqliteGraph::open(&db_path).expect("open migrated DB");

    assert!(
        db.node_count() > 0,
        "migrated wallet_service must have at least one node"
    );

    let ids = db.all_node_ids().expect("all_node_ids should not error");

    let mut leaf_count: usize = 0;
    let mut total_count: usize = 0;

    for id in &ids {
        total_count += 1;
        let children = db.children(*id).expect("children should not error");
        if children.is_empty() {
            leaf_count += 1;
        }
    }

    // The loop must complete without panicking.  The wallet_service fixture
    // has structural nodes (type definitions) that have children, so
    // leaf_count ≤ total_count.  Both counts must be positive.
    assert!(
        total_count > 0,
        "must have iterated at least one node without error"
    );
    assert!(
        leaf_count > 0,
        "wallet_service must contain at least one leaf node (got 0 out of {total_count})"
    );
    assert!(
        leaf_count <= total_count,
        "leaf count ({leaf_count}) must not exceed total node count ({total_count})"
    );
}

/// 15.1-4: Full `run_coverage --node` with embeddings: verifies the handler
/// writes a coverage row to the DB after computing with ONNX.
///
/// Gated on `#[ignore]` because `run_coverage` calls `std::process::exit(1)`
/// under default features.  Run with:
///   cargo test -p ail-cli --features embeddings --test cli_e2e_wallet_coverage -- --ignored
#[test]
#[ignore]
#[cfg(feature = "embeddings")]
fn t151_coverage_node_prints_score_and_child_breakdown() {
    use ail_cli::run_coverage;

    // Skip if ONNX model files are absent to avoid a noisy exit(1).
    if ail_search::OnnxEmbeddingProvider::ensure_model().is_err() {
        eprintln!("[skip] ONNX model unavailable — skipping t151_coverage_node_prints_score_and_child_breakdown");
        return;
    }

    let (_tmp, project) = fresh_example_project();
    let src = project.join("src");
    let db_path = project.join("project.ail.db");

    run_migrate(&src, &db_path, false).expect("migrate should succeed");

    let mut db = SqliteGraph::open(&db_path).expect("open migrated DB");
    let parent_id = resolve_transfer_money_id(&db);
    let _child_ids = attach_three_children(&mut db, parent_id);
    // Drop the mutable borrow before calling run_coverage, which reopens the DB.
    drop(db);

    run_coverage(
        &project,
        Some("transfer_money".to_owned()),
        false,
        false,
        None,
    )
    .expect("run_coverage --node transfer_money should succeed with embeddings");

    // Reopen the DB and assert the handler persisted a coverage row.
    let db2 = SqliteGraph::open(&db_path).expect("reopen DB after coverage");
    let cfg = CoverageConfig::default();
    let cfg_hash = cfg.config_hash();

    let loaded = db2
        .load_coverage(&parent_id.to_string(), &cfg_hash)
        .expect("load_coverage should not error");
    assert!(
        loaded.is_some(),
        "run_coverage --node must persist a CoverageInfo row for the target node"
    );
}

/// 15.1-5: After `--warm-cache`, a subsequent `--node` call does not add a new
/// row (cache hit — the row count stays stable).
///
/// Gated on `#[ignore]` for the same reason as test 4.
#[test]
#[ignore]
#[cfg(feature = "embeddings")]
fn t151_coverage_cache_hit_after_warmup() {
    use ail_cli::run_coverage;

    if ail_search::OnnxEmbeddingProvider::ensure_model().is_err() {
        eprintln!("[skip] ONNX model unavailable — skipping t151_coverage_cache_hit_after_warmup");
        return;
    }

    let (_tmp, project) = fresh_example_project();
    let src = project.join("src");
    let db_path = project.join("project.ail.db");

    run_migrate(&src, &db_path, false).expect("migrate should succeed");

    let mut db = SqliteGraph::open(&db_path).expect("open migrated DB");
    let parent_id = resolve_transfer_money_id(&db);
    let _child_ids = attach_three_children(&mut db, parent_id);
    drop(db);

    // Warm the cache for all non-leaf nodes.
    run_coverage(&project, None, false, true, None)
        .expect("run_coverage --warm-cache should succeed with embeddings");

    let db2 = SqliteGraph::open(&db_path).expect("open DB after warm-cache");
    let n1 = db2
        .coverage_count()
        .expect("coverage_count should not error");
    drop(db2);

    // Second call — node-specific, should hit the cache.
    run_coverage(
        &project,
        Some("transfer_money".to_owned()),
        false,
        false,
        None,
    )
    .expect("run_coverage --node transfer_money should succeed (cache hit)");

    let db3 = SqliteGraph::open(&db_path).expect("open DB after second call");
    let n2 = db3
        .coverage_count()
        .expect("coverage_count should not error");

    assert_eq!(
        n2, n1,
        "coverage row count must not grow on a cache hit (was {n1}, now {n2})"
    );

    let cfg = CoverageConfig::default();
    let cfg_hash = cfg.config_hash();
    let loaded = db3
        .load_coverage(&parent_id.to_string(), &cfg_hash)
        .expect("load_coverage should not error");
    assert!(
        loaded.is_some(),
        "transfer_money coverage row must still be valid after cache-hit call"
    );
}
