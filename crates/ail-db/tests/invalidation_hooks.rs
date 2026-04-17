//! Integration tests for coverage cache invalidation hooks in `ail-db`.
//!
//! Task 13.2 Phase C — 2 tests with the `t132_` prefix.
//!
//! Verifies that `update_node` and `remove_node` automatically invalidate
//! coverage rows for the mutated node's ancestors.

use ail_db::SqliteGraph;
use ail_graph::{
    ChildContributionInfo, CoverageInfo, CoverageStatus, EdgeKind, GraphBackend, MissingAspectInfo,
    Node, NodeId, Pattern,
};
use tempfile::NamedTempFile;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn fresh_db() -> (NamedTempFile, SqliteGraph) {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_path_buf();
    let db = SqliteGraph::open_or_create(&path).unwrap();
    (file, db)
}

fn make_do(intent: &str) -> Node {
    Node::new(NodeId::new(), intent, Pattern::Do)
}

fn make_full_coverage(config_hash: &str) -> CoverageInfo {
    CoverageInfo {
        score: Some(0.92),
        status: CoverageStatus::Full,
        child_contributions: vec![ChildContributionInfo {
            node_id: "some-child".to_string(),
            intent_preview: "do something".to_string(),
            projection_magnitude: 0.88,
        }],
        missing_aspects: vec![MissingAspectInfo {
            concept: "logging".to_string(),
            similarity: 0.25,
        }],
        empty_parent: false,
        degenerate_basis_fallback: false,
        computed_at: 1_700_001_000,
        config_hash: config_hash.to_owned(),
    }
}

// ─── Build a 3-node chain A → B → C ─────────────────────────────────────────
//
// Returns (a_id, b_id, c_id).
fn build_chain(db: &mut SqliteGraph) -> (NodeId, NodeId, NodeId) {
    let a = GraphBackend::add_node(db, make_do("node A")).unwrap();
    let b = GraphBackend::add_node(db, make_do("node B")).unwrap();
    let c = GraphBackend::add_node(db, make_do("node C")).unwrap();
    GraphBackend::add_edge(db, a, b, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(db, b, c, EdgeKind::Ev).unwrap();
    (a, b, c)
}

// ─── t132_update_node_invalidates_coverage_ancestors ─────────────────────────

/// Updating node C (a leaf) must invalidate coverage rows for A and B.
#[test]
fn t132_update_node_invalidates_coverage_ancestors() {
    let (_guard, mut db) = fresh_db();
    let (a, b, c) = build_chain(&mut db);

    let hash = "abcdef0123456789";
    db.save_coverage(&a.to_string(), &make_full_coverage(hash))
        .unwrap();
    db.save_coverage(&b.to_string(), &make_full_coverage(hash))
        .unwrap();

    // Sanity: both rows are initially valid.
    assert!(db.load_coverage(&a.to_string(), hash).unwrap().is_some());
    assert!(db.load_coverage(&b.to_string(), hash).unwrap().is_some());

    // Update leaf C — its ancestors A and B lose children-coverage currency.
    let updated_c = make_do("node C updated");
    GraphBackend::update_node(&mut db, c, updated_c).unwrap();

    // Both ancestor coverage rows must now be stale.
    assert!(
        db.load_coverage(&a.to_string(), hash).unwrap().is_none(),
        "coverage for A must be invalidated after updating C"
    );
    assert!(
        db.load_coverage(&b.to_string(), hash).unwrap().is_none(),
        "coverage for B must be invalidated after updating C"
    );
}

// ─── t132_remove_node_invalidates_coverage_ancestors ─────────────────────────

/// Removing node C must invalidate coverage rows for its ancestors A and B.
#[test]
fn t132_remove_node_invalidates_coverage_ancestors() {
    let (_guard, mut db) = fresh_db();
    let (a, b, c) = build_chain(&mut db);

    let hash = "deadbeefcafebabe";
    db.save_coverage(&a.to_string(), &make_full_coverage(hash))
        .unwrap();
    db.save_coverage(&b.to_string(), &make_full_coverage(hash))
        .unwrap();

    // Sanity: both rows are initially valid.
    assert!(db.load_coverage(&a.to_string(), hash).unwrap().is_some());
    assert!(db.load_coverage(&b.to_string(), hash).unwrap().is_some());

    // Remove leaf C — its ancestors A and B lose a child and their scores are stale.
    GraphBackend::remove_node(&mut db, c).unwrap();

    // Both ancestor coverage rows must now be stale.
    assert!(
        db.load_coverage(&a.to_string(), hash).unwrap().is_none(),
        "coverage for A must be invalidated after removing C"
    );
    assert!(
        db.load_coverage(&b.to_string(), hash).unwrap().is_none(),
        "coverage for B must be invalidated after removing C"
    );
}
