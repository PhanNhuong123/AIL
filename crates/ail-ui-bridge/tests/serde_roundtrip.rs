use ail_ui_bridge::pipeline::{load_verified_from_path, read_project_name};
use ail_ui_bridge::serialize::serialize_graph;
use ail_ui_bridge::types::graph_json::GraphJson;
use ail_ui_bridge::types::node_detail::{
    CounterexampleDetail, NodeDetail, VerificationDetail, VerifyOutcome,
};
use ail_ui_bridge::types::status::Status;
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

/// VerifyOutcome roundtrip — every variant serializes as the lowercase string
/// that matches the frontend `VerifyOutcome` union.
#[test]
fn test_verify_outcome_serializes_lowercase() {
    let cases = [
        (VerifyOutcome::Sat, "sat"),
        (VerifyOutcome::Unsat, "unsat"),
        (VerifyOutcome::Unknown, "unknown"),
        (VerifyOutcome::Timeout, "timeout"),
    ];
    for (variant, expected) in cases {
        let json = serde_json::to_string(&variant).expect("serialize");
        assert_eq!(json, format!("\"{expected}\""));
        let back: VerifyOutcome = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, variant);
    }
}

/// Hand-constructed `NodeDetail` with `outcome=Some(Unsat)` roundtrips and
/// emits the field in the JSON wire shape.
#[test]
fn test_node_detail_outcome_roundtrips() {
    let detail = NodeDetail {
        name: "check_balance".to_string(),
        status: Status::Fail,
        description: String::new(),
        receives: Vec::new(),
        returns: Vec::new(),
        rules: Vec::new(),
        inherited: Vec::new(),
        proven: Vec::new(),
        verification: VerificationDetail {
            ok: false,
            counterexample: Some(CounterexampleDetail {
                scenario: "balance < amount".to_string(),
                effect: "transfer proceeds despite insufficient funds".to_string(),
                violates: "balance >= amount".to_string(),
            }),
            outcome: Some(VerifyOutcome::Unsat),
        },
        code: None,
    };

    let json = serde_json::to_string(&detail).expect("serialize");
    assert!(json.contains("\"outcome\":\"unsat\""));

    let back: NodeDetail = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, detail);
}

/// `outcome=None` is omitted on the wire (skip_serializing_if).
#[test]
fn test_node_detail_outcome_none_is_omitted() {
    let detail = NodeDetail {
        name: "ok".to_string(),
        status: Status::Ok,
        description: String::new(),
        receives: Vec::new(),
        returns: Vec::new(),
        rules: Vec::new(),
        inherited: Vec::new(),
        proven: Vec::new(),
        verification: VerificationDetail {
            ok: true,
            counterexample: None,
            outcome: None,
        },
        code: None,
    };
    let json = serde_json::to_string(&detail).expect("serialize");
    assert!(!json.contains("\"outcome\""));
}
