/// Tests for the 3-stage typed pipeline path used by `load_project`.
///
/// The key invariant: `load_typed_from_path` + `serialize_typed_graph` must
/// succeed and return a well-formed `GraphJson` regardless of whether
/// `load_verified_from_path` (4-stage, with Z3 contract verification) would
/// fail. These tests assert that "load" is decoupled from "verify".
use std::path::{Path, PathBuf};

use ail_ui_bridge::pipeline::{load_typed_from_path, read_project_name};
use ail_ui_bridge::serialize::serialize_typed_graph;

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

/// Test 1: `load_typed_from_path` + `serialize_typed_graph` succeeds for the
/// wallet_service project and returns a well-formed GraphJson.
///
/// This test is deliberately independent of the verify stage: even if the
/// project had a Z3 contract failure, the typed path would still return Ok.
/// The wallet_service project is used because it is the canonical example;
/// the `issues` field must be empty because verification has not run.
#[test]
fn load_project_succeeds_when_verify_not_run() {
    let path = wallet_src_path();
    let typed = load_typed_from_path(&path)
        .expect("load_typed_from_path must succeed for wallet_service");
    let name = read_project_name(&wallet_root());
    let graph = serialize_typed_graph(&typed, &name);

    assert!(
        graph.project.node_count > 0,
        "expected node_count > 0, got {}",
        graph.project.node_count
    );
    assert!(
        !graph.modules.is_empty(),
        "expected at least 1 module, got {}",
        graph.modules.len()
    );
    assert_eq!(
        graph.project.fn_count, 3,
        "expected 3 Do functions (add_money, deduct_money, transfer_money), got {}",
        graph.project.fn_count
    );
    // Verify has NOT run — issues must be empty regardless of contract content.
    assert!(
        graph.issues.is_empty(),
        "expected no issues from typed-only path (verify not run), got: {:?}",
        graph.issues
    );
    // Project name must be read from ail.config.toml, not be 'src'.
    assert_ne!(
        graph.project.name, "src",
        "project.name must not be 'src' — read_project_name should receive the project root"
    );
}

/// Test 2: `serialize_typed_graph` and `serialize_graph` produce the same
/// structural output (project id/name, module count, module names, cluster
/// count) when both pipelines succeed.
///
/// The `detail` and `issues` fields may differ: the verified path may have
/// verification-level detail while the typed path produces `issues == []`.
/// This test validates that the structural skeleton is identical so the
/// frontend can trust typed-path output as a valid initial graph.
///
/// Gated on `z3-verify` to match the existing pattern in `graph_json_wallet.rs`
/// — without Z3, the verified path is a no-op pass and the comparison still
/// holds, but the feature gate is kept for symmetry with the existing test
/// matrix.
#[cfg(feature = "z3-verify")]
#[test]
fn serialize_typed_graph_structure_matches_serialize_graph() {
    use ail_ui_bridge::pipeline::load_verified_from_path;
    use ail_ui_bridge::serialize::serialize_graph;

    let path = wallet_src_path();
    let name = read_project_name(&wallet_root());

    let typed = load_typed_from_path(&path)
        .expect("load_typed_from_path must succeed for wallet_service");
    let typed_json = serialize_typed_graph(&typed, &name);

    let verified = load_verified_from_path(&path)
        .expect("load_verified_from_path must succeed for wallet_service");
    let verified_json = serialize_graph(&verified, &name);

    // Structural fields must match.
    assert_eq!(
        typed_json.project.id, verified_json.project.id,
        "project.id must match between typed and verified paths"
    );
    assert_eq!(
        typed_json.project.name, verified_json.project.name,
        "project.name must match between typed and verified paths"
    );
    assert_eq!(
        typed_json.modules.len(),
        verified_json.modules.len(),
        "module count must match between typed and verified paths"
    );
    assert_eq!(
        typed_json.clusters.len(),
        verified_json.clusters.len(),
        "cluster count must match between typed and verified paths"
    );

    // Module names must match (order-stable because IdMap uses deterministic
    // path-based IDs derived from the parse tree traversal order).
    let typed_names: Vec<&str> = typed_json.modules.iter().map(|m| m.name.as_str()).collect();
    let verified_names: Vec<&str> = verified_json
        .modules
        .iter()
        .map(|m| m.name.as_str())
        .collect();
    assert_eq!(
        typed_names, verified_names,
        "module names must match between typed and verified paths"
    );

    // Typed path must have empty issues (verify not run); verified may or may
    // not have issues depending on Z3 results — we don't assert verified_json
    // here since it is orthogonal to the structural comparison.
    assert!(
        typed_json.issues.is_empty(),
        "typed path must produce no issues (verify not run), got: {:?}",
        typed_json.issues
    );
}
