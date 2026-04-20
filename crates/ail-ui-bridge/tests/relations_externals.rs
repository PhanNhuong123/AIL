use ail_ui_bridge::pipeline::{load_verified_from_path, read_project_name};
use ail_ui_bridge::serialize::serialize_graph;
use ail_ui_bridge::types::graph_json::RelationJson;
use std::path::{Path, PathBuf};

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

/// Test 12: `RelationJson { style: Some("data") }` serializes with the
/// `style` field present and roundtrips correctly.
#[test]
fn test_relations_have_style() {
    let rel = RelationJson {
        from: "mod.fn_a".to_string(),
        to: "mod.TypeX".to_string(),
        label: String::new(),
        style: Some("data".to_string()),
    };

    let json = serde_json::to_string(&rel).expect("serialize");
    assert!(
        json.contains("\"style\":\"data\""),
        "serialized JSON must contain style:data, got: {json}"
    );

    let restored: RelationJson = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(rel, restored, "roundtrip must preserve equality");
    assert_eq!(restored.style.as_deref(), Some("data"));
}

/// Test 13: `externals` field exists in the serialized JSON and is empty
/// for the wallet_service project (no external nodes in the default MVP).
#[test]
fn test_externals_separate() {
    let path = wallet_src_path();
    let verified = load_verified_from_path(&path).expect("wallet pipeline must succeed");
    let name = read_project_name(&wallet_root());
    let graph = serialize_graph(&verified, &name);

    // The externals vec itself is empty (MVP: no external nodes).
    assert!(
        graph.externals.is_empty(),
        "externals must be empty for wallet_service"
    );

    // But the field must exist as an array in the serialized JSON.
    let json_value = serde_json::to_value(&graph).expect("serialize to value");
    let externals_field = json_value
        .get("externals")
        .expect("'externals' key must be present in serialized JSON");
    assert!(
        externals_field.is_array(),
        "'externals' must be an array, got: {externals_field:?}"
    );
}
