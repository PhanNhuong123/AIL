use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ail_ui_bridge::pipeline::{load_verified_from_path, read_project_name};
use ail_ui_bridge::serialize::issues::collect_issues;
use ail_ui_bridge::serialize::serialize_graph;
use ail_ui_bridge::types::graph_json::{GraphJson, IssueJson, ProjectJson};
use ail_ui_bridge::types::node_detail::{CounterexampleDetail, NodeDetail, VerificationDetail};
use ail_ui_bridge::types::status::Status;

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

/// Test 1: wallet_service has no verification failures — issues must be empty.
#[test]
fn test_issues_empty_for_wallet_service() {
    let path = wallet_src_path();
    let verified = load_verified_from_path(&path).expect("wallet pipeline must succeed");
    let name = read_project_name(&wallet_root());
    let graph = serialize_graph(&verified, &name);

    assert!(
        graph.issues.is_empty(),
        "expected no issues for wallet_service, got: {:?}",
        graph.issues
    );
}

/// Test 2: hand-built IssueJson roundtrips through serde with optional fields.
#[test]
fn test_issues_roundtrip_hand_built() {
    let issue = IssueJson {
        node_id: "n1".to_string(),
        message: "boom".to_string(),
        stage: None,
        severity: Some("warn".to_string()),
        source: Some("rule".to_string()),
    };

    let json = serde_json::to_string(&issue).expect("serialize");
    let restored: IssueJson = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(issue, restored, "roundtrip must preserve equality");
    assert_eq!(restored.node_id, "n1");
    assert_eq!(restored.message, "boom");
    assert!(restored.stage.is_none(), "stage must be absent");
    assert_eq!(restored.severity.as_deref(), Some("warn"));
    assert_eq!(restored.source.as_deref(), Some("rule"));

    // Stage absent → must not appear in the serialized JSON.
    assert!(
        !json.contains("\"stage\""),
        "absent stage must not appear in JSON, got: {json}"
    );
}

/// Test 4: GraphJson with non-empty issues list roundtrips through serde.
#[test]
fn test_graph_json_with_issues_roundtrips() {
    let issue_fail = IssueJson {
        node_id: "mod1.fn1".to_string(),
        message: "balance invariant violated".to_string(),
        stage: None,
        severity: Some("fail".to_string()),
        source: Some("verify".to_string()),
    };
    let issue_warn = IssueJson {
        node_id: "mod1.fn2".to_string(),
        message: "unreachable branch".to_string(),
        stage: None,
        severity: Some("warn".to_string()),
        source: Some("rule".to_string()),
    };

    let graph = GraphJson {
        project: ProjectJson {
            id: "test".to_string(),
            name: "test".to_string(),
            description: String::new(),
            node_count: 0,
            module_count: 0,
            fn_count: 0,
            status: Status::Ok,
        },
        clusters: Vec::new(),
        modules: Vec::new(),
        externals: Vec::new(),
        relations: Vec::new(),
        types: Vec::new(),
        errors: Vec::new(),
        issues: vec![issue_fail.clone(), issue_warn.clone()],
        detail: BTreeMap::new(),
    };

    let json = serde_json::to_string(&graph).expect("serialize");
    let restored: GraphJson = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(restored, graph, "roundtrip must preserve equality");
    assert_eq!(restored.issues.len(), 2, "must restore 2 issues");
    assert_eq!(restored.issues[0], issue_fail, "first issue must match");
    assert_eq!(restored.issues[1], issue_warn, "second issue must match");
}

/// Test 3: collect_issues emits one issue per failing verification node.
#[test]
fn test_issues_populated_when_detail_has_failing_node() {
    let mut detail: BTreeMap<String, NodeDetail> = BTreeMap::new();

    detail.insert(
        "mod1.fn1".to_string(),
        NodeDetail {
            name: "fn1".to_string(),
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
                    scenario: "balance too low".to_string(),
                    effect: "negative balance".to_string(),
                    violates: "balance >= 0".to_string(),
                }),
            },
            code: None,
        },
    );

    // Add a passing node — must not appear in issues.
    detail.insert(
        "mod1.fn2".to_string(),
        NodeDetail {
            name: "fn2".to_string(),
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
            },
            code: None,
        },
    );

    let issues = collect_issues(&detail);

    assert_eq!(issues.len(), 1, "expected exactly 1 issue");
    let issue = &issues[0];
    assert_eq!(issue.node_id, "mod1.fn1");
    assert_eq!(issue.message, "balance >= 0");
    assert_eq!(issue.severity.as_deref(), Some("fail"));
    assert_eq!(issue.source.as_deref(), Some("verify"));
    assert!(issue.stage.is_none());
}
