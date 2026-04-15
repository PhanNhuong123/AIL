use ail_graph::{
    compute_context_packet_for_backend, AilGraph, ContractKind, EdgeKind, GraphError, NodeId,
    Pattern, ScopeVariableKind,
};

mod helpers;
use helpers::{
    add_contract, add_field, add_param, make_child, make_node, make_sibling_after, set_return_type,
};

// ─── Rule 1 DOWN ───────────────────────────────────────────────────────────

#[test]
fn cic_down_root_has_no_inherited_constraints() {
    let mut graph = AilGraph::new();
    let root = make_node(&mut graph, Pattern::Do, "root", Some("root"));
    add_contract(&mut graph, root, ContractKind::Before, "own_pre");
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(root).unwrap();
    // Root has no ancestors, and its own contracts are not part of
    // inherited_constraints per spec Rule 1 DOWN.
    assert!(packet.inherited_constraints.is_empty());
}

#[test]
fn cic_down_child_inherits_parent_contract() {
    let mut graph = AilGraph::new();
    let root = make_node(&mut graph, Pattern::Do, "transfer", Some("transfer"));
    add_contract(&mut graph, root, ContractKind::Before, "amount > 0");
    let child = make_child(&mut graph, root, Pattern::Do, "validate", Some("validate"));
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(child).unwrap();
    assert_eq!(packet.inherited_constraints.len(), 1);
    assert_eq!(packet.inherited_constraints[0].expression.0, "amount > 0");
    assert_eq!(packet.inherited_constraints[0].origin_node, root);
}

#[test]
fn cic_down_grandchild_inherits_full_ancestor_chain_in_root_first_order() {
    let mut graph = AilGraph::new();
    let root = make_node(&mut graph, Pattern::Do, "root", None);
    add_contract(&mut graph, root, ContractKind::Before, "pre_root");
    let mid = make_child(&mut graph, root, Pattern::Do, "mid", None);
    add_contract(&mut graph, mid, ContractKind::Before, "pre_mid");
    let leaf = make_child(&mut graph, mid, Pattern::Do, "leaf", None);
    add_contract(&mut graph, leaf, ContractKind::Before, "pre_leaf_own");
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(leaf).unwrap();
    // inherited = root + mid (leaf's own contracts excluded), root-first order.
    let exprs: Vec<&str> = packet
        .inherited_constraints
        .iter()
        .map(|c| c.expression.0.as_str())
        .collect();
    assert_eq!(exprs, vec!["pre_root", "pre_mid"]);
}

#[test]
fn cic_down_inherited_constraint_carries_origin_node_id() {
    let mut graph = AilGraph::new();
    let root = make_node(&mut graph, Pattern::Do, "root", None);
    add_contract(&mut graph, root, ContractKind::Always, "invariant");
    let child = make_child(&mut graph, root, Pattern::Do, "child", None);
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(child).unwrap();
    assert_eq!(packet.inherited_constraints.len(), 1);
    assert_eq!(packet.inherited_constraints[0].origin_node, root);
}

// ─── intent_chain ──────────────────────────────────────────────────────────

#[test]
fn cic_intent_chain_root_is_single_entry() {
    let mut graph = AilGraph::new();
    let root = make_node(&mut graph, Pattern::Do, "root_intent", None);
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(root).unwrap();
    assert_eq!(packet.intent_chain, vec!["root_intent".to_string()]);
}

#[test]
fn cic_intent_chain_grandchild_is_root_to_current() {
    let mut graph = AilGraph::new();
    let root = make_node(&mut graph, Pattern::Do, "root", None);
    let mid = make_child(&mut graph, root, Pattern::Do, "mid", None);
    let leaf = make_child(&mut graph, mid, Pattern::Do, "leaf", None);
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(leaf).unwrap();
    assert_eq!(
        packet.intent_chain,
        vec!["root".to_string(), "mid".to_string(), "leaf".to_string()]
    );
}

// ─── Rule 3 ACROSS ─────────────────────────────────────────────────────────

#[test]
fn cic_across_first_sibling_receives_no_cross_scope() {
    let mut graph = AilGraph::new();
    let root = make_node(&mut graph, Pattern::Do, "root", None);
    let first = make_child(&mut graph, root, Pattern::Do, "first", None);
    // Second sibling exists (after first), but we query first, which has no
    // prev sibling and therefore should not pick anything up via ACROSS.
    let _second = make_sibling_after(&mut graph, first, root, Pattern::Let, "let x", Some("x"));
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(first).unwrap();
    assert!(
        packet.scope.is_empty(),
        "expected empty scope, got {:?}",
        packet.scope
    );
}

#[test]
fn cic_across_next_sibling_receives_prev_let_binding() {
    let mut graph = AilGraph::new();
    let root = make_node(&mut graph, Pattern::Do, "root", None);
    let let_node = make_child(
        &mut graph,
        root,
        Pattern::Let,
        "compute",
        Some("new_balance"),
    );
    let next = make_sibling_after(&mut graph, let_node, root, Pattern::Do, "next", None);
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(next).unwrap();
    assert_eq!(packet.scope.len(), 1);
    assert_eq!(packet.scope[0].name, "new_balance");
    assert_eq!(packet.scope[0].kind, ScopeVariableKind::LetBinding);
    assert_eq!(packet.scope[0].origin_node, let_node);
}

#[test]
fn cic_across_next_sibling_receives_prev_fetch_binding() {
    let mut graph = AilGraph::new();
    let root = make_node(&mut graph, Pattern::Do, "root", None);
    let fetch_node = make_child(&mut graph, root, Pattern::Fetch, "fetch user", Some("user"));
    let next = make_sibling_after(&mut graph, fetch_node, root, Pattern::Do, "next", None);
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(next).unwrap();
    assert_eq!(packet.scope.len(), 1);
    assert_eq!(packet.scope[0].name, "user");
    assert_eq!(packet.scope[0].kind, ScopeVariableKind::FetchResult);
}

#[test]
fn cic_across_chain_of_three_siblings_accumulates_scope_in_order() {
    let mut graph = AilGraph::new();
    let root = make_node(&mut graph, Pattern::Do, "root", None);
    let a = make_child(&mut graph, root, Pattern::Let, "let a", Some("a"));
    let b = make_sibling_after(&mut graph, a, root, Pattern::Let, "let b", Some("b"));
    let c = make_sibling_after(&mut graph, b, root, Pattern::Do, "c", None);
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(c).unwrap();
    let names: Vec<&str> = packet.scope.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, vec!["a", "b"]);
}

#[test]
fn cic_across_nested_node_sees_uncle_let_binding() {
    // Graph:
    //   root (do transfer_money)
    //   └── execute (do)
    //       ├── new_balance (let)
    //       └── persist (do)            ← sibling of new_balance inside execute
    //           └── save_sender (save)  ← TARGET, depth 3
    //
    // save_sender should see `new_balance` through the depth-aware walk:
    // at ancestor level = persist, prev_sibling_of(persist) = new_balance.
    let mut graph = AilGraph::new();
    let root = make_node(&mut graph, Pattern::Do, "transfer_money", None);
    let execute = make_child(&mut graph, root, Pattern::Do, "execute", None);
    let new_balance = make_child(
        &mut graph,
        execute,
        Pattern::Let,
        "compute new balance",
        Some("new_balance"),
    );
    let persist = make_sibling_after(
        &mut graph,
        new_balance,
        execute,
        Pattern::Do,
        "persist",
        None,
    );
    let save_sender = make_child(&mut graph, persist, Pattern::Save, "save sender", None);
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(save_sender).unwrap();
    let names: Vec<&str> = packet.scope.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["new_balance"],
        "expected uncle let-binding visible, got {:?}",
        packet.scope
    );
    assert_eq!(packet.scope[0].origin_node, new_balance);
}

// ─── Rule 4 DIAGONAL ───────────────────────────────────────────────────────

#[test]
fn cic_diagonal_type_constraint_resolved_from_scope_variable() {
    // Prove the scope-variable mechanism independent of Ed edges.
    let mut graph = AilGraph::new();
    let age_type = make_node(&mut graph, Pattern::Define, "Age type", Some("Age"));
    add_contract(&mut graph, age_type, ContractKind::Always, "value >= 0");
    let root = make_node(&mut graph, Pattern::Do, "root", None);
    add_param(&mut graph, root, "n", "Age");
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(root).unwrap();
    assert_eq!(packet.type_constraints.len(), 1);
    assert_eq!(packet.type_constraints[0].expression.0, "value >= 0");
    assert_eq!(packet.type_constraints[0].origin_node, age_type);
}

#[test]
fn cic_diagonal_call_contract_from_manually_wired_ed_edge() {
    let mut graph = AilGraph::new();
    let caller = make_node(&mut graph, Pattern::Do, "caller", Some("caller"));
    let callee = make_node(&mut graph, Pattern::Do, "callee", Some("callee"));
    add_contract(&mut graph, callee, ContractKind::Before, "pre");
    add_contract(&mut graph, callee, ContractKind::After, "post");
    // Hand-wire the Ed edge the parser will eventually emit.
    graph.add_edge(caller, callee, EdgeKind::Ed).unwrap();
    graph.set_root(caller).unwrap();

    let packet = graph.compute_context_packet(caller).unwrap();
    assert_eq!(packet.call_contracts.len(), 2);
    let exprs: Vec<&str> = packet
        .call_contracts
        .iter()
        .map(|c| c.expression.0.as_str())
        .collect();
    assert!(exprs.contains(&"pre"));
    assert!(exprs.contains(&"post"));
    assert!(packet
        .call_contracts
        .iter()
        .all(|c| c.origin_node == callee));
}

#[test]
fn cic_diagonal_node_with_no_ed_edges_and_no_typed_scope_has_empty_diagonal_fields() {
    let mut graph = AilGraph::new();
    let root = make_node(&mut graph, Pattern::Do, "root", None);
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(root).unwrap();
    assert!(packet.type_constraints.is_empty());
    assert!(packet.call_contracts.is_empty());
    assert!(packet.template_constraints.is_empty());
}

#[test]
fn cic_diagonal_constraint_carries_origin_of_the_target_node() {
    let mut graph = AilGraph::new();
    let age_type = make_node(&mut graph, Pattern::Define, "Age type", Some("Age"));
    add_contract(&mut graph, age_type, ContractKind::Always, "age >= 0");
    let root = make_node(&mut graph, Pattern::Do, "root", None);
    add_param(&mut graph, root, "a", "Age");
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(root).unwrap();
    assert_eq!(packet.type_constraints.len(), 1);
    assert_eq!(packet.type_constraints[0].origin_node, age_type);
}

// ─── recursive type unfolding ──────────────────────────────────────────────

#[test]
fn cic_recursive_type_unfolds_user_balance_walletbalance_chain() {
    // Flagship example from the spec:
    //   define WalletBalance:number where value >= 0
    //   describe User as balance: WalletBalance
    //   do transfer(sender: User)
    // Expected: type_constraints contains `value >= 0` via recursive unfold.
    let mut graph = AilGraph::new();
    let wb = make_node(
        &mut graph,
        Pattern::Define,
        "WalletBalance",
        Some("WalletBalance"),
    );
    add_contract(&mut graph, wb, ContractKind::Always, "value >= 0");
    let user = make_node(&mut graph, Pattern::Describe, "User", Some("User"));
    add_field(&mut graph, user, "balance", "WalletBalance");
    let transfer = make_node(&mut graph, Pattern::Do, "transfer", Some("transfer"));
    add_param(&mut graph, transfer, "sender", "User");
    graph.set_root(transfer).unwrap();

    let packet = graph.compute_context_packet(transfer).unwrap();
    assert_eq!(
        packet.type_constraints.len(),
        1,
        "expected 1 unfolded type constraint, got {:?}",
        packet.type_constraints
    );
    assert_eq!(packet.type_constraints[0].expression.0, "value >= 0");
    assert_eq!(packet.type_constraints[0].origin_node, wb);
}

#[test]
fn cic_recursive_type_stops_when_field_type_has_no_constraint() {
    // User has a field `id: string`. "string" is not a defined type in this
    // graph, so the recursive walker simply skips it. Expected: 0 type
    // constraints.
    let mut graph = AilGraph::new();
    let user = make_node(&mut graph, Pattern::Describe, "User", Some("User"));
    add_field(&mut graph, user, "id", "string");
    let root = make_node(&mut graph, Pattern::Do, "root", None);
    add_param(&mut graph, root, "u", "User");
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(root).unwrap();
    assert!(packet.type_constraints.is_empty());
}

#[test]
fn cic_recursive_type_cycle_does_not_loop_forever() {
    // describe User as friend: User — self-referential type. The visited
    // set must prevent infinite recursion, and User's own contracts must
    // still be collected once.
    let mut graph = AilGraph::new();
    let user = make_node(&mut graph, Pattern::Describe, "User", Some("User"));
    add_contract(&mut graph, user, ContractKind::Always, "valid_user");
    add_field(&mut graph, user, "friend", "User");
    let root = make_node(&mut graph, Pattern::Do, "root", None);
    add_param(&mut graph, root, "u", "User");
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(root).unwrap();
    assert_eq!(packet.type_constraints.len(), 1);
    assert_eq!(packet.type_constraints[0].expression.0, "valid_user");
}

// ─── scope assembly ───────────────────────────────────────────────────────

#[test]
fn cic_scope_includes_params_of_enclosing_do() {
    let mut graph = AilGraph::new();
    let root = make_node(&mut graph, Pattern::Do, "root", None);
    add_param(&mut graph, root, "a", "A");
    add_param(&mut graph, root, "b", "B");
    let child = make_child(&mut graph, root, Pattern::Do, "child", None);
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(child).unwrap();
    let names: Vec<&str> = packet.scope.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, vec!["a", "b"]);
    assert!(packet
        .scope
        .iter()
        .all(|v| v.kind == ScopeVariableKind::Parameter));
}

#[test]
fn cic_scope_parameter_preserves_raw_type_ref_text() {
    let mut graph = AilGraph::new();
    let root = make_node(&mut graph, Pattern::Do, "root", None);
    add_param(&mut graph, root, "amount", "PositiveAmount");
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(root).unwrap();
    assert_eq!(packet.scope.len(), 1);
    assert_eq!(packet.scope[0].type_ref, "PositiveAmount");
}

// ─── must_produce ─────────────────────────────────────────────────────────

#[test]
fn cic_must_produce_is_return_type_of_nearest_enclosing_do() {
    let mut graph = AilGraph::new();
    let root = make_node(&mut graph, Pattern::Do, "root", None);
    set_return_type(&mut graph, root, "TransferResult");
    let child = make_child(&mut graph, root, Pattern::Do, "child", None);
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(child).unwrap();
    assert_eq!(packet.must_produce, Some("TransferResult".to_string()));
}

#[test]
fn cic_must_produce_is_own_return_type_when_target_is_do() {
    // When the packet is computed for a `Do` node that declares its own
    // `return_type`, `must_produce` resolves to that return type. The
    // nearest-enclosing walk is inclusive of the target node itself.
    let mut graph = AilGraph::new();
    let root = make_node(&mut graph, Pattern::Do, "root", None);
    set_return_type(&mut graph, root, "MyResult");
    graph.set_root(root).unwrap();

    let packet = graph.compute_context_packet(root).unwrap();
    assert_eq!(packet.must_produce, Some("MyResult".to_string()));
}

// ─── determinism ──────────────────────────────────────────────────────────

#[test]
fn cic_compute_is_deterministic_across_repeated_calls() {
    let mut graph = AilGraph::new();
    let root = make_node(&mut graph, Pattern::Do, "root", None);
    add_contract(&mut graph, root, ContractKind::Before, "pre");
    add_param(&mut graph, root, "x", "Int");
    let child = make_child(&mut graph, root, Pattern::Do, "child", None);
    graph.set_root(root).unwrap();

    let first = graph.compute_context_packet(child).unwrap();
    let second = graph.compute_context_packet(child).unwrap();
    let third = graph.compute_context_packet(child).unwrap();
    assert_eq!(first, second);
    assert_eq!(second, third);
}

// ─── error path ───────────────────────────────────────────────────────────

#[test]
fn cic_compute_returns_node_not_found_for_unknown_id() {
    let graph = AilGraph::new();
    let unknown = NodeId::new();
    let result = graph.compute_context_packet(unknown);
    assert!(matches!(result, Err(GraphError::NodeNotFound(_))));
}

// ─── verified_facts (Phase 1 invariant) ───────────────────────────────────

#[test]
fn cic_verified_facts_is_empty_in_phase_one() {
    let mut graph = AilGraph::new();
    let root = make_node(&mut graph, Pattern::Do, "root", None);
    add_contract(&mut graph, root, ContractKind::After, "postcondition");
    let a = make_child(&mut graph, root, Pattern::Do, "a", None);
    let b = make_sibling_after(&mut graph, a, root, Pattern::Do, "b", None);
    add_contract(&mut graph, a, ContractKind::After, "a_post");
    graph.set_root(root).unwrap();

    // Both `b` (next-sibling of a) and `root` should have empty verified_facts
    // in Phase 1 — the Rule 2 UP / verified-ACROSS promotion happens in
    // Phase 3 once Z3 verification has run.
    let b_packet = graph.compute_context_packet(b).unwrap();
    let root_packet = graph.compute_context_packet(root).unwrap();
    assert!(b_packet.verified_facts.is_empty());
    assert!(root_packet.verified_facts.is_empty());
}

// ─── parity: concrete == backend-agnostic ─────────────────────────────────

/// Verify that `AilGraph::compute_context_packet` and
/// `compute_context_packet_for_backend` produce identical output for every
/// node in a representative graph that exercises all four CIC rules
/// (DOWN, ACROSS, DIAGONAL/scope, and promoted-fact collection).
///
/// This test must pass before `compute_context_packet` can safely delegate to
/// `compute_context_packet_for_backend` (Step 5 of the gap-fix plan).
#[test]
fn cic_concrete_and_backend_agnostic_produce_identical_packets() {
    // Build a graph that exercises:
    //   - Rule 1 DOWN: root contracts flow into children
    //   - Rule 1 ACROSS: sibling scope variables flow into later siblings
    //   - DIAGONAL: type constraints and params
    //   - Promoted facts: Check sibling before a Do node
    let mut graph = AilGraph::new();

    // root Do with a before-contract (DOWN rule)
    let root = make_node(&mut graph, Pattern::Do, "transfer", Some("transfer"));
    add_contract(&mut graph, root, ContractKind::Before, "amount > 0");
    add_param(&mut graph, root, "amount", "number");
    graph.set_root(root).unwrap();

    // check sibling: promotes a fact (promoted_facts rule)
    let check = make_child(&mut graph, root, Pattern::Check, "validate", None);

    // do sibling after check: receives promoted fact from check
    let do_node = make_sibling_after(&mut graph, check, root, Pattern::Do, "compute", None);
    add_param(&mut graph, do_node, "balance", "number");
    set_return_type(&mut graph, do_node, "number");

    // let child inside do_node: scope variable test
    let _let_node = make_child(&mut graph, do_node, Pattern::Let, "new_balance", None);

    // Collect all node ids
    let all_ids: Vec<NodeId> = graph.all_nodes().map(|n| n.id).collect();
    assert!(!all_ids.is_empty());

    for id in all_ids {
        let concrete = graph
            .compute_context_packet(id)
            .unwrap_or_else(|e| panic!("concrete failed for {id:?}: {e}"));
        let backend = compute_context_packet_for_backend(&graph, id)
            .unwrap_or_else(|e| panic!("backend failed for {id:?}: {e}"));
        assert_eq!(
            concrete, backend,
            "concrete and backend-agnostic packets differ for node {id:?}"
        );
    }
}
