use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ail_ui_bridge::lens::compute_lens_metrics;
use ail_ui_bridge::pipeline::{load_verified_from_path, read_project_name};
use ail_ui_bridge::serialize::serialize_graph;
use ail_ui_bridge::types::graph_json::{
    FunctionJson, GraphJson, ModuleJson, ProjectJson, StepJson,
};
use ail_ui_bridge::types::lens_stats::{Lens, LensStats};
use ail_ui_bridge::types::node_detail::CounterexampleDetail;
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

fn empty_graph() -> GraphJson {
    GraphJson {
        project: ProjectJson {
            id: "empty".to_string(),
            name: "empty".to_string(),
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
        issues: Vec::new(),
        detail: BTreeMap::new(),
    }
}

fn graph_with_typed_fn() -> GraphJson {
    use ail_ui_bridge::types::node_detail::{
        NodeDetail, ReceivesEntry, ReturnsEntry, VerificationDetail,
    };

    let step = StepJson {
        id: "mod1.fn1.step1".to_string(),
        name: "step1".to_string(),
        status: Status::Ok,
        intent: "check balance".to_string(),
        branch: None,
    };

    let function = FunctionJson {
        id: "mod1.fn1".to_string(),
        name: "fn1".to_string(),
        status: Status::Ok,
        steps: Some(vec![step.clone()]),
    };

    let module = ModuleJson {
        id: "mod1".to_string(),
        name: "mod1".to_string(),
        description: String::new(),
        cluster: "default".to_string(),
        cluster_name: "test".to_string(),
        cluster_color: "#2997ff".to_string(),
        status: Status::Ok,
        node_count: 3,
        functions: vec![function.clone()],
    };

    let mut detail = BTreeMap::new();
    detail.insert(
        "mod1.fn1".to_string(),
        NodeDetail {
            name: "fn1".to_string(),
            status: Status::Ok,
            description: String::new(),
            receives: vec![
                ReceivesEntry {
                    name: "amount".to_string(),
                    desc: "Money".to_string(),
                },
                // Duplicate type intentionally — should be deduplicated
                ReceivesEntry {
                    name: "fee".to_string(),
                    desc: "Money".to_string(),
                },
            ],
            returns: vec![ReturnsEntry {
                name: "Balance".to_string(),
                desc: String::new(),
            }],
            rules: Vec::new(),
            inherited: Vec::new(),
            proven: Vec::new(),
            verification: VerificationDetail {
                ok: true,
                counterexample: None,
                outcome: None,
            },
            code: None,
        },
    );
    detail.insert(
        "mod1.fn1.step1".to_string(),
        NodeDetail {
            name: "step1".to_string(),
            status: Status::Ok,
            description: String::new(),
            receives: vec![ReceivesEntry {
                name: "x".to_string(),
                desc: "Money".to_string(), // duplicate of above
            }],
            returns: vec![ReturnsEntry {
                name: "Result".to_string(),
                desc: String::new(),
            }],
            rules: Vec::new(),
            inherited: Vec::new(),
            proven: Vec::new(),
            verification: VerificationDetail {
                ok: true,
                counterexample: None,
                outcome: None,
            },
            code: None,
        },
    );

    GraphJson {
        project: ProjectJson {
            id: "test".to_string(),
            name: "test".to_string(),
            description: String::new(),
            node_count: 3,
            module_count: 1,
            fn_count: 1,
            status: Status::Ok,
        },
        clusters: Vec::new(),
        modules: vec![module],
        externals: Vec::new(),
        relations: Vec::new(),
        types: Vec::new(),
        errors: Vec::new(),
        issues: Vec::new(),
        detail,
    }
}

/// Test 1: verify lens project scope counts rollup on wallet_service.
#[test]
fn test_lens_verify_project_scope_counts_rollup() {
    let path = wallet_src_path();
    let verified = load_verified_from_path(&path).expect("wallet pipeline must succeed");
    let name = read_project_name(&wallet_root());
    let graph = serialize_graph(&verified, &name);

    let stats = compute_lens_metrics(&graph, Lens::Verify, None);

    match stats {
        LensStats::Verify {
            proven,
            unproven,
            counterexamples,
        } => {
            assert!(proven > 0, "expected proven > 0, got {proven}");
            assert_eq!(unproven, 0, "expected unproven == 0, got {unproven}");
            assert_eq!(
                counterexamples, 0,
                "expected counterexamples == 0, got {counterexamples}"
            );
        }
        other => panic!("expected LensStats::Verify, got {other:?}"),
    }
}

/// Test 2: empty graph returns zero Rules variant.
#[test]
fn test_lens_rules_with_empty_graph() {
    let graph = empty_graph();
    let stats = compute_lens_metrics(&graph, Lens::Rules, None);
    assert_eq!(stats, LensStats::zero(Lens::Rules));
    match stats {
        LensStats::Rules {
            total,
            unproven,
            broken,
        } => {
            assert_eq!(total, 0);
            assert_eq!(unproven, 0);
            assert_eq!(broken, 0);
        }
        other => panic!("expected LensStats::Rules, got {other:?}"),
    }
}

/// Test 3: data lens deduplicates and sorts types across scope.
#[test]
fn test_lens_data_function_scope_dedupes_types() {
    let graph = graph_with_typed_fn();
    // scope = the function
    let stats = compute_lens_metrics(&graph, Lens::Data, Some("mod1.fn1"));

    match stats {
        LensStats::Data { types, signals } => {
            // Receives: Money (×2 deduped), Balance, Result from fn + step
            // In BTreeSet order: Balance, Money, Result
            assert!(
                types.contains(&"Money".to_string()),
                "expected 'Money' in types, got: {types:?}"
            );
            // Deduplication: Money should appear only once
            let money_count = types.iter().filter(|t| t.as_str() == "Money").count();
            assert_eq!(money_count, 1, "Money should be deduplicated");
            // Sorted (BTreeSet guarantees alphabetical order)
            let mut sorted = types.clone();
            sorted.sort();
            assert_eq!(types, sorted, "types must be sorted");
            // No branches in this function
            assert_eq!(signals, 0, "expected 0 signals (no branch steps)");
        }
        other => panic!("expected LensStats::Data, got {other:?}"),
    }
}

/// Test 4: structure lens with step scope returns steps=1, nodes=1.
#[test]
fn test_lens_structure_step_scope() {
    let graph = graph_with_typed_fn();
    let stats = compute_lens_metrics(&graph, Lens::Structure, Some("mod1.fn1.step1"));

    assert_eq!(
        stats,
        LensStats::Structure {
            modules: 0,
            functions: 0,
            steps: 1,
            nodes: 1,
        }
    );
}

/// Test 5: tests lens returns zero placeholder.
#[test]
fn test_lens_tests_returns_zero_placeholder() {
    let graph = empty_graph();
    let stats = compute_lens_metrics(&graph, Lens::Tests, None);
    assert_eq!(
        stats,
        LensStats::Tests {
            total: 0,
            passing: 0,
            failing: 0,
        }
    );
}

/// Test 6: unknown scope returns zero stats.
#[test]
fn test_lens_scope_unknown_returns_zero() {
    let graph = empty_graph();
    let stats = compute_lens_metrics(&graph, Lens::Verify, Some("does-not-exist"));
    assert_eq!(stats, LensStats::zero(Lens::Verify));
}

/// Test 8: synthetic graph with one failing node + counterexample reaches
/// `counterexamples == 1`, `unproven == 1`, `proven == 0` in Verify lens.
#[test]
fn test_lens_verify_counterexample_reaches_counter() {
    use ail_ui_bridge::types::node_detail::{NodeDetail, VerificationDetail, VerifyOutcome};

    let mut detail = BTreeMap::new();
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
                    scenario: "balance is negative".to_string(),
                    effect: "overdraft".to_string(),
                    violates: "balance >= 0".to_string(),
                }),
                outcome: Some(VerifyOutcome::Unsat),
            },
            code: None,
        },
    );

    let module = ModuleJson {
        id: "mod1".to_string(),
        name: "mod1".to_string(),
        description: String::new(),
        cluster: "default".to_string(),
        cluster_name: "test".to_string(),
        cluster_color: "#2997ff".to_string(),
        status: Status::Fail,
        node_count: 1,
        functions: vec![FunctionJson {
            id: "mod1.fn1".to_string(),
            name: "fn1".to_string(),
            status: Status::Fail,
            steps: None,
        }],
    };

    let graph = GraphJson {
        project: ProjectJson {
            id: "test".to_string(),
            name: "test".to_string(),
            description: String::new(),
            node_count: 1,
            module_count: 1,
            fn_count: 1,
            status: Status::Fail,
        },
        clusters: Vec::new(),
        modules: vec![module],
        externals: Vec::new(),
        relations: Vec::new(),
        types: Vec::new(),
        errors: Vec::new(),
        issues: Vec::new(),
        detail,
    };

    let stats = compute_lens_metrics(&graph, Lens::Verify, None);
    match stats {
        LensStats::Verify {
            proven,
            unproven,
            counterexamples,
        } => {
            assert_eq!(proven, 0, "expected proven == 0, got {proven}");
            assert_eq!(unproven, 1, "expected unproven == 1, got {unproven}");
            assert_eq!(
                counterexamples, 1,
                "expected counterexamples == 1, got {counterexamples}"
            );
        }
        other => panic!("expected LensStats::Verify, got {other:?}"),
    }
}

/// Test 7: serde roundtrip for all LensStats variants preserves the `lens` tag.
#[test]
fn test_lens_stats_serde_all_variants() {
    let variants: Vec<LensStats> = vec![
        LensStats::Structure {
            modules: 2,
            functions: 5,
            steps: 12,
            nodes: 20,
        },
        LensStats::Rules {
            total: 3,
            unproven: 1,
            broken: 0,
        },
        LensStats::Verify {
            proven: 7,
            unproven: 0,
            counterexamples: 0,
        },
        LensStats::Data {
            types: vec!["Money".to_string(), "UserId".to_string()],
            signals: 2,
        },
        LensStats::Tests {
            total: 0,
            passing: 0,
            failing: 0,
        },
    ];

    let expected_tags = ["structure", "rules", "verify", "data", "tests"];

    for (variant, expected_tag) in variants.iter().zip(expected_tags.iter()) {
        let json = serde_json::to_string(variant)
            .unwrap_or_else(|e| panic!("serialize failed for {variant:?}: {e}"));

        // Check the `"lens":"<tag>"` discriminator is present.
        let tag_pattern = format!("\"lens\":\"{expected_tag}\"");
        assert!(
            json.contains(&tag_pattern),
            "expected JSON to contain {tag_pattern}, got: {json}"
        );

        // Deserialize and compare.
        let restored: LensStats = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("deserialize failed for {json}: {e}"));
        assert_eq!(
            variant, &restored,
            "roundtrip must preserve equality for {expected_tag}"
        );
    }
}
