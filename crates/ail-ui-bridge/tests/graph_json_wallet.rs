use ail_ui_bridge::pipeline::{load_verified_from_path, read_project_name};
use ail_ui_bridge::serialize::serialize_graph;
use ail_ui_bridge::types::node_detail::{CounterexampleDetail, NodeDetail, VerificationDetail};
use ail_ui_bridge::types::status::Status;
use std::path::{Path, PathBuf};

/// Project root — where `ail.config.toml` lives.
fn wallet_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("wallet_service")
}

/// Source directory fed to the parse stage.
fn wallet_src_path() -> PathBuf {
    wallet_root().join("src")
}

/// Test 1: wallet_service serializes with correct counts.
///
/// wallet_service/src has: add_money, deduct_money, transfer_money (Do nodes)
/// plus types and value objects. fn_count == 3, modules.len() >= 1, node_count > 0.
#[test]
fn test_load_wallet_service() {
    let path = wallet_src_path();
    let verified = load_verified_from_path(&path).expect("wallet pipeline must succeed");
    // read_project_name must receive the project root, not src/, so that it
    // finds ail.config.toml and returns the real project name rather than "src".
    let name = read_project_name(&wallet_root());
    let graph = serialize_graph(&verified, &name);

    // Guard against the regression where read_project_name returns "src"
    // because it was called with the src/ subdirectory.
    assert_ne!(
        graph.project.name, "src",
        "project.name must not be 'src' — read_project_name should receive the project root"
    );

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
}

/// Test 2: all modules have the single default cluster populated.
#[test]
fn test_clusters_populated() {
    let path = wallet_src_path();
    let verified = load_verified_from_path(&path).expect("wallet pipeline must succeed");
    let name = read_project_name(&wallet_root());
    let graph = serialize_graph(&verified, &name);

    assert_eq!(
        graph.clusters.len(),
        1,
        "expected exactly 1 cluster, got {}",
        graph.clusters.len()
    );
    let cluster = &graph.clusters[0];
    assert_eq!(cluster.id, "default");
    assert_eq!(cluster.color, "#2997ff");
    // Cluster name should equal the project name.
    assert_eq!(cluster.name, graph.project.name);

    for module in &graph.modules {
        assert_eq!(
            module.cluster, "default",
            "module '{}' must have cluster='default'",
            module.name
        );
        assert_eq!(
            module.cluster_color, "#2997ff",
            "module '{}' must have cluster_color='#2997ff'",
            module.name
        );
    }
}

/// Test 5: hand-constructed NodeDetail with synthetic counterexample roundtrips.
#[test]
fn test_detail_has_counterexample() {
    let detail = NodeDetail {
        name: "check_balance".to_string(),
        status: Status::Fail,
        description: "Check that sender has enough balance.".to_string(),
        receives: Vec::new(),
        returns: Vec::new(),
        rules: Vec::new(),
        inherited: Vec::new(),
        proven: Vec::new(),
        verification: VerificationDetail {
            ok: false,
            counterexample: Some(CounterexampleDetail {
                scenario: "User with 100, tries to send 500".to_string(),
                effect: "balance goes negative".to_string(),
                violates: "sender_balance >= amount".to_string(),
            }),
        },
        code: None,
    };

    // Roundtrip through JSON.
    let json = serde_json::to_string(&detail).expect("serialize");
    let restored: NodeDetail = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(detail, restored);
    let ce = restored
        .verification
        .counterexample
        .as_ref()
        .expect("counterexample must be present");
    assert_eq!(ce.scenario, "User with 100, tries to send 500");
    assert_eq!(ce.violates, "sender_balance >= amount");
}

/// Test 8: for transfer_money's child step, `detail[step_path].inherited`
/// must be non-empty and include at least one ancestor rule text.
#[test]
fn test_inherited_rules_populated() {
    let path = wallet_src_path();
    let verified = load_verified_from_path(&path).expect("wallet pipeline must succeed");
    let name = read_project_name(&wallet_root());
    let graph = serialize_graph(&verified, &name);

    // Find transfer_money function.
    let transfer_fn = graph
        .modules
        .iter()
        .flat_map(|m| m.functions.iter())
        .find(|f| f.name.contains("transfer"))
        .expect("transfer_money function must be present");

    // Get its steps.
    let steps = transfer_fn
        .steps
        .as_ref()
        .expect("transfer must have steps");
    assert!(
        !steps.is_empty(),
        "transfer_money must have at least one step"
    );

    // Find a step that has inherited rules in the detail map.
    let mut found_inherited = false;
    for step in steps {
        if let Some(step_detail) = graph.detail.get(&step.id) {
            if !step_detail.inherited.is_empty() {
                found_inherited = true;
                // Verify at least one inherited rule has non-empty text.
                assert!(
                    step_detail.inherited.iter().any(|r| !r.text.is_empty()),
                    "inherited rule text must be non-empty"
                );
                break;
            }
        }
    }

    assert!(
        found_inherited,
        "expected at least one step of transfer_money to have inherited rules; \
         detail keys: {:?}",
        graph.detail.keys().collect::<Vec<_>>()
    );
}
