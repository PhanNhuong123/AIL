use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ail_graph::{AilGraph, EdgeKind, Node, NodeId, Pattern};
use ail_ui_bridge::ids::IdMap;
use ail_ui_bridge::pipeline::{load_verified_from_path, read_project_name};
use ail_ui_bridge::serialize::externals::classify_externals;
use ail_ui_bridge::serialize::serialize_graph;

fn wallet_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("wallet_service")
}

fn wallet_src_path() -> PathBuf {
    wallet_root().join("src")
}

/// Test 1: wallet_service has no external references — externals must be empty.
#[test]
fn test_externals_empty_for_wallet_service() {
    let path = wallet_src_path();
    let verified = load_verified_from_path(&path).expect("wallet pipeline must succeed");
    let name = read_project_name(&wallet_root());
    let graph = serialize_graph(&verified, &name);

    assert!(
        graph.externals.is_empty(),
        "externals must be empty for wallet_service, got: {:?}",
        graph.externals
    );

    // The field must exist as an array in the serialized JSON.
    let json_value = serde_json::to_value(&graph).expect("serialize to value");
    let externals_field = json_value
        .get("externals")
        .expect("'externals' key must be present in serialized JSON");
    assert!(
        externals_field.is_array(),
        "'externals' must be an array, got: {externals_field:?}"
    );
}

/// Test 2: two separate serialize calls on the same graph produce identical
/// externals arrays — classifier is deterministic.
#[test]
fn test_externals_deterministic_sort() {
    let path = wallet_src_path();
    let verified = load_verified_from_path(&path).expect("wallet pipeline must succeed");
    let name = read_project_name(&wallet_root());

    let graph_a = serialize_graph(&verified, &name);
    let graph_b = serialize_graph(&verified, &name);

    let json_a = serde_json::to_string(&graph_a.externals).expect("serialize a");
    let json_b = serde_json::to_string(&graph_b.externals).expect("serialize b");

    assert_eq!(
        json_a, json_b,
        "externals must be byte-for-byte identical across two calls"
    );
}

/// Test 3: synthetic graph with one node that has an Ed edge to an unindexed
/// target — `classify_externals` must return exactly one `ExternalJson` with
/// an id prefixed `external_`.
#[test]
fn test_externals_classify_synthetic_cross_ref() {
    // Build an in-memory graph with two nodes: source and target.
    // Only source is added to the IdMap; target is "external" from the
    // classifier's perspective because id_map.get_path(target_id) == "".
    let mut graph = AilGraph::new();

    let source_id = NodeId::new();
    let target_id = NodeId::new();

    let source_node = Node::new(source_id, "source function", Pattern::Do);
    let target_node = Node::new(target_id, "external target", Pattern::Do);

    graph.add_node(source_node).expect("add source");
    graph.add_node(target_node).expect("add target");

    // Ed edge: source → target (cross-reference).
    graph
        .add_edge(source_id, target_id, EdgeKind::Ed)
        .expect("add Ed edge");

    // Build IdMap manually — only include the source node, not the target.
    // That makes target_id "external" (get_path returns "").
    let mut id_map = IdMap {
        forward: BTreeMap::new(),
        reverse: BTreeMap::new(),
    };
    let source_path = "source_function".to_string();
    id_map
        .forward
        .insert(source_id.to_string(), source_path.clone());
    id_map.reverse.insert(source_path, source_id);

    let externals = classify_externals(&graph, &id_map);

    assert_eq!(
        externals.len(),
        1,
        "expected exactly 1 external, got: {externals:?}"
    );
    assert!(
        externals[0].id.starts_with("external_"),
        "external id must be prefixed 'external_', got: {}",
        externals[0].id
    );
}
