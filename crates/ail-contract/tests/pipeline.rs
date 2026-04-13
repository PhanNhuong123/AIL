//! Integration tests for the `verify(TypedGraph) → VerifiedGraph` pipeline
//! entry point and the `ContractSummary` breaking-change detection hooks.
//!
//! Run with:
//!   cargo test -p ail-contract                         # static path only
//!   cargo test -p ail-contract --features z3-verify    # full pipeline

use ail_contract::{
    verify, BreakingChange, ContractRecord, ContractStageError, ContractSummary, VerifiedGraph,
};
use ail_graph::{
    validate_graph, AilGraph, Contract, ContractKind, EdgeKind, Expression, Field, Node, NodeId,
    Param, Pattern,
};
use ail_types::{type_check, TypedGraph};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn param(name: &str, type_ref: &str) -> Param {
    Param {
        name: name.to_owned(),
        type_ref: type_ref.to_owned(),
    }
}

fn contract(kind: ContractKind, expr: &str) -> Contract {
    Contract {
        kind,
        expression: Expression(expr.to_owned()),
    }
}

fn field(name: &str, type_ref: &str) -> Field {
    Field {
        name: name.to_owned(),
        type_ref: type_ref.to_owned(),
    }
}

/// Build a minimal TypedGraph from a raw AilGraph. Panics in tests if the
/// graph fails validation or type-checking.
fn make_typed(graph: AilGraph) -> TypedGraph {
    let valid = validate_graph(graph).expect("test graph must be valid");
    type_check(valid, &[]).expect("test graph must type-check")
}

/// Build a TypedGraph containing only a structural root and no Do nodes.
fn empty_graph() -> TypedGraph {
    let mut graph = AilGraph::new();
    let root = Node::new(NodeId::new(), "root", Pattern::Describe);
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();
    make_typed(graph)
}

/// Build a TypedGraph with a single Do node under a structural Describe root.
///
/// `define_nodes` is a list of `(intent, name, base_type)` Define nodes that
/// are added before the Do node so type refs can be resolved. The Do node has
/// `params` and `contracts` as given.
fn single_do_graph(
    define_nodes: Vec<(&str, &str, &str)>,
    params: Vec<Param>,
    return_type: Option<&str>,
    contracts: Vec<Contract>,
) -> (TypedGraph, NodeId) {
    let mut graph = AilGraph::new();

    // Structural root
    let root_id = {
        let mut n = Node::new(NodeId::new(), "root", Pattern::Describe);
        n.children = Some(vec![]);
        let id = graph.add_node(n).unwrap();
        graph.set_root(id).unwrap();
        id
    };

    // Optional Define nodes for user-defined type refs
    for (intent, name, base_type) in define_nodes {
        let mut n = Node::new(NodeId::new(), intent, Pattern::Define);
        n.metadata.name = Some(name.to_owned());
        n.metadata.base_type = Some(base_type.to_owned());
        let def_id = graph.add_node(n).unwrap();
        graph.add_edge(root_id, def_id, EdgeKind::Ev).unwrap();
        graph
            .get_node_mut(root_id)
            .unwrap()
            .children
            .as_mut()
            .unwrap()
            .push(def_id);
    }

    // Do node
    let do_id = {
        let mut n = Node::new(NodeId::new(), "do test function", Pattern::Do);
        n.metadata.name = Some("test_fn".to_string());
        n.metadata.params = params;
        n.metadata.return_type = return_type.map(str::to_string);
        n.contracts = contracts;
        let id = graph.add_node(n).unwrap();
        graph.add_edge(root_id, id, EdgeKind::Ev).unwrap();
        graph
            .get_node_mut(root_id)
            .unwrap()
            .children
            .as_mut()
            .unwrap()
            .push(id);
        id
    };

    let typed = make_typed(graph);
    (typed, do_id)
}

/// Build a TypedGraph that models the wallet domain:
///
/// ```text
/// define WalletBalance : number
/// define PositiveAmount : number
/// describe User { id: text, balance: WalletBalance }
/// do transfer_money(sender_balance: WalletBalance, amount: PositiveAmount)
///    before amount > 0
///    after  sender_balance >= 0
/// ```
///
/// This exercises the full `validate_graph → type_check → verify` chain and is
/// the "full wallet-domain fixture" required by Task 3.4.
fn wallet_domain_typed_graph() -> TypedGraph {
    let mut graph = AilGraph::new();

    // Structural root
    let root_id = {
        let mut n = Node::new(NodeId::new(), "wallet service", Pattern::Describe);
        n.children = Some(vec![]);
        let id = graph.add_node(n).unwrap();
        graph.set_root(id).unwrap();
        id
    };

    // Define WalletBalance : number  (value >= 0)
    let wb_id = {
        let mut n = Node::new(NodeId::new(), "wallet balance type", Pattern::Define);
        n.metadata.name = Some("WalletBalance".to_owned());
        n.metadata.base_type = Some("number".to_owned());
        n.contracts = vec![contract(ContractKind::Always, "value >= 0")];
        graph.add_node(n).unwrap()
    };
    graph.add_edge(root_id, wb_id, EdgeKind::Ev).unwrap();
    graph
        .get_node_mut(root_id)
        .unwrap()
        .children
        .as_mut()
        .unwrap()
        .push(wb_id);

    // Define PositiveAmount : number  (value > 0)
    let pa_id = {
        let mut n = Node::new(NodeId::new(), "positive amount type", Pattern::Define);
        n.metadata.name = Some("PositiveAmount".to_owned());
        n.metadata.base_type = Some("number".to_owned());
        n.contracts = vec![contract(ContractKind::Always, "value > 0")];
        graph.add_node(n).unwrap()
    };
    graph.add_edge(root_id, pa_id, EdgeKind::Ev).unwrap();
    graph
        .get_node_mut(root_id)
        .unwrap()
        .children
        .as_mut()
        .unwrap()
        .push(pa_id);

    // Describe User { balance: WalletBalance }
    let user_id = {
        let mut n = Node::new(NodeId::new(), "user type", Pattern::Describe);
        n.metadata.name = Some("User".to_owned());
        n.metadata.fields = vec![field("balance", "WalletBalance")];
        graph.add_node(n).unwrap()
    };
    graph.add_edge(root_id, user_id, EdgeKind::Ev).unwrap();
    graph
        .get_node_mut(root_id)
        .unwrap()
        .children
        .as_mut()
        .unwrap()
        .push(user_id);

    // Do transfer_money(sender_balance: WalletBalance, amount: PositiveAmount)
    //   before sender_balance >= amount   (enough funds to transfer)
    //   before amount > 0                 (positive transfer amount)
    //   after  sender_balance >= 0        (balance non-negative after deduction)
    //
    // Entailment: sender_balance >= amount AND amount > 0  =>  sender_balance > 0 >= 0
    let do_id = {
        let mut n = Node::new(NodeId::new(), "transfer money function", Pattern::Do);
        n.metadata.name = Some("transfer_money".to_owned());
        n.metadata.params = vec![
            param("sender_balance", "WalletBalance"),
            param("amount", "PositiveAmount"),
        ];
        n.contracts = vec![
            contract(ContractKind::Before, "sender_balance >= amount"),
            contract(ContractKind::Before, "amount > 0"),
            contract(ContractKind::After, "sender_balance >= 0"),
        ];
        graph.add_node(n).unwrap()
    };
    graph.add_edge(root_id, do_id, EdgeKind::Ev).unwrap();
    graph
        .get_node_mut(root_id)
        .unwrap()
        .children
        .as_mut()
        .unwrap()
        .push(do_id);

    make_typed(graph)
}

// ── Pipeline entry point tests ────────────────────────────────────────────────

#[test]
fn pipeline_verify_empty_graph_returns_verified_graph() {
    let typed = empty_graph();
    let result = verify(typed);
    assert!(
        result.is_ok(),
        "graph with no Do nodes should produce VerifiedGraph; got {:?}",
        result.unwrap_err()
    );
}

#[test]
fn pipeline_verify_valid_do_node_returns_verified_graph() {
    let (typed, _) = single_do_graph(
        vec![],
        vec![param("n", "integer")],
        None,
        vec![
            contract(ContractKind::Before, "n > 0"),
            contract(ContractKind::After, "n > -1"),
        ],
    );
    let result = verify(typed);
    assert!(
        result.is_ok(),
        "valid Do node should produce VerifiedGraph; errors: {:?}",
        result.unwrap_err()
    );
}

#[test]
fn pipeline_verify_static_error_aborts_before_z3() {
    // `old()` in a Before contract is a static error (AIL-C002).
    // Even with z3-verify enabled, Z3 must NOT be invoked — static errors
    // short-circuit the pipeline.
    let (typed, _) = single_do_graph(
        vec![],
        vec![param("n", "integer")],
        None,
        vec![
            contract(ContractKind::Before, "old(n) > 0"), // invalid: old() in before
            contract(ContractKind::After, "n > 0"),
        ],
    );
    let result = verify(typed);
    assert!(
        result.is_err(),
        "old() in before contract must be a static error"
    );

    let errors = result.unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ContractStageError::StaticCheck(_))),
        "error must be StaticCheck, not Z3Verify; errors: {errors:?}"
    );
}

#[test]
fn pipeline_verify_wallet_domain_fixture_passes() {
    // Full wallet-domain graph: WalletBalance, PositiveAmount, User,
    // transfer_money with amount > 0 / sender_balance >= 0.
    let typed = wallet_domain_typed_graph();
    let result = verify(typed);
    assert!(
        result.is_ok(),
        "wallet domain fixture should pass verify; errors: {:?}",
        result.unwrap_err()
    );
}

#[test]
fn pipeline_verified_graph_exposes_graph_and_typed() {
    let typed = empty_graph();
    let verified: VerifiedGraph = verify(typed).unwrap();

    // graph() delegates to typed inner
    let node_count = verified.graph().node_count();
    assert_eq!(node_count, 1, "empty graph has 1 node (the root)");

    // typed() exposes inner TypedGraph
    assert_eq!(verified.typed().graph().node_count(), node_count);

    // into_inner() consumes and returns TypedGraph
    let recovered = verified.into_inner();
    assert_eq!(recovered.graph().node_count(), node_count);
}

// ── ContractSummary tests ─────────────────────────────────────────────────────

#[test]
fn pipeline_contract_summary_empty_for_no_do_nodes() {
    let typed = empty_graph();
    let verified = verify(typed).unwrap();
    let summary = verified.contract_summary();
    assert!(
        summary.entries.is_empty(),
        "graph with no Do nodes should have an empty summary"
    );
}

#[test]
fn pipeline_contract_summary_captures_do_functions() {
    let (typed, _) = single_do_graph(
        vec![],
        vec![param("n", "integer")],
        None,
        vec![
            contract(ContractKind::Before, "n > 0"),
            contract(ContractKind::After, "n > -1"),
        ],
    );
    let verified = verify(typed).unwrap();
    let summary = verified.contract_summary();

    assert!(
        summary.entries.contains_key("test_fn"),
        "summary should contain 'test_fn'; entries: {:?}",
        summary.entries.keys().collect::<Vec<_>>()
    );

    let record = &summary.entries["test_fn"];
    assert_eq!(record.before, vec!["n > 0".to_string()]);
    assert_eq!(record.after, vec!["n > -1".to_string()]);
    assert!(record.always.is_empty());
}

#[test]
fn pipeline_contract_summary_cached_on_repeated_calls() {
    let typed = wallet_domain_typed_graph();
    let verified = verify(typed).unwrap();

    // Call twice — must return the same data (cached via OnceLock).
    let summary1 = verified.contract_summary();
    let summary2 = verified.contract_summary();
    assert_eq!(summary1, summary2);
}

// ── Breaking change detection tests ──────────────────────────────────────────

/// Build a minimal `ContractSummary` with a single function entry.
fn make_summary(
    fn_name: &str,
    before: Vec<&str>,
    after: Vec<&str>,
    always: Vec<&str>,
) -> ContractSummary {
    use std::collections::BTreeMap;

    let record = ContractRecord {
        node_id: NodeId::new(),
        before: before.into_iter().map(str::to_string).collect(),
        after: after.into_iter().map(str::to_string).collect(),
        always: always.into_iter().map(str::to_string).collect(),
    };
    let mut entries = BTreeMap::new();
    entries.insert(fn_name.to_string(), record);
    ContractSummary { entries }
}

#[test]
fn pipeline_breaking_changes_no_change_when_equal() {
    let baseline = make_summary("transfer", vec!["amount > 0"], vec!["balance >= 0"], vec![]);
    let new = make_summary("transfer", vec!["amount > 0"], vec!["balance >= 0"], vec![]);

    let changes = new.breaking_changes(&baseline);
    assert!(
        changes.is_empty(),
        "identical summaries should have no breaking changes; got: {changes:?}"
    );
}

#[test]
fn pipeline_breaking_changes_detects_removed_function() {
    let baseline = make_summary("transfer", vec!["amount > 0"], vec!["balance >= 0"], vec![]);
    // new summary has NO entry for "transfer"
    let new = ContractSummary {
        entries: std::collections::BTreeMap::new(),
    };

    let changes = new.breaking_changes(&baseline);
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, BreakingChange::FunctionRemoved { name } if name == "transfer")),
        "removed function should be flagged; changes: {changes:?}"
    );
}

#[test]
fn pipeline_breaking_changes_detects_removed_contract() {
    let baseline = make_summary(
        "transfer",
        vec!["amount > 0", "amount < 1000"],
        vec!["balance >= 0"],
        vec![],
    );
    // new drops "amount < 1000" from before
    let new = make_summary("transfer", vec!["amount > 0"], vec!["balance >= 0"], vec![]);

    let changes = new.breaking_changes(&baseline);
    assert!(
        changes.iter().any(|c| matches!(c,
            BreakingChange::ContractRemoved { function, kind, expr }
            if function == "transfer" && kind == "before" && expr == "amount < 1000"
        )),
        "removed before contract should be flagged; changes: {changes:?}"
    );
}

#[test]
fn pipeline_breaking_changes_detects_added_contract() {
    let baseline = make_summary("transfer", vec!["amount > 0"], vec!["balance >= 0"], vec![]);
    // new adds "amount < 1000" to before
    let new = make_summary(
        "transfer",
        vec!["amount > 0", "amount < 1000"],
        vec!["balance >= 0"],
        vec![],
    );

    let changes = new.breaking_changes(&baseline);
    assert!(
        changes.iter().any(|c| matches!(c,
            BreakingChange::ContractAdded { function, kind, expr }
            if function == "transfer" && kind == "before" && expr == "amount < 1000"
        )),
        "added before contract should be flagged; changes: {changes:?}"
    );
}

#[test]
fn pipeline_breaking_changes_new_function_is_not_breaking() {
    let baseline = make_summary("transfer", vec!["amount > 0"], vec!["balance >= 0"], vec![]);
    // new adds a second function — this is additive, not breaking
    let mut new = make_summary("transfer", vec!["amount > 0"], vec!["balance >= 0"], vec![]);
    let extra = ContractRecord {
        node_id: NodeId::new(),
        before: vec!["id > 0".to_string()],
        after: vec!["user is valid".to_string()],
        always: vec![],
    };
    new.entries.insert("get_user".to_string(), extra);

    let changes = new.breaking_changes(&baseline);
    assert!(
        !changes
            .iter()
            .any(|c| matches!(c, BreakingChange::FunctionRemoved { .. })),
        "adding a new function should NOT be breaking; changes: {changes:?}"
    );
    assert!(
        changes.is_empty(),
        "adding a new function should produce no breaking changes; changes: {changes:?}"
    );
}

#[test]
fn pipeline_breaking_changes_whitespace_difference_is_flagged() {
    // v0.1 limitation: string equality means whitespace differences are flagged.
    // This test documents the known behaviour rather than asserting it is correct.
    let baseline = make_summary("transfer", vec!["amount > 0"], vec!["balance >= 0"], vec![]);
    // Same semantics, extra spaces
    let new_with_spaces = make_summary(
        "transfer",
        vec!["amount  >  0"], // extra spaces — same meaning, different string
        vec!["balance >= 0"],
        vec![],
    );

    let changes = new_with_spaces.breaking_changes(&baseline);
    // Document v0.1 behaviour: flagged as both a removal and an addition.
    assert!(
        !changes.is_empty(),
        "v0.1: whitespace difference is currently flagged as a breaking change"
    );
    // Ensure both ContractRemoved (old string) and ContractAdded (new string) appear.
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, BreakingChange::ContractRemoved { .. })),
        "old string should be flagged as ContractRemoved"
    );
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, BreakingChange::ContractAdded { .. })),
        "new string should be flagged as ContractAdded"
    );
}
