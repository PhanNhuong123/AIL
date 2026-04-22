use ail_ui_bridge::pipeline::{load_verified_from_path, read_project_name};
use ail_ui_bridge::serialize::serialize_graph;
use ail_ui_bridge::types::graph_json::GraphJson;
use std::path::Path;
use std::path::PathBuf;

fn wallet_service_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("wallet_service")
}

fn wallet_service_src() -> PathBuf {
    wallet_service_root().join("src")
}

/// Test 14: full roundtrip — serialize → JSON string → deserialize → PartialEq.
#[test]
fn test_serialize_roundtrip() {
    let src_path = wallet_service_src();
    let verified = load_verified_from_path(&src_path).expect("wallet pipeline must succeed");
    // read_project_name expects the project root (where ail.config.toml lives).
    let name = read_project_name(&wallet_service_root());
    let original = serialize_graph(&verified, &name);

    let json_str = serde_json::to_string(&original).expect("serialize to string must succeed");
    let restored: GraphJson =
        serde_json::from_str(&json_str).expect("deserialize from string must succeed");

    assert_eq!(original, restored, "roundtrip must preserve equality");
    assert_eq!(
        restored.issues, original.issues,
        "roundtrip must preserve issues equality"
    );
    assert_eq!(
        restored.externals, original.externals,
        "roundtrip must preserve externals equality"
    );
}
