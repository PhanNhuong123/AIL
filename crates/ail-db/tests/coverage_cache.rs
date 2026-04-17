//! Integration tests for the `coverage_cache` table in `ail-db`.
//!
//! Task 13.2 Phase C — 7 tests with the `t132_` prefix.
//!
//! Tests use a temporary file (WAL mode) following the same helper pattern as
//! `sqlite_tests.rs`. Nodes are inserted before coverage rows because of the
//! FK constraint (`node_id REFERENCES nodes(id) ON DELETE CASCADE`).

use ail_db::SqliteGraph;
use ail_graph::{
    ChildContributionInfo, CoverageInfo, CoverageStatus, GraphBackend, MissingAspectInfo, Node,
    NodeId, Pattern,
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

/// Build a minimal `CoverageInfo` with the provided fields.
fn make_coverage_info(
    score: Option<f32>,
    status: CoverageStatus,
    config_hash: &str,
) -> CoverageInfo {
    CoverageInfo {
        score,
        status,
        child_contributions: vec![ChildContributionInfo {
            node_id: "child-1".to_string(),
            intent_preview: "validate input".to_string(),
            projection_magnitude: 0.42,
        }],
        missing_aspects: vec![MissingAspectInfo {
            concept: "error handling".to_string(),
            similarity: 0.31,
        }],
        empty_parent: false,
        degenerate_basis_fallback: false,
        computed_at: 1_700_000_000,
        config_hash: config_hash.to_owned(),
    }
}

// ─── t132_save_and_load_coverage_roundtrip ───────────────────────────────────

#[test]
fn t132_save_and_load_coverage_roundtrip() {
    let (_guard, mut db) = fresh_db();

    let node_id = GraphBackend::add_node(&mut db, make_do("compute total")).unwrap();
    let node_id_str = node_id.to_string();

    let config_hash = "aabbccdd11223344";
    let info = make_coverage_info(Some(0.85), CoverageStatus::Partial, config_hash);

    db.save_coverage(&node_id_str, &info).unwrap();

    let loaded = db
        .load_coverage(&node_id_str, config_hash)
        .unwrap()
        .expect("must find a valid row");

    assert_eq!(loaded.score, info.score);
    assert_eq!(loaded.status, info.status);
    assert_eq!(loaded.config_hash, info.config_hash);
    assert_eq!(loaded.computed_at, info.computed_at);
    assert_eq!(loaded.empty_parent, info.empty_parent);
    assert_eq!(
        loaded.degenerate_basis_fallback,
        info.degenerate_basis_fallback
    );
    assert_eq!(loaded.child_contributions.len(), 1);
    assert!((loaded.child_contributions[0].projection_magnitude - 0.42).abs() < 1e-5);
    assert_eq!(loaded.missing_aspects.len(), 1);
    assert_eq!(loaded.missing_aspects[0].concept, "error handling");
}

// ─── t132_load_returns_none_when_config_hash_differs ─────────────────────────

#[test]
fn t132_load_returns_none_when_config_hash_differs() {
    let (_guard, mut db) = fresh_db();

    let node_id = GraphBackend::add_node(&mut db, make_do("hash mismatch node")).unwrap();
    let node_id_str = node_id.to_string();

    let info = make_coverage_info(Some(0.95), CoverageStatus::Full, "hash_A");
    db.save_coverage(&node_id_str, &info).unwrap();

    // Load with a different config hash — must return None.
    let result = db.load_coverage(&node_id_str, "hash_B").unwrap();
    assert!(result.is_none(), "mismatched config_hash must return None");
}

// ─── t132_load_returns_none_when_invalid ─────────────────────────────────────

#[test]
fn t132_load_returns_none_when_invalid() {
    let (_guard, mut db) = fresh_db();

    let node_id = GraphBackend::add_node(&mut db, make_do("invalidated node")).unwrap();
    let node_id_str = node_id.to_string();

    let hash = "deadbeef12345678";
    let info = make_coverage_info(None, CoverageStatus::Leaf, hash);
    db.save_coverage(&node_id_str, &info).unwrap();

    // Mark the row stale via the public invalidation API (passes node itself in list).
    db.invalidate_coverage_for_ancestors(&[node_id_str.clone()])
        .unwrap();

    let result = db.load_coverage(&node_id_str, hash).unwrap();
    assert!(result.is_none(), "stale row (valid=0) must return None");
}

// ─── t132_invalidate_for_ancestors_marks_chain ───────────────────────────────

#[test]
fn t132_invalidate_for_ancestors_marks_chain() {
    use ail_graph::EdgeKind;

    let (_guard, mut db) = fresh_db();

    let grandparent_id = GraphBackend::add_node(&mut db, make_do("grandparent")).unwrap();
    let parent_id = GraphBackend::add_node(&mut db, make_do("parent")).unwrap();
    let child_id = GraphBackend::add_node(&mut db, make_do("child")).unwrap();

    // Build hierarchy: grandparent → parent → child
    GraphBackend::add_edge(&mut db, grandparent_id, parent_id, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut db, parent_id, child_id, EdgeKind::Ev).unwrap();

    let hash = "cafebabe00000001";
    let gp_str = grandparent_id.to_string();
    let par_str = parent_id.to_string();

    db.save_coverage(
        &gp_str,
        &make_coverage_info(Some(0.9), CoverageStatus::Full, hash),
    )
    .unwrap();
    db.save_coverage(
        &par_str,
        &make_coverage_info(Some(0.75), CoverageStatus::Partial, hash),
    )
    .unwrap();

    // Invalidate grandparent + parent.
    let updated = db
        .invalidate_coverage_for_ancestors(&[gp_str.clone(), par_str.clone()])
        .unwrap();
    assert_eq!(updated, 2, "must update exactly 2 rows");

    // Both rows must now be stale.
    let gp_result = db.load_coverage(&gp_str, hash).unwrap();
    assert!(gp_result.is_none(), "grandparent coverage must be stale");

    let par_result = db.load_coverage(&par_str, hash).unwrap();
    assert!(par_result.is_none(), "parent coverage must be stale");
}

// ─── t132_invalidate_empty_list_returns_zero ─────────────────────────────────

#[test]
fn t132_invalidate_empty_list_returns_zero() {
    let (_guard, mut db) = fresh_db();
    let node_id = GraphBackend::add_node(&mut db, make_do("some node")).unwrap();
    let hash = "0000000000000001";
    db.save_coverage(
        &node_id.to_string(),
        &make_coverage_info(Some(0.5), CoverageStatus::Weak, hash),
    )
    .unwrap();

    // Empty list must return Ok(0) without touching any rows.
    let count = db.invalidate_coverage_for_ancestors(&[]).unwrap();
    assert_eq!(count, 0, "empty list must return 0");

    // The saved row must still be valid.
    let loaded = db.load_coverage(&node_id.to_string(), hash).unwrap();
    assert!(
        loaded.is_some(),
        "row must still be valid after empty invalidation"
    );
}

// ─── t132_clear_coverage_empties_table ───────────────────────────────────────

#[test]
fn t132_clear_coverage_empties_table() {
    let (_guard, mut db) = fresh_db();

    let hash = "1122334455667788";
    for intent in &["node a", "node b", "node c"] {
        let id = GraphBackend::add_node(&mut db, make_do(intent)).unwrap();
        db.save_coverage(
            &id.to_string(),
            &make_coverage_info(Some(0.8), CoverageStatus::Full, hash),
        )
        .unwrap();
    }

    assert_eq!(db.coverage_count().unwrap(), 3);

    db.clear_coverage().unwrap();

    assert_eq!(
        db.coverage_count().unwrap(),
        0,
        "coverage_cache must be empty after clear"
    );
}

// ─── t132_table_row_count_includes_coverage_cache ────────────────────────────

#[test]
fn t132_table_row_count_includes_coverage_cache() {
    let (_guard, mut db) = fresh_db();

    let node_id = GraphBackend::add_node(&mut db, make_do("row count node")).unwrap();
    let hash = "ffeeddcc99887766";
    db.save_coverage(
        &node_id.to_string(),
        &make_coverage_info(Some(1.0), CoverageStatus::Full, hash),
    )
    .unwrap();

    let count = db.table_row_count("coverage_cache").unwrap();
    assert_eq!(
        count, 1,
        "table_row_count must report 1 row in coverage_cache"
    );
}
