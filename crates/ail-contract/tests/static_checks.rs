use ail_contract::{check_static_contracts, ContractError};
use ail_graph::{
    validate_graph, AilGraph, Contract, ContractKind, EdgeKind, Expression, Node, NodeId, Param,
    Pattern,
};
use ail_types::type_check;

// ─── Graph-building helpers ───────────────────────────────────────────────────

/// Wrap an AilGraph into a TypedGraph via validate → type_check.
/// Panics if either stage fails — tests only call this on known-valid graphs.
fn make_typed_graph(graph: AilGraph) -> ail_types::TypedGraph {
    let valid = validate_graph(graph).expect("validate_graph failed");
    type_check(valid, &[]).expect("type_check failed")
}

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

/// Build a minimal single-Do graph.
///
/// Structure:
/// ```
/// root (Describe, structural)
///   └─ do_node (Do, leaf, params=[amount:number], contracts=[before_expr, after_expr])
/// ```
fn single_do_graph(before_expr: &str, after_expr: &str) -> AilGraph {
    let mut graph = AilGraph::new();

    let mut root = Node::new(NodeId::new(), "root", Pattern::Describe);
    root.children = Some(vec![]);
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let mut do_node = Node::new(NodeId::new(), "do something with amount", Pattern::Do);
    do_node.metadata.params = vec![param("amount", "number")];
    do_node.metadata.return_type = Some("number".to_owned());
    do_node.contracts = vec![
        contract(ContractKind::Before, before_expr),
        contract(ContractKind::After, after_expr),
    ];
    let do_id = graph.add_node(do_node).unwrap();

    graph.add_edge(root_id, do_id, EdgeKind::Ev).unwrap();
    graph.get_node_mut(root_id).unwrap().children = Some(vec![do_id]);

    graph
}

/// Build a graph with a Do function that has a Raise child and an Error declaration.
///
/// Structure:
/// ```
/// root (Describe, structural)
///   ├─ error_node (Error, leaf, name="InsufficientFunds")
///   └─ do_node (Do, structural, params=[amount:number])
///        └─ raise_node (Raise, leaf, expression=raise_expr)
/// ```
///
/// When `wire_ed` is true, adds an Ed edge from `do_node` to `error_node` so
/// that "InsufficientFunds" is in the function's declared error list.
fn raise_graph(raise_expr: &str, wire_ed: bool) -> AilGraph {
    let mut graph = AilGraph::new();

    let mut root = Node::new(NodeId::new(), "root", Pattern::Describe);
    root.children = Some(vec![]);
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    // Error declaration node
    let mut error_node = Node::new(NodeId::new(), "insufficient funds error", Pattern::Error);
    error_node.metadata.name = Some("InsufficientFunds".to_owned());
    let error_id = graph.add_node(error_node).unwrap();

    // Do function (structural — has body)
    let mut do_node = Node::new(NodeId::new(), "transfer funds", Pattern::Do);
    do_node.metadata.params = vec![param("amount", "number")];
    do_node.metadata.return_type = Some("number".to_owned());
    do_node.contracts = vec![
        contract(ContractKind::Before, "amount > 0"),
        contract(ContractKind::After, "result >= 0"),
    ];
    do_node.children = Some(vec![]);
    let do_id = graph.add_node(do_node).unwrap();

    // Raise action (leaf inside the Do body)
    let mut raise_node = Node::new(
        NodeId::new(),
        "raise on insufficient balance",
        Pattern::Raise,
    );
    raise_node.expression = Some(Expression(raise_expr.to_owned()));
    let raise_id = graph.add_node(raise_node).unwrap();

    // Tree edges
    graph.add_edge(root_id, error_id, EdgeKind::Ev).unwrap();
    graph.add_edge(root_id, do_id, EdgeKind::Ev).unwrap();
    graph.add_edge(error_id, do_id, EdgeKind::Eh).unwrap();
    graph.add_edge(do_id, raise_id, EdgeKind::Ev).unwrap();

    graph.get_node_mut(root_id).unwrap().children = Some(vec![error_id, do_id]);
    graph.get_node_mut(do_id).unwrap().children = Some(vec![raise_id]);

    // Optional Ed edge: Do → Error (declares the error is part of this function's interface)
    if wire_ed {
        graph.add_edge(do_id, error_id, EdgeKind::Ed).unwrap();
    }

    graph
}

/// Build a graph with a template Do and an implementing Do.
///
/// The template has named phase children. The implementing Do may or may not
/// have all phases (controlled by `impl_phase_names`).
///
/// Structure:
/// ```
/// root (Describe, structural)
///   ├─ template_do (Do, structural, name="TemplateFunc", children=[phase nodes])
///   └─ impl_do    (Do, structural, name="ImplFunc",     children=[impl phase nodes])
///                  └─ Ed edge → template_do
/// ```
fn following_graph(template_phase_names: &[&str], impl_phase_names: &[&str]) -> AilGraph {
    let mut graph = AilGraph::new();

    let mut root = Node::new(NodeId::new(), "root", Pattern::Describe);
    root.children = Some(vec![]);
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    // Template Do
    let mut tmpl_do = Node::new(NodeId::new(), "template function", Pattern::Do);
    tmpl_do.metadata.name = Some("TemplateFunc".to_owned());
    tmpl_do.metadata.return_type = Some("number".to_owned());
    tmpl_do.contracts = vec![
        contract(ContractKind::Before, "true == true"),
        contract(ContractKind::After, "result >= 0"),
    ];
    tmpl_do.children = Some(vec![]);
    let tmpl_id = graph.add_node(tmpl_do).unwrap();

    // Template phase children
    let mut tmpl_child_ids = Vec::new();
    for (i, &phase_name) in template_phase_names.iter().enumerate() {
        let mut phase = Node::new(NodeId::new(), &format!("{phase_name} phase"), Pattern::Let);
        phase.metadata.name = Some(phase_name.to_owned());
        let phase_id = graph.add_node(phase).unwrap();
        graph.add_edge(tmpl_id, phase_id, EdgeKind::Ev).unwrap();
        if i > 0 {
            graph
                .add_edge(tmpl_child_ids[i - 1], phase_id, EdgeKind::Eh)
                .unwrap();
        }
        tmpl_child_ids.push(phase_id);
    }
    graph.get_node_mut(tmpl_id).unwrap().children = Some(tmpl_child_ids);

    // Implementing Do
    let mut impl_do = Node::new(NodeId::new(), "implementing function", Pattern::Do);
    impl_do.metadata.name = Some("ImplFunc".to_owned());
    impl_do.metadata.return_type = Some("number".to_owned());
    impl_do.contracts = vec![
        contract(ContractKind::Before, "true == true"),
        contract(ContractKind::After, "result >= 0"),
    ];
    impl_do.children = Some(vec![]);
    let impl_id = graph.add_node(impl_do).unwrap();

    // Implementing phase children
    let mut impl_child_ids = Vec::new();
    for (i, &phase_name) in impl_phase_names.iter().enumerate() {
        let mut phase = Node::new(
            NodeId::new(),
            &format!("{phase_name} implementation"),
            Pattern::Let,
        );
        phase.metadata.name = Some(phase_name.to_owned());
        let phase_id = graph.add_node(phase).unwrap();
        graph.add_edge(impl_id, phase_id, EdgeKind::Ev).unwrap();
        if i > 0 {
            graph
                .add_edge(impl_child_ids[i - 1], phase_id, EdgeKind::Eh)
                .unwrap();
        }
        impl_child_ids.push(phase_id);
    }
    graph.get_node_mut(impl_id).unwrap().children = Some(impl_child_ids);

    // Tree edges from root
    graph.add_edge(root_id, tmpl_id, EdgeKind::Ev).unwrap();
    graph.add_edge(root_id, impl_id, EdgeKind::Ev).unwrap();
    graph.add_edge(tmpl_id, impl_id, EdgeKind::Eh).unwrap();
    graph.get_node_mut(root_id).unwrap().children = Some(vec![tmpl_id, impl_id]);

    // Ed edge: impl_do → template_do (template following)
    graph.add_edge(impl_id, tmpl_id, EdgeKind::Ed).unwrap();

    graph
}

// ─── helpers for assertion ────────────────────────────────────────────────────

fn has_before_illegal_ref(errors: &[ContractError], illegal_ref: &str) -> bool {
    errors.iter().any(|e| {
        matches!(e, ContractError::BeforeContractIllegalRef { illegal_ref: r, .. } if r == illegal_ref)
    })
}

fn has_before_uses_old(errors: &[ContractError]) -> bool {
    errors
        .iter()
        .any(|e| matches!(e, ContractError::BeforeContractUsesOld { .. }))
}

fn has_after_illegal_ref(errors: &[ContractError], illegal_ref: &str) -> bool {
    errors.iter().any(|e| {
        matches!(e, ContractError::AfterContractIllegalRef { illegal_ref: r, .. } if r == illegal_ref)
    })
}

fn has_raise_unknown(errors: &[ContractError], error_name: &str) -> bool {
    errors.iter().any(
        |e| matches!(e, ContractError::RaiseUnknownError { error_name: n, .. } if n == error_name),
    )
}

fn has_parse_error(errors: &[ContractError]) -> bool {
    errors
        .iter()
        .any(|e| matches!(e, ContractError::ContractParseError { .. }))
}

// ─── Before-contract tests ────────────────────────────────────────────────────

#[test]
fn c001_before_contract_illegal_ref() {
    // "bad_var" is not a declared parameter — BeforeContractIllegalRef expected.
    let graph = single_do_graph("bad_var >= 0", "result >= 0");
    let typed = make_typed_graph(graph);
    let errors = check_static_contracts(&typed);
    assert!(
        has_before_illegal_ref(&errors, "bad_var"),
        "expected BeforeContractIllegalRef for 'bad_var'; got: {errors:?}"
    );
}

#[test]
fn c002_before_contract_uses_old() {
    // old() in a before-contract makes no sense — pre-state IS the state.
    let graph = single_do_graph("old(amount) > 0", "result >= 0");
    let typed = make_typed_graph(graph);
    let errors = check_static_contracts(&typed);
    assert!(
        has_before_uses_old(&errors),
        "expected BeforeContractUsesOld; got: {errors:?}"
    );
}

#[test]
fn c003_valid_before_contract_passes() {
    // "amount" is a declared parameter — no errors.
    let graph = single_do_graph("amount > 0", "result >= 0");
    let typed = make_typed_graph(graph);
    let errors = check_static_contracts(&typed);
    let before_errors: Vec<_> = errors
        .iter()
        .filter(|e| {
            matches!(
                e,
                ContractError::BeforeContractIllegalRef { .. }
                    | ContractError::BeforeContractUsesOld { .. }
            )
        })
        .collect();
    assert!(
        before_errors.is_empty(),
        "expected no before-contract errors; got: {before_errors:?}"
    );
}

// ─── After-contract tests ─────────────────────────────────────────────────────

#[test]
fn c004_after_contract_allows_result() {
    // "result" is always allowed in after-contracts.
    let graph = single_do_graph("amount > 0", "result >= 0");
    let typed = make_typed_graph(graph);
    let errors = check_static_contracts(&typed);
    let after_errors: Vec<_> = errors
        .iter()
        .filter(|e| matches!(e, ContractError::AfterContractIllegalRef { .. }))
        .collect();
    assert!(
        after_errors.is_empty(),
        "result should be allowed in after-contract; got: {after_errors:?}"
    );
}

#[test]
fn c005_after_contract_allows_old() {
    // old(amount) is valid in after-contracts — it snapshots pre-execution input.
    let graph = single_do_graph("amount > 0", "result >= old(amount)");
    let typed = make_typed_graph(graph);
    let errors = check_static_contracts(&typed);
    let after_errors: Vec<_> = errors
        .iter()
        .filter(|e| matches!(e, ContractError::AfterContractIllegalRef { .. }))
        .collect();
    assert!(
        after_errors.is_empty(),
        "old(amount) should be allowed in after-contract; got: {after_errors:?}"
    );
}

#[test]
fn c006_after_contract_illegal_ref() {
    // "bad_var" is neither a parameter nor "result" — AfterContractIllegalRef expected.
    let graph = single_do_graph("amount > 0", "bad_var >= 0");
    let typed = make_typed_graph(graph);
    let errors = check_static_contracts(&typed);
    assert!(
        has_after_illegal_ref(&errors, "bad_var"),
        "expected AfterContractIllegalRef for 'bad_var'; got: {errors:?}"
    );
}

#[test]
fn c007_valid_after_contract_passes() {
    // Parameters and "result" are allowed in after-contracts.
    let graph = single_do_graph("amount > 0", "result >= amount");
    let typed = make_typed_graph(graph);
    let errors = check_static_contracts(&typed);
    let after_errors: Vec<_> = errors
        .iter()
        .filter(|e| matches!(e, ContractError::AfterContractIllegalRef { .. }))
        .collect();
    assert!(
        after_errors.is_empty(),
        "amount and result should be allowed in after-contract; got: {after_errors:?}"
    );
}

// ─── After-contract: internal binding rejection ───────────────────────────────

#[test]
fn c014_after_contract_rejects_let_binding_ref() {
    // "new_balance" looks like an internal let binding, not a declared parameter.
    // Postconditions must only reference stable interface elements (params + result).
    let graph = single_do_graph("amount > 0", "new_balance >= 0");
    let typed = make_typed_graph(graph);
    let errors = check_static_contracts(&typed);
    assert!(
        has_after_illegal_ref(&errors, "new_balance"),
        "expected AfterContractIllegalRef for internal-binding-style ref 'new_balance'; got: {errors:?}"
    );
}

// ─── Raise error-reference tests ──────────────────────────────────────────────

#[test]
fn c008_raise_unknown_error() {
    // "UnknownError" is not declared by the enclosing function.
    let graph = raise_graph("UnknownError", false);
    let typed = make_typed_graph(graph);
    let errors = check_static_contracts(&typed);
    assert!(
        has_raise_unknown(&errors, "UnknownError"),
        "expected RaiseUnknownError for 'UnknownError'; got: {errors:?}"
    );
}

#[test]
fn c009_raise_known_error_passes() {
    // "InsufficientFunds" is declared via an Ed edge from the enclosing Do.
    let graph = raise_graph("InsufficientFunds", true);
    let typed = make_typed_graph(graph);
    let errors = check_static_contracts(&typed);
    let raise_errors: Vec<_> = errors
        .iter()
        .filter(|e| matches!(e, ContractError::RaiseUnknownError { .. }))
        .collect();
    assert!(
        raise_errors.is_empty(),
        "expected no raise errors for declared error; got: {raise_errors:?}"
    );
}

#[test]
fn c008b_raise_expression_with_carries_payload_still_extracts_name() {
    // When a raise expression includes a payload clause, only the first token
    // (the error type name) should be checked.
    // "UnknownError carries balance = amount" → error name = "UnknownError"
    let graph = raise_graph("UnknownError carries balance = amount", false);
    let typed = make_typed_graph(graph);
    let errors = check_static_contracts(&typed);
    assert!(
        has_raise_unknown(&errors, "UnknownError"),
        "expected RaiseUnknownError; the carries payload should not confuse name extraction; got: {errors:?}"
    );
}

// ─── Template-following tests ─────────────────────────────────────────────────

#[test]
fn c010_following_all_phases_present_produces_no_errors() {
    // When the implementing Do provides all phases required by the template,
    // no FollowingMissingPhase errors should be reported.
    //
    // Note: FollowingMissingPhase (AIL-C005) is also caught by Phase 1
    // validation rule v008. A TypedGraph can only be produced if all template
    // phases are already present, so this test verifies no false positives.
    let graph = following_graph(&["validate", "execute"], &["validate", "execute"]);
    let typed = make_typed_graph(graph);
    let errors = check_static_contracts(&typed);
    let following_errors: Vec<_> = errors
        .iter()
        .filter(|e| matches!(e, ContractError::FollowingMissingPhase { .. }))
        .collect();
    assert!(
        following_errors.is_empty(),
        "expected no following errors when all phases present; got: {following_errors:?}"
    );
}

#[test]
fn c011_following_no_template_refs_produces_no_errors() {
    // A Do node with no Ed edges produces no template-phase errors.
    let graph = single_do_graph("amount > 0", "result >= 0");
    let typed = make_typed_graph(graph);
    let errors = check_static_contracts(&typed);
    let following_errors: Vec<_> = errors
        .iter()
        .filter(|e| matches!(e, ContractError::FollowingMissingPhase { .. }))
        .collect();
    assert!(
        following_errors.is_empty(),
        "expected no following errors when no template refs; got: {following_errors:?}"
    );
}

// ─── Parse error handling ─────────────────────────────────────────────────────

#[test]
fn c012_contract_parse_error_reported() {
    // A contract expression that cannot be parsed emits ContractParseError
    // and does not panic. The invalid expression is in the before-contract position.
    let graph = single_do_graph("<<<totally invalid!!!>>>", "result >= 0");
    let typed = make_typed_graph(graph);
    let errors = check_static_contracts(&typed);
    assert!(
        has_parse_error(&errors),
        "expected ContractParseError for unparseable expression; got: {errors:?}"
    );
}

// ─── Quantifier variable exclusion ────────────────────────────────────────

#[test]
fn c015_quantifier_bound_variable_not_flagged_as_illegal_ref() {
    // `for all item in items, item.price > 0` — `item` is a quantifier-bound
    // variable, not a scope variable. It must NOT be flagged as an illegal ref.
    // `items` IS a parameter, so it is allowed as the collection source.
    let mut graph = AilGraph::new();

    let mut root = Node::new(NodeId::new(), "root", Pattern::Describe);
    root.children = Some(vec![]);
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let mut do_node = Node::new(NodeId::new(), "process order items", Pattern::Do);
    do_node.metadata.params = vec![param("items", "list")];
    do_node.metadata.return_type = Some("number".to_owned());
    do_node.contracts = vec![
        contract(ContractKind::Before, "items == items"),
        // After contract uses a ForAll quantifier: `item` is bound, not a scope var.
        contract(ContractKind::After, "for all item in items, item == item"),
    ];
    let do_id = graph.add_node(do_node).unwrap();

    graph.add_edge(root_id, do_id, EdgeKind::Ev).unwrap();
    graph.get_node_mut(root_id).unwrap().children = Some(vec![do_id]);

    let typed = make_typed_graph(graph);
    let errors = check_static_contracts(&typed);
    let after_errors: Vec<_> = errors
        .iter()
        .filter(|e| matches!(e, ContractError::AfterContractIllegalRef { .. }))
        .collect();
    assert!(
        after_errors.is_empty(),
        "quantifier-bound 'item' should not be flagged as illegal ref; got: {after_errors:?}"
    );
}

// ─── Integration: minimal valid graph ────────────────────────────────────────

#[test]
fn c013_no_errors_on_valid_do_with_param_contracts() {
    // A Do node with well-scoped contracts should produce zero contract errors.
    let graph = single_do_graph("amount > 0", "result >= amount");
    let typed = make_typed_graph(graph);
    let errors = check_static_contracts(&typed);
    assert!(
        errors.is_empty(),
        "expected no contract errors on well-formed Do; got: {errors:?}"
    );
}
