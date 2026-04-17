use ail_graph::{
    validate_graph, AilGraph, ContextPacket, Contract, ContractKind, EdgeKind, Expression, Field,
    Node, NodeId, Param, Pattern, ScopeVariable, ScopeVariableKind, ValidGraph,
};
use ail_types::{type_check, TypeError, TypedGraph};

// ─── Graph-building helpers ───────────────────────────────────────────────────

/// Create a minimal single-node valid graph: one Describe leaf as root.
/// No type refs, no contracts — passes all validation rules.
fn minimal_valid_graph() -> ValidGraph {
    let mut graph = AilGraph::new();
    let root = Node::new(NodeId::new(), "root", Pattern::Describe);
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();
    validate_graph(graph).unwrap()
}

/// Create a ValidGraph containing only Define/Describe/Error type nodes under a
/// structural Describe root, plus one Do function node.
///
/// Type hierarchy:
/// ```
/// define WalletBalance : number
/// define PositiveAmt   : number
/// describe User { balance: WalletBalance }
/// describe TransferResult { sender: User, amount: PositiveAmt }
/// error   InsufficientBalance { balance: WalletBalance }
/// do      transfer(sender: User, amount: PositiveAmt) -> TransferResult
/// ```
///
/// The `transfer` Do node is a top-level Do (parent = Describe), so it needs
/// before+after contracts.
fn wallet_type_graph() -> (ValidGraph, WalletIds) {
    let mut graph = AilGraph::new();

    // ── Root (structural Describe) ────────────────────────────────────────────
    let root_id = add_structural(&mut graph, "wallet", Pattern::Describe);

    // ── Define nodes ─────────────────────────────────────────────────────────
    let wallet_balance_id = add_define(&mut graph, "wallet_balance", "WalletBalance", "number");
    let positive_amt_id = add_define(&mut graph, "positive_amt", "PositiveAmt", "number");

    // ── Describe nodes ────────────────────────────────────────────────────────
    let user_id_node = add_describe(
        &mut graph,
        "user",
        "User",
        vec![field("balance", "WalletBalance")],
    );
    let transfer_result_id = add_describe(
        &mut graph,
        "transfer_result",
        "TransferResult",
        vec![field("sender", "User"), field("amount", "PositiveAmt")],
    );

    // ── Error node ────────────────────────────────────────────────────────────
    let insuf_error_id = add_error(
        &mut graph,
        "insuf_error",
        "InsufficientBalance",
        vec![field("balance", "WalletBalance")],
    );

    // ── Do function ───────────────────────────────────────────────────────────
    let transfer_id = add_do(
        &mut graph,
        "transfer",
        "transfer",
        vec![param("sender", "User"), param("amount", "PositiveAmt")],
        "TransferResult",
    );

    // Wire Ev edges: root → children
    let children = [
        wallet_balance_id,
        positive_amt_id,
        user_id_node,
        transfer_result_id,
        insuf_error_id,
        transfer_id,
    ];
    for &child in &children {
        graph.add_edge(root_id, child, EdgeKind::Ev).unwrap();
    }
    // Wire Eh edges between siblings
    for w in children.windows(2) {
        graph.add_edge(w[0], w[1], EdgeKind::Eh).unwrap();
    }
    // Mark root as structural
    graph.get_node_mut(root_id).unwrap().children = Some(children.to_vec());

    graph.set_root(root_id).unwrap();

    let ids = WalletIds {
        transfer: transfer_id,
    };

    (validate_graph(graph).unwrap(), ids)
}

struct WalletIds {
    transfer: NodeId,
}

// ─── Node-creation helpers ────────────────────────────────────────────────────

fn add_structural(graph: &mut AilGraph, intent: &str, pattern: Pattern) -> NodeId {
    let mut node = Node::new(NodeId::new(), intent, pattern);
    // children will be set after adding edges; pre-mark as structural
    node.children = Some(vec![]);
    graph.add_node(node).unwrap()
}

fn add_define(graph: &mut AilGraph, intent: &str, name: &str, base_type: &str) -> NodeId {
    let mut node = Node::new(NodeId::new(), intent, Pattern::Define);
    node.metadata.name = Some(name.to_owned());
    node.metadata.base_type = Some(base_type.to_owned());
    graph.add_node(node).unwrap()
}

fn add_describe(graph: &mut AilGraph, intent: &str, name: &str, fields: Vec<Field>) -> NodeId {
    let mut node = Node::new(NodeId::new(), intent, Pattern::Describe);
    node.metadata.name = Some(name.to_owned());
    node.metadata.fields = fields;
    graph.add_node(node).unwrap()
}

fn add_error(graph: &mut AilGraph, intent: &str, name: &str, carries: Vec<Field>) -> NodeId {
    let mut node = Node::new(NodeId::new(), intent, Pattern::Error);
    node.metadata.name = Some(name.to_owned());
    node.metadata.carries = carries;
    graph.add_node(node).unwrap()
}

/// Create a top-level Do leaf node with before+after contracts.
fn add_do(
    graph: &mut AilGraph,
    intent: &str,
    name: &str,
    params: Vec<Param>,
    return_type: &str,
) -> NodeId {
    let mut node = Node::new(NodeId::new(), intent, Pattern::Do);
    node.metadata.name = Some(name.to_owned());
    node.metadata.params = params;
    node.metadata.return_type = Some(return_type.to_owned());
    node.contracts = vec![
        Contract {
            kind: ContractKind::Before,
            expression: Expression("amount > 0".to_owned()),
        },
        Contract {
            kind: ContractKind::After,
            expression: Expression("amount > 0".to_owned()),
        },
    ];
    graph.add_node(node).unwrap()
}

fn field(name: &str, type_ref: &str) -> Field {
    Field {
        name: name.to_owned(),
        type_ref: type_ref.to_owned(),
    }
}

fn param(name: &str, type_ref: &str) -> Param {
    Param {
        name: name.to_owned(),
        type_ref: type_ref.to_owned(),
    }
}

// ─── ContextPacket helpers ────────────────────────────────────────────────────

/// Create a ContextPacket with a single scope variable.
fn packet_with_scope(node_id: NodeId, var_name: &str, type_ref: &str) -> ContextPacket {
    ContextPacket {
        node_id,
        intent_chain: vec![],
        inherited_constraints: vec![],
        type_constraints: vec![],
        call_contracts: vec![],
        template_constraints: vec![],
        verified_facts: vec![],
        promoted_facts: vec![],
        scope: vec![ScopeVariable {
            name: var_name.to_owned(),
            type_ref: type_ref.to_owned(),
            origin_node: node_id,
            kind: ScopeVariableKind::Parameter,
        }],
        must_produce: None,
        coverage: None,
    }
}

/// Create a ContextPacket with `must_produce` set.
fn packet_with_must_produce(node_id: NodeId, must_produce: &str) -> ContextPacket {
    ContextPacket {
        node_id,
        intent_chain: vec![],
        inherited_constraints: vec![],
        type_constraints: vec![],
        call_contracts: vec![],
        template_constraints: vec![],
        verified_facts: vec![],
        promoted_facts: vec![],
        scope: vec![],
        must_produce: Some(must_produce.to_owned()),
        coverage: None,
    }
}

// ─── Check: is every error a specific variant? ────────────────────────────────

fn has_undefined_type(errors: &[TypeError], name: &str) -> bool {
    errors
        .iter()
        .any(|e| matches!(e, TypeError::UndefinedType { name: n, .. } if n == name))
}

fn has_undefined_field(errors: &[TypeError], field: &str) -> bool {
    errors
        .iter()
        .any(|e| matches!(e, TypeError::UndefinedField { field: f, .. } if f == field))
}

fn has_type_mismatch(errors: &[TypeError], expected: &str, actual: &str) -> bool {
    errors.iter().any(|e| {
        matches!(e, TypeError::TypeMismatch { expected: ex, actual: ac, .. }
            if ex == expected && ac == actual)
    })
}

fn has_param_mismatch(errors: &[TypeError], param: &str) -> bool {
    errors
        .iter()
        .any(|e| matches!(e, TypeError::ParamTypeMismatch { param: p, .. } if p == param))
}

// ─── t024 — minimal valid graph ──────────────────────────────────────────────

#[test]
fn t024_minimal_valid_graph_produces_typed_graph() {
    let valid = minimal_valid_graph();
    let result = type_check(valid, &[]);
    assert!(result.is_ok(), "minimal graph should produce TypedGraph");
    // TypedGraph::graph() is accessible
    let typed: TypedGraph = result.unwrap();
    assert_eq!(typed.graph().node_count(), 1);
}

// ─── t025 — wallet fixture acceptance test ────────────────────────────────────

#[test]
fn t025_wallet_type_graph_produces_typed_graph() {
    let (valid, _ids) = wallet_type_graph();
    // Empty packets — only check 1 runs (type ref resolution for graph nodes).
    let result = type_check(valid, &[]);
    assert!(
        result.is_ok(),
        "wallet type graph should produce TypedGraph; errors: {:?}",
        result.err()
    );
}

// ─── t026-t029 — base types resolve ──────────────────────────────────────────

#[test]
fn t026_base_type_integer_resolves_via_scope_var() {
    let valid = minimal_valid_graph();
    let node_id = valid.ail_graph().root_id().unwrap();
    let packet = packet_with_scope(node_id, "x", "integer");
    assert!(type_check(valid, &[packet]).is_ok());
}

#[test]
fn t027_base_type_number_resolves_via_scope_var() {
    let valid = minimal_valid_graph();
    let node_id = valid.ail_graph().root_id().unwrap();
    let packet = packet_with_scope(node_id, "x", "number");
    assert!(type_check(valid, &[packet]).is_ok());
}

#[test]
fn t028_base_type_text_resolves_via_scope_var() {
    let valid = minimal_valid_graph();
    let node_id = valid.ail_graph().root_id().unwrap();
    let packet = packet_with_scope(node_id, "x", "text");
    assert!(type_check(valid, &[packet]).is_ok());
}

#[test]
fn t029_base_type_boolean_resolves_via_scope_var() {
    let valid = minimal_valid_graph();
    let node_id = valid.ail_graph().root_id().unwrap();
    let packet = packet_with_scope(node_id, "x", "boolean");
    assert!(type_check(valid, &[packet]).is_ok());
}

// ─── t030-t031 — builtin semantic types resolve ───────────────────────────────

#[test]
fn t030_builtin_positive_integer_resolves_via_scope_var() {
    let valid = minimal_valid_graph();
    let node_id = valid.ail_graph().root_id().unwrap();
    let packet = packet_with_scope(node_id, "count", "PositiveInteger");
    assert!(type_check(valid, &[packet]).is_ok());
}

#[test]
fn t031_all_seven_builtins_resolve_as_scope_var_types() {
    let valid = minimal_valid_graph();
    let node_id = valid.ail_graph().root_id().unwrap();
    let builtin_names = [
        "PositiveInteger",
        "NonNegativeInteger",
        "PositiveAmount",
        "Percentage",
        "NonEmptyText",
        "EmailAddress",
        "Identifier",
    ];
    let scope: Vec<ScopeVariable> = builtin_names
        .iter()
        .enumerate()
        .map(|(i, name)| ScopeVariable {
            name: format!("var_{i}"),
            type_ref: (*name).to_owned(),
            origin_node: node_id,
            kind: ScopeVariableKind::Parameter,
        })
        .collect();
    let packet = ContextPacket {
        node_id,
        intent_chain: vec![],
        inherited_constraints: vec![],
        type_constraints: vec![],
        call_contracts: vec![],
        template_constraints: vec![],
        verified_facts: vec![],
        promoted_facts: vec![],
        scope,
        must_produce: None,
        coverage: None,
    };
    assert!(type_check(valid, &[packet]).is_ok());
}

// ─── t032-t033 — user-defined type nodes resolve ─────────────────────────────

#[test]
fn t032_user_defined_define_node_resolves_as_scope_var_type() {
    let (valid, ids) = wallet_type_graph();
    // Transfer node is in scope; scope var `balance: WalletBalance` should resolve.
    let packet = packet_with_scope(ids.transfer, "balance", "WalletBalance");
    assert!(type_check(valid, &[packet]).is_ok());
}

#[test]
fn t033_user_defined_describe_node_resolves_as_scope_var_type() {
    let (valid, ids) = wallet_type_graph();
    let packet = packet_with_scope(ids.transfer, "sender", "User");
    assert!(type_check(valid, &[packet]).is_ok());
}

// ─── t034-t035 — undefined types raise errors ────────────────────────────────

#[test]
fn t034_undefined_scope_var_type_raises_undefined_type_error() {
    let valid = minimal_valid_graph();
    let node_id = valid.ail_graph().root_id().unwrap();
    let packet = packet_with_scope(node_id, "x", "GhostType");
    let errors = type_check(valid, &[packet]).unwrap_err();
    assert!(
        has_undefined_type(&errors, "GhostType"),
        "expected UndefinedType(GhostType); got: {:?}",
        errors
    );
}

#[test]
fn t035_undefined_must_produce_type_raises_error() {
    let valid = minimal_valid_graph();
    let node_id = valid.ail_graph().root_id().unwrap();
    let packet = packet_with_must_produce(node_id, "GhostType");
    let errors = type_check(valid, &[packet]).unwrap_err();
    assert!(has_undefined_type(&errors, "GhostType"));
}

// ─── t036-t037 — contract field access (check 2) ─────────────────────────────

/// Build a graph for field-access tests:
///   define WalletBalance : number
///   describe User { balance: WalletBalance }
///   do transfer(sender: User) -> WalletBalance [contracts use sender.balance]
fn field_access_graph(contract_expr: &str) -> (ValidGraph, NodeId) {
    let mut graph = AilGraph::new();

    let root_id = add_structural(&mut graph, "root", Pattern::Describe);
    let wb_id = add_define(&mut graph, "wb", "WalletBalance", "number");
    let user_id = add_describe(
        &mut graph,
        "user",
        "User",
        vec![field("balance", "WalletBalance")],
    );
    let do_id = {
        let mut node = Node::new(NodeId::new(), "transfer", Pattern::Do);
        node.metadata.name = Some("transfer".to_owned());
        node.metadata.params = vec![param("sender", "User")];
        node.metadata.return_type = Some("WalletBalance".to_owned());
        node.contracts = vec![
            Contract {
                kind: ContractKind::Before,
                expression: Expression(contract_expr.to_owned()),
            },
            Contract {
                kind: ContractKind::After,
                expression: Expression("sender.balance >= 0".to_owned()),
            },
        ];
        graph.add_node(node).unwrap()
    };

    let children = [wb_id, user_id, do_id];
    for &child in &children {
        graph.add_edge(root_id, child, EdgeKind::Ev).unwrap();
    }
    for w in children.windows(2) {
        graph.add_edge(w[0], w[1], EdgeKind::Eh).unwrap();
    }
    graph.get_node_mut(root_id).unwrap().children = Some(children.to_vec());
    graph.set_root(root_id).unwrap();

    let do_node_id = do_id;
    (validate_graph(graph).unwrap(), do_node_id)
}

#[test]
fn t036_valid_field_access_in_contract_passes() {
    let (valid, do_id) = field_access_graph("sender.balance >= 0");
    // Packet for the Do node with `sender: User` in scope.
    let packet = packet_with_scope(do_id, "sender", "User");
    let result = type_check(valid, &[packet]);
    assert!(
        result.is_ok(),
        "sender.balance should resolve: {:?}",
        result.err()
    );
}

#[test]
fn t037_undefined_field_in_contract_raises_error() {
    let (valid, do_id) = field_access_graph("sender.nonexistent >= 0");
    let packet = packet_with_scope(do_id, "sender", "User");
    let errors = type_check(valid, &[packet]).unwrap_err();
    assert!(
        has_undefined_field(&errors, "nonexistent"),
        "expected UndefinedField(nonexistent); got: {:?}",
        errors
    );
}

// ─── t038-t040 — data flow type compatibility (check 3) ──────────────────────

#[test]
fn t038_matching_return_type_and_must_produce_passes() {
    let (valid, ids) = wallet_type_graph();
    // Transfer's return_type is "TransferResult"; must_produce also "TransferResult" → Ok.
    let mut packet = packet_with_must_produce(ids.transfer, "TransferResult");
    packet.node_id = ids.transfer;
    let result = type_check(valid, &[packet]);
    assert!(
        result.is_ok(),
        "matching types should pass: {:?}",
        result.err()
    );
}

#[test]
fn t039_mismatched_return_type_and_must_produce_raises_type_mismatch() {
    let (valid, ids) = wallet_type_graph();
    // Transfer's return_type is "TransferResult"; must_produce is "WalletBalance" → mismatch.
    let packet = packet_with_must_produce(ids.transfer, "WalletBalance");
    let errors = type_check(valid, &[packet]).unwrap_err();
    assert!(
        has_type_mismatch(&errors, "WalletBalance", "TransferResult"),
        "expected TypeMismatch(WalletBalance, TransferResult); got: {:?}",
        errors
    );
}

#[test]
fn t040_subtype_not_checked_in_phase2_gives_type_mismatch() {
    // Phase 2 uses string equality, not structural subtyping.
    // PositiveAmt (value > 0) IS a logical subtype of WalletBalance (value >= 0),
    // but string "PositiveAmt" ≠ "WalletBalance" → TypeMismatch.
    // TODO(phase-3): Z3-backed subtype check will fix this false positive.
    let mut graph = AilGraph::new();
    let root_id = add_structural(&mut graph, "root", Pattern::Describe);
    let wb_id = add_define(&mut graph, "wb", "WalletBalance", "number");
    let pa_id = add_define(&mut graph, "pa", "PositiveAmt", "number");
    let do_id = add_do(&mut graph, "fn", "fn", vec![], "PositiveAmt");

    let children = [wb_id, pa_id, do_id];
    for &child in &children {
        graph.add_edge(root_id, child, EdgeKind::Ev).unwrap();
    }
    for w in children.windows(2) {
        graph.add_edge(w[0], w[1], EdgeKind::Eh).unwrap();
    }
    graph.get_node_mut(root_id).unwrap().children = Some(children.to_vec());
    graph.set_root(root_id).unwrap();
    let valid = validate_graph(graph).unwrap();

    // must_produce = "WalletBalance", return_type = "PositiveAmt" → TypeMismatch
    let packet = packet_with_must_produce(do_id, "WalletBalance");
    let errors = type_check(valid, &[packet]).unwrap_err();
    assert!(
        has_type_mismatch(&errors, "WalletBalance", "PositiveAmt"),
        "phase-2 subtype check not implemented; expected TypeMismatch; got: {:?}",
        errors
    );
}

// ─── t041-t042 — deep 3-level field access ────────────────────────────────────

/// Graph for deep field access tests:
///   define WalletBalance : number
///   describe User { balance: WalletBalance }
///   describe TransferResult { sender: User }
///   do fn(result: TransferResult) -> WalletBalance [contract uses result.sender.balance]
fn deep_access_graph(contract_expr: &str) -> (ValidGraph, NodeId) {
    let mut graph = AilGraph::new();

    let root_id = add_structural(&mut graph, "root", Pattern::Describe);
    let wb_id = add_define(&mut graph, "wb", "WalletBalance", "number");
    let user_id = add_describe(
        &mut graph,
        "user",
        "User",
        vec![field("balance", "WalletBalance")],
    );
    let tr_id = add_describe(
        &mut graph,
        "transfer_result",
        "TransferResult",
        vec![field("sender", "User")],
    );
    let do_id = {
        let mut node = Node::new(NodeId::new(), "fn", Pattern::Do);
        node.metadata.name = Some("fn".to_owned());
        node.metadata.params = vec![param("result", "TransferResult")];
        node.metadata.return_type = Some("WalletBalance".to_owned());
        node.contracts = vec![
            Contract {
                kind: ContractKind::Before,
                expression: Expression(contract_expr.to_owned()),
            },
            Contract {
                kind: ContractKind::After,
                expression: Expression("result.sender.balance >= 0".to_owned()),
            },
        ];
        graph.add_node(node).unwrap()
    };

    let children = [wb_id, user_id, tr_id, do_id];
    for &child in &children {
        graph.add_edge(root_id, child, EdgeKind::Ev).unwrap();
    }
    for w in children.windows(2) {
        graph.add_edge(w[0], w[1], EdgeKind::Eh).unwrap();
    }
    graph.get_node_mut(root_id).unwrap().children = Some(children.to_vec());
    graph.set_root(root_id).unwrap();

    (validate_graph(graph).unwrap(), do_id)
}

#[test]
fn t041_deep_three_level_field_access_passes() {
    let (valid, do_id) = deep_access_graph("result.sender.balance >= 0");
    let packet = packet_with_scope(do_id, "result", "TransferResult");
    let result = type_check(valid, &[packet]);
    assert!(
        result.is_ok(),
        "result.sender.balance should resolve (3 levels): {:?}",
        result.err()
    );
}

#[test]
fn t042_deep_field_access_invalid_middle_raises_undefined_field() {
    let (valid, do_id) = deep_access_graph("result.nonexistent.balance >= 0");
    let packet = packet_with_scope(do_id, "result", "TransferResult");
    let errors = type_check(valid, &[packet]).unwrap_err();
    assert!(
        has_undefined_field(&errors, "nonexistent"),
        "expected UndefinedField(nonexistent) on TransferResult; got: {:?}",
        errors
    );
}

// ─── t043 — parametric type list<T> ──────────────────────────────────────────

#[test]
fn t043_list_generic_inner_type_resolves() {
    let (valid, ids) = wallet_type_graph();
    // "list<WalletBalance>" — WalletBalance is a Define node in the graph.
    let packet = packet_with_scope(ids.transfer, "balances", "list<WalletBalance>");
    let result = type_check(valid, &[packet]);
    assert!(
        result.is_ok(),
        "list<WalletBalance> should resolve: {:?}",
        result.err()
    );
}

// ─── t044 — error collection ──────────────────────────────────────────────────

#[test]
fn t044_multiple_errors_are_all_collected_in_one_pass() {
    let valid = minimal_valid_graph();
    let node_id = valid.ail_graph().root_id().unwrap();
    // Two scope variables with undefined types — both errors should be collected.
    let scope = vec![
        ScopeVariable {
            name: "a".to_owned(),
            type_ref: "GhostA".to_owned(),
            origin_node: node_id,
            kind: ScopeVariableKind::Parameter,
        },
        ScopeVariable {
            name: "b".to_owned(),
            type_ref: "GhostB".to_owned(),
            origin_node: node_id,
            kind: ScopeVariableKind::Parameter,
        },
    ];
    let packet = ContextPacket {
        node_id,
        intent_chain: vec![],
        inherited_constraints: vec![],
        type_constraints: vec![],
        call_contracts: vec![],
        template_constraints: vec![],
        verified_facts: vec![],
        promoted_facts: vec![],
        scope,
        must_produce: None,
        coverage: None,
    };
    let errors = type_check(valid, &[packet]).unwrap_err();
    assert!(
        errors.len() >= 2,
        "expected ≥2 errors (one per undefined type); got: {:?}",
        errors
    );
    assert!(has_undefined_type(&errors, "GhostA"));
    assert!(has_undefined_type(&errors, "GhostB"));
}

// ─── t045-t046 — parameter types via Ed edges (check 4) ──────────────────────

/// Build a graph with an Ed edge from a Fetch leaf to a Do node:
///   define PositiveAmt : number
///   define WalletBalance : number
///   describe Root
///     fetch_node (Fetch leaf)  →Ed→  callee_do (Do, param amount:<callee_param_type>)
fn ed_edge_graph(callee_param_type: &str) -> (ValidGraph, NodeId, NodeId) {
    let mut graph = AilGraph::new();

    let root_id = add_structural(&mut graph, "root", Pattern::Describe);
    let pa_id = add_define(&mut graph, "pa", "PositiveAmt", "number");
    let wb_id = add_define(&mut graph, "wb", "WalletBalance", "number");

    // Caller: a Fetch leaf (not a Do, so no contracts needed).
    let caller_id = {
        let mut node = Node::new(NodeId::new(), "fetch_sender", Pattern::Fetch);
        node.metadata.name = Some("sender".to_owned());
        graph.add_node(node).unwrap()
    };

    // Callee: a Do leaf with param `amount:<callee_param_type>`.
    let callee_id = {
        let mut node = Node::new(NodeId::new(), "do_transfer", Pattern::Do);
        node.metadata.name = Some("transfer".to_owned());
        node.metadata.params = vec![param("amount", callee_param_type)];
        node.metadata.return_type = Some("PositiveAmt".to_owned());
        node.contracts = vec![
            Contract {
                kind: ContractKind::Before,
                expression: Expression("amount > 0".to_owned()),
            },
            Contract {
                kind: ContractKind::After,
                expression: Expression("amount > 0".to_owned()),
            },
        ];
        graph.add_node(node).unwrap()
    };

    // Wire Ev: root → [pa, wb, caller, callee]
    let children = [pa_id, wb_id, caller_id, callee_id];
    for &child in &children {
        graph.add_edge(root_id, child, EdgeKind::Ev).unwrap();
    }
    for w in children.windows(2) {
        graph.add_edge(w[0], w[1], EdgeKind::Eh).unwrap();
    }
    graph.get_node_mut(root_id).unwrap().children = Some(children.to_vec());

    // Wire Ed: caller →Ed→ callee
    graph.add_edge(caller_id, callee_id, EdgeKind::Ed).unwrap();

    graph.set_root(root_id).unwrap();
    (validate_graph(graph).unwrap(), caller_id, callee_id)
}

#[test]
fn t045_ed_edge_with_matching_param_types_passes() {
    let (valid, caller_id, _) = ed_edge_graph("PositiveAmt");
    // Caller scope has `amount: PositiveAmt` — matches callee param.
    let packet = packet_with_scope(caller_id, "amount", "PositiveAmt");
    let result = type_check(valid, &[packet]);
    assert!(
        result.is_ok(),
        "matching param types should pass: {:?}",
        result.err()
    );
}

#[test]
fn t046_ed_edge_with_mismatched_param_types_raises_error() {
    let (valid, caller_id, _) = ed_edge_graph("PositiveAmt");
    // Caller scope has `amount: WalletBalance` — does NOT match callee param `PositiveAmt`.
    let packet = packet_with_scope(caller_id, "amount", "WalletBalance");
    let errors = type_check(valid, &[packet]).unwrap_err();
    assert!(
        has_param_mismatch(&errors, "amount"),
        "expected ParamTypeMismatch for 'amount'; got: {:?}",
        errors
    );
}

// ─── t047 — old() wrapper in contracts ───────────────────────────────────────

#[test]
fn t047_old_wrapper_field_access_passes() {
    // Contract: `old(sender.balance) >= 0` — old() should unwrap and resolve sender.balance.
    let (valid, do_id) = field_access_graph("old(sender.balance) >= 0");
    let packet = packet_with_scope(do_id, "sender", "User");
    let result = type_check(valid, &[packet]);
    assert!(
        result.is_ok(),
        "old(sender.balance) should resolve correctly: {:?}",
        result.err()
    );
}

// ─── t048 — void base type resolves ──────────────────────────────────────────

#[test]
fn t048_void_type_resolves_as_base_kind() {
    // `void` is in ail-graph's BUILTIN_TYPE_NAMES for Do nodes that return nothing.
    // The type checker must resolve it without raising UndefinedType.
    let valid = minimal_valid_graph();
    let node_id = valid.ail_graph().root_id().unwrap();
    let packet = packet_with_scope(node_id, "nothing", "void");
    let result = type_check(valid, &[packet]);
    assert!(
        result.is_ok(),
        "'void' should resolve as a base type: {:?}",
        result.err()
    );
}
