//! Integration tests for `ail migrate` and `ail export`.
//!
//! All tests that touch the wallet_full fixture parse the same directory tree
//! and migrate it to a per-test temp database so they can run in parallel
//! without interfering with each other.

use std::path::Path;

use ail_cli::{migrate_graph, run_export, run_migrate, CliError};
use ail_db::SqliteGraph;
use ail_graph::{
    graph::GraphBackend,
    types::{EdgeKind, NodeId, Pattern},
    AilGraph,
};
use ail_text::parse_directory;

// Path to the wallet_full fixture shared across several crates.
fn wallet_fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("ail-text")
        .join("tests")
        .join("fixtures")
        .join("wallet_full")
}

// ── Roundtrip ─────────────────────────────────────────────────────────────────

#[test]
fn t075_migrate_wallet_service_roundtrip() {
    let fixture = wallet_fixture_path();
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("wallet.ail.db");

    run_migrate(&fixture, &db_path, false).expect("migrate should succeed");

    // DB node count must equal AilGraph node count.
    let graph = parse_directory(&fixture).unwrap();
    let db = SqliteGraph::open(&db_path).unwrap();
    assert_eq!(
        db.node_count(),
        graph.node_count(),
        "DB node count should match parsed graph"
    );
}

// ── Node preservation ─────────────────────────────────────────────────────────

#[test]
fn t075_migrate_preserves_all_nodes() {
    let fixture = wallet_fixture_path();
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("wallet.ail.db");

    // Parse once so IDs are stable throughout the test.
    let graph = parse_directory(&fixture).unwrap();
    migrate_graph(&graph, &db_path).unwrap();

    let db = SqliteGraph::open(&db_path).unwrap();

    for node in graph.all_nodes() {
        let found = db.get_node(node.id).expect("DB read should not error");
        assert!(
            found.is_some(),
            "node {} (intent: {}) missing from DB",
            node.id,
            node.intent
        );
    }
}

// ── Edge preservation ─────────────────────────────────────────────────────────

#[test]
fn t075_migrate_preserves_all_edges() {
    let fixture = wallet_fixture_path();
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("wallet.ail.db");

    // Parse once so IDs are stable throughout the test.
    let graph = parse_directory(&fixture).unwrap();
    migrate_graph(&graph, &db_path).unwrap();

    let db = SqliteGraph::open(&db_path).unwrap();

    // Every Ev parent→child relationship in AilGraph must exist in SqliteGraph.
    for node in graph.all_nodes() {
        if let Some(children) = &node.children {
            for &child_id in children {
                let db_parent = db
                    .parent(child_id)
                    .expect("DB parent lookup should not error");
                assert_eq!(
                    db_parent,
                    Some(node.id),
                    "parent of {} should be {} (intent: {})",
                    child_id,
                    node.id,
                    node.intent
                );
            }
        }
    }
}

// ── Contract preservation ─────────────────────────────────────────────────────

#[test]
fn t075_migrate_preserves_contracts() {
    let fixture = wallet_fixture_path();
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("wallet.ail.db");

    // Parse once so IDs are stable throughout the test.
    let graph = parse_directory(&fixture).unwrap();
    migrate_graph(&graph, &db_path).unwrap();

    let db = SqliteGraph::open(&db_path).unwrap();

    for node in graph.all_nodes() {
        if !node.contracts.is_empty() {
            let db_node = db
                .get_node(node.id)
                .expect("DB read should not error")
                .expect("node should exist in DB");
            assert_eq!(
                node.contracts, db_node.contracts,
                "contracts mismatch for node {} ({})",
                node.id, node.intent
            );
        }
    }
}

// ── Child ordering ────────────────────────────────────────────────────────────

#[test]
fn t075_migrate_preserves_node_order() {
    let fixture = wallet_fixture_path();
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("wallet.ail.db");

    // Parse once so IDs are stable throughout the test.
    let graph = parse_directory(&fixture).unwrap();
    migrate_graph(&graph, &db_path).unwrap();

    let db = SqliteGraph::open(&db_path).unwrap();

    for node in graph.all_nodes() {
        if let Some(expected) = &node.children {
            if !expected.is_empty() {
                let db_children = db
                    .children(node.id)
                    .expect("DB children lookup should not error");
                assert_eq!(
                    &db_children, expected,
                    "child order mismatch for node {} ({})",
                    node.id, node.intent
                );
            }
        }
    }
}

// ── Ed edge transfer ──────────────────────────────────────────────────────────

/// Build a minimal AilGraph with an Ed edge manually, migrate it, and verify
/// the Ed edge appears in the database. This exercises the [7.5-A] code path
/// that is a no-op on freshly-parsed projects but must work for any AilGraph.
#[test]
fn t075_migrate_preserves_ed_edges() {
    use ail_graph::types::Node;

    let mut graph = AilGraph::new();

    let id_caller = graph
        .add_node(Node::new(NodeId::new(), "caller func", Pattern::Do))
        .unwrap();
    let id_callee = graph
        .add_node(Node::new(NodeId::new(), "callee type", Pattern::Define))
        .unwrap();

    // Wire an outgoing Ed (diagonal) edge: caller → callee.
    graph.add_edge(id_caller, id_callee, EdgeKind::Ed).unwrap();

    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("ed_test.ail.db");

    migrate_graph(&graph, &db_path).expect("migrate with Ed edge should succeed");

    let db = SqliteGraph::open(&db_path).unwrap();
    let refs = db
        .outgoing_diagonal_refs(id_caller)
        .expect("outgoing_diagonal_refs should not error");

    assert_eq!(
        refs,
        vec![id_callee],
        "Ed edge caller->{id_callee} should be present in DB"
    );
}

// ── Verify catches mismatch ───────────────────────────────────────────────────

/// Migrate the wallet fixture, then remove a node from the database to create
/// a deliberate mismatch. Calling run_verify_graph against the original graph
/// should return VerifyFailed for the deleted node.
#[test]
fn t075_migrate_verify_catches_mismatch() {
    let fixture = wallet_fixture_path();
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("wallet.ail.db");

    // Parse once for stable IDs throughout the test.
    let graph = parse_directory(&fixture).unwrap();
    migrate_graph(&graph, &db_path).unwrap();

    // Remove a non-root node from the DB to create a deliberate mismatch.
    let root_id = graph.root_id();
    let target_id = graph
        .all_nodes()
        .find(|n| Some(n.id) != root_id)
        .expect("wallet graph should have non-root nodes")
        .id;
    {
        let mut db = SqliteGraph::open(&db_path).unwrap();
        db.remove_node(target_id)
            .expect("remove_node should succeed");
    }

    // Verify the original graph against the modified DB — should detect the missing node.
    let result = ail_cli::commands::migrate::run_verify_graph(&graph, &db_path);
    assert!(
        matches!(result, Err(CliError::VerifyFailed { .. })),
        "expected VerifyFailed but got: {result:?}"
    );
}

// ── Export ────────────────────────────────────────────────────────────────────

#[test]
fn t075_export_produces_valid_ail_files() {
    let fixture = wallet_fixture_path();
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("wallet.ail.db");
    let export_dir = tmp.path().join("exported");

    run_migrate(&fixture, &db_path, false).unwrap();
    run_export(&db_path, &export_dir).expect("export should succeed");

    let out_file = export_dir.join("export.ail");
    assert!(out_file.exists(), "export.ail should be created");

    let content = std::fs::read_to_string(&out_file).unwrap();
    assert!(!content.trim().is_empty(), "export.ail should not be empty");

    // The exported file must be re-parseable as valid .ail syntax.
    parse_directory(&export_dir).expect("exported directory should be parseable");
}

#[test]
fn t075_export_roundtrip_matches_original() {
    let fixture = wallet_fixture_path();
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("wallet.ail.db");
    let export_dir = tmp.path().join("exported");

    run_migrate(&fixture, &db_path, false).unwrap();
    run_export(&db_path, &export_dir).unwrap();

    let original = parse_directory(&fixture).unwrap();
    let exported = parse_directory(&export_dir).unwrap();

    // Every non-container node from the original must appear in the exported graph
    // with the same intent, pattern, and contracts.
    // (Container Describe nodes with name=None are structural and may be rendered
    // differently by the exporter, so we skip them.)
    // Node IDs are NOT compared — exported .ail files are re-parsed with fresh UUIDs.
    let skipped_container = |n: &&ail_graph::types::Node| -> bool {
        n.pattern == Pattern::Describe && n.metadata.name.is_none()
    };

    for orig_node in original.all_nodes().filter(|n| !skipped_container(n)) {
        // Match by intent and pattern since re-parsed nodes get fresh UUIDs.
        let exported_node = exported
            .all_nodes()
            .find(|n| n.intent == orig_node.intent && n.pattern == orig_node.pattern);
        assert!(
            exported_node.is_some(),
            "node {} ({}) missing from exported graph",
            orig_node.id,
            orig_node.intent
        );
        let exported_node = exported_node.unwrap();
        assert_eq!(
            orig_node.contracts, exported_node.contracts,
            "contracts mismatch for {} ({})",
            orig_node.id, orig_node.intent
        );
    }
}

// ── Empty project ─────────────────────────────────────────────────────────────

#[test]
fn t075_migrate_empty_project() {
    let tmp = tempfile::tempdir().unwrap();
    let src_dir = tmp.path().join("empty_src");
    let db_path = tmp.path().join("empty.ail.db");

    // An empty directory is a valid project (just the root container node).
    std::fs::create_dir_all(&src_dir).unwrap();
    run_migrate(&src_dir, &db_path, false).expect("empty project should migrate without error");

    let db = SqliteGraph::open(&db_path).unwrap();
    // The root container is the only node (one Describe node for the directory).
    assert!(
        db.node_count() >= 1,
        "DB should have at least the root container node"
    );
}
