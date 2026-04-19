//! CLI integration test: verify + coverage after agent-like writes on wallet_service (Phase 15 task 15.2).
//!
//! Proves:
//! - `run_verify` (filesystem backend) passes on the unmodified wallet_service.
//! - After agent-like writes (3 child nodes + Ev edges inserted directly into the
//!   migrated SQLite DB), `run_verify` still passes — DB-side mutations are
//!   transparent to the filesystem-based verifier.
//! - Coverage status transitions from `Leaf` to a non-leaf status after children
//!   are attached and ancestor coverage rows are invalidated (embeddings-gated).
//!
//! Two tests are `#[ignore]` + `#[cfg(feature = "embeddings")]` because
//! `run_coverage` calls `std::process::exit(1)` under default features — those
//! tests exercise the full ONNX-backed compute + cache paths and are run
//! manually with `cargo test -p ail-cli --features embeddings -- --ignored`.

use std::fs;
use std::path::{Path, PathBuf};

use ail_cli::{run_migrate, run_verify};
use ail_db::SqliteGraph;
use ail_graph::graph::GraphBackend;
use ail_graph::types::{EdgeKind, Node, NodeId, Pattern};

#[cfg(feature = "embeddings")]
use ail_coverage::CoverageResult;
#[cfg(feature = "embeddings")]
use ail_graph::cic::{CoverageConfig, CoverageInfo, CoverageStatus};

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

/// Copy the example into a temp directory and return `(temp_dir, project_root)`.
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

/// Attach three children to `parent_id` via `Ev` edges and return their IDs.
fn attach_three_children(db: &mut SqliteGraph, parent_id: NodeId) -> [NodeId; 3] {
    let intents = [
        "validate users have sufficient balance",
        "execute transfer between wallets",
        "save transfer result to ledger",
    ];

    // Placeholder ids — every element is overwritten below before being used.
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

// ─── Tests ────────────────────────────────────────────────────────────────────

/// 15.2-1: `run_verify` (filesystem backend) passes before and after agent-like
/// writes to the SQLite DB.
///
/// The filesystem verifier parses `.ail` sources directly and does not read the
/// SQLite DB, so inserting children into the DB is transparent to it. This test
/// proves that agent-like writes do not corrupt any shared state that would
/// cause the pipeline to fail.
#[test]
fn ail_verify_passes_after_agent_like_writes() {
    let (_tmp, project) = fresh_example_project();
    let src = project.join("src");
    let db_path = project.join("project.ail.db");

    // 1. Migrate wallet to SQLite.
    run_migrate(&src, &db_path, false).expect("migrate must succeed on wallet_service");

    // 2. Baseline verify: must pass on unmodified wallet (filesystem backend).
    run_verify(&project, None, None).expect("baseline verify must pass");

    // 3. Open the DB and attach 3 children under transfer_money.
    let mut db = SqliteGraph::open(&db_path).expect("DB open must succeed");
    let parent_id = resolve_transfer_money_id(&db);
    let _child_ids = attach_three_children(&mut db, parent_id);
    drop(db);

    // 4. Verify again with the filesystem backend: must still pass.
    //    The verifier re-parses `.ail` sources — DB mutations are transparent.
    run_verify(&project, None, None).expect("post-write verify must pass");
}

/// 15.2-2: Proves that the seeded Leaf coverage row is marked stale
/// (load_coverage returns None) after children are attached via add_edge(Ev)
/// and ancestor invalidation is applied.
///
/// Gated on `#[ignore]` because `run_coverage` calls `std::process::exit(1)`
/// under default features. Run with:
///   cargo test -p ail-cli --features embeddings --test cli_e2e_wallet_agent -- --ignored
#[test]
#[ignore]
#[cfg(feature = "embeddings")]
fn coverage_status_changes_after_agent_like_writes() {
    if ail_search::OnnxEmbeddingProvider::ensure_model().is_err() {
        eprintln!(
            "[skip] ONNX model unavailable — skipping coverage_status_changes_after_agent_like_writes"
        );
        return;
    }

    let (_tmp, project) = fresh_example_project();
    let src = project.join("src");
    let db_path = project.join("project.ail.db");

    run_migrate(&src, &db_path, false).expect("migrate must succeed");

    let mut db = SqliteGraph::open(&db_path).expect("DB open must succeed");
    let parent_id = resolve_transfer_money_id(&db);

    let cfg = CoverageConfig::default();
    let cfg_hash = cfg.config_hash();

    // Snapshot 1: seed a Leaf coverage row for transfer_money (no children yet).
    let leaf_result = CoverageResult {
        score: None,
        child_contributions: vec![],
        missing_aspects: vec![],
        empty_parent: false,
        degenerate_basis_fallback: false,
    };
    let leaf_info: CoverageInfo = leaf_result.into_info(&cfg, cfg_hash.clone());
    assert_eq!(
        leaf_info.status,
        CoverageStatus::Leaf,
        "a no-child CoverageResult must produce CoverageStatus::Leaf"
    );

    db.save_coverage(&parent_id.to_string(), &leaf_info)
        .expect("save_coverage must succeed");

    let before = db
        .load_coverage(&parent_id.to_string(), &cfg_hash)
        .expect("load_coverage must not error")
        .expect("coverage row must be present after save");
    assert_eq!(
        before.status,
        CoverageStatus::Leaf,
        "seeded coverage row must report Leaf status"
    );

    // Attach 3 children — this triggers automatic ancestor invalidation via
    // the `add_edge(Ev)` hook in `SqliteGraph`.
    let _child_ids = attach_three_children(&mut db, parent_id);

    // The Ev edge insertion already invalidated the parent's coverage row.
    // Explicit invalidation is safe and idempotent.
    let ancestors = db
        .ancestors(parent_id)
        .expect("ancestors must not error")
        .iter()
        .map(|id| id.to_string())
        .chain(std::iter::once(parent_id.to_string()))
        .collect::<Vec<_>>();
    db.invalidate_coverage_for_ancestors(&ancestors)
        .expect("invalidate_coverage_for_ancestors must succeed");

    // After invalidation the row must be stale — load_coverage returns None.
    let after_invalidation = db
        .load_coverage(&parent_id.to_string(), &cfg_hash)
        .expect("load_coverage after invalidation must not error");
    assert!(
        after_invalidation.is_none(),
        "coverage row must be stale after ancestor invalidation; parent now has children"
    );
}
