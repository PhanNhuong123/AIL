use ail_contract::verify;
use ail_emit::{emit_ts_function_definitions, EmitConfig, FileOwnership};
use ail_graph::{
    validate_graph, AilGraph, Contract, ContractKind, EdgeKind, Expression, Node, NodeId,
    NodeMetadata, Param, Pattern,
};
use ail_types::type_check;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a verified graph with a top-level Do function node and the given children.
///
/// Type definition nodes are siblings of the Do function under the root container.
/// This mirrors the Python `build_verified_fn_graph` helper in `emit_functions.rs`.
fn build_verified_fn_graph(
    fn_name: &str,
    params: Vec<(&str, &str)>,
    return_type: &str,
    type_nodes: Vec<Node>,
    children: Vec<Node>,
) -> ail_contract::VerifiedGraph {
    let mut graph = AilGraph::new();

    let root = Node {
        id: NodeId::new(),
        intent: "root container".to_owned(),
        pattern: Pattern::Describe,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    let root_id = root.id;
    graph.add_node(root).expect("add root");
    graph.set_root(root_id).expect("set root");

    let mut root_children: Vec<NodeId> = Vec::new();
    for node in type_nodes {
        let node_id = node.id;
        graph.add_node(node).expect("add type node");
        graph
            .add_edge(root_id, node_id, EdgeKind::Ev)
            .expect("Ev root→type");
        root_children.push(node_id);
    }

    let fn_node_id = NodeId::new();
    let mut fn_node = Node {
        id: fn_node_id,
        intent: fn_name.to_owned(),
        pattern: Pattern::Do,
        children: None,
        expression: None,
        contracts: vec![
            Contract {
                kind: ContractKind::Before,
                expression: Expression("true == true".to_owned()),
            },
            Contract {
                kind: ContractKind::After,
                expression: Expression("true == true".to_owned()),
            },
        ],
        metadata: NodeMetadata::default(),
    };
    fn_node.metadata.name = Some(fn_name.to_owned());
    fn_node.metadata.params = params
        .into_iter()
        .map(|(n, t)| Param {
            name: n.to_owned(),
            type_ref: t.to_owned(),
        })
        .collect();
    fn_node.metadata.return_type = Some(return_type.to_owned());

    graph.add_node(fn_node).expect("add fn node");
    graph
        .add_edge(root_id, fn_node_id, EdgeKind::Ev)
        .expect("Ev root→fn");
    if let Some(prev) = root_children.last() {
        graph
            .add_edge(*prev, fn_node_id, EdgeKind::Eh)
            .expect("Eh type→fn");
    }
    root_children.push(fn_node_id);

    let mut child_ids: Vec<NodeId> = Vec::new();
    for node in children {
        let child_id = node.id;
        graph.add_node(node).expect("add child node");
        graph
            .add_edge(fn_node_id, child_id, EdgeKind::Ev)
            .expect("Ev fn→child");
        child_ids.push(child_id);
    }
    for i in 0..child_ids.len().saturating_sub(1) {
        graph
            .add_edge(child_ids[i], child_ids[i + 1], EdgeKind::Eh)
            .expect("Eh sibling");
    }
    if !child_ids.is_empty() {
        graph.get_node_mut(fn_node_id).expect("fn").children = Some(child_ids);
    }
    {
        graph.get_node_mut(root_id).expect("root").children = Some(root_children);
    }

    let valid = validate_graph(graph).expect("validate");
    let typed = type_check(valid, &[]).expect("type_check");
    verify(typed).expect("verify")
}

// ── Node constructors ─────────────────────────────────────────────────────────

fn define_type_node(name: &str, base_type: &str) -> Node {
    let mut node = Node {
        id: NodeId::new(),
        intent: format!("{name} type definition"),
        pattern: Pattern::Define,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    node.metadata.name = Some(name.to_owned());
    node.metadata.base_type = Some(base_type.to_owned());
    node
}

fn describe_type_node(name: &str) -> Node {
    let mut node = Node {
        id: NodeId::new(),
        intent: format!("{name} type"),
        pattern: Pattern::Describe,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    node.metadata.name = Some(name.to_owned());
    node
}

fn let_node(name: &str, type_ref: &str, expr: &str) -> Node {
    let mut node = Node {
        id: NodeId::new(),
        intent: format!("compute {name}"),
        pattern: Pattern::Let,
        children: None,
        expression: Some(Expression(expr.to_owned())),
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    node.metadata.name = Some(name.to_owned());
    node.metadata.return_type = Some(type_ref.to_owned());
    node
}

fn check_node(condition: &str, error_type: &str, assigns: Vec<(&str, &str)>) -> Node {
    let mut node = Node {
        id: NodeId::new(),
        intent: format!("check {condition}"),
        pattern: Pattern::Check,
        children: None,
        expression: Some(Expression(condition.to_owned())),
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    node.metadata.otherwise_error = Some(error_type.to_owned());
    node.metadata.otherwise_assigns = assigns
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    node
}

fn fetch_node(var: &str, type_ref: &str, expr: &str) -> Node {
    let mut node = Node {
        id: NodeId::new(),
        intent: format!("fetch {var}"),
        pattern: Pattern::Fetch,
        children: None,
        expression: Some(Expression(expr.to_owned())),
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    node.metadata.name = Some(var.to_owned());
    node.metadata.return_type = Some(type_ref.to_owned());
    node
}

fn save_node(var: &str, dst: &str) -> Node {
    let mut node = Node {
        id: NodeId::new(),
        intent: format!("save {var}"),
        pattern: Pattern::Save,
        children: None,
        expression: Some(Expression(format!("to {dst}"))),
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    node.metadata.name = Some(var.to_owned());
    node
}

fn update_node(entity_type: &str, expr: &str) -> Node {
    let mut node = Node {
        id: NodeId::new(),
        intent: format!("update {entity_type}"),
        pattern: Pattern::Update,
        children: None,
        expression: Some(Expression(expr.to_owned())),
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    node.metadata.return_type = Some(entity_type.to_owned());
    node
}

fn remove_node(entity_type: &str, expr: &str) -> Node {
    let mut node = Node {
        id: NodeId::new(),
        intent: format!("remove {entity_type}"),
        pattern: Pattern::Remove,
        children: None,
        expression: Some(Expression(expr.to_owned())),
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    node.metadata.return_type = Some(entity_type.to_owned());
    node
}

fn return_node(type_name: &str, with_expr: Option<&str>) -> Node {
    let mut node = Node {
        id: NodeId::new(),
        intent: format!("return {type_name}"),
        pattern: Pattern::Return,
        children: None,
        expression: with_expr.map(|e| Expression(format!("with {e}"))),
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    node.metadata.name = Some(type_name.to_owned());
    node
}

fn raise_node(error_type: &str, assigns: Vec<(&str, &str)>) -> Node {
    let mut node = Node {
        id: NodeId::new(),
        intent: format!("raise {error_type}"),
        pattern: Pattern::Raise,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    node.metadata.name = Some(error_type.to_owned());
    node.metadata.otherwise_assigns = assigns
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    node
}

fn match_node(discriminant: &str, arms: Vec<(&str, &str)>, otherwise: Option<&str>) -> Node {
    let mut node = Node {
        id: NodeId::new(),
        intent: format!("match {discriminant}"),
        pattern: Pattern::Match,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    node.metadata.discriminant = Some(discriminant.to_owned());
    node.metadata.arms = arms
        .into_iter()
        .map(|(v, e)| (v.to_owned(), e.to_owned()))
        .collect();
    node.metadata.otherwise_result = otherwise.map(|s| s.to_owned());
    node
}

fn sync_config() -> EmitConfig {
    EmitConfig {
        async_mode: false,
        ..Default::default()
    }
}

fn file_content<'a>(output: &'a ail_emit::EmitOutput, path: &str) -> &'a str {
    output
        .files
        .iter()
        .find(|f| f.path == path)
        .unwrap_or_else(|| {
            panic!(
                "file not found: {path}\navailable: {:?}",
                output.files.iter().map(|f| &f.path).collect::<Vec<_>>()
            )
        })
        .content
        .as_str()
}

// ── Function declaration tests ────────────────────────────────────────────────

#[test]
fn t092_do_emits_function_declaration() {
    let verified = build_verified_fn_graph("noop", vec![], "void", vec![], vec![]);
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/noop.ts");
    assert!(
        content.contains("function noop"),
        "function keyword missing:\n{content}"
    );
}

#[test]
fn t092_do_async_emits_async_function() {
    let verified = build_verified_fn_graph(
        "load_user",
        vec![("user_id", "text")],
        "text",
        vec![describe_type_node("User")],
        vec![fetch_node(
            "user",
            "User",
            "from database where id is user_id",
        )],
    );
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/load_user.ts");
    assert!(
        content.contains("async function loadUser"),
        "async keyword missing:\n{content}"
    );
    assert!(
        content.contains("Promise<"),
        "Promise return type missing:\n{content}"
    );
}

#[test]
fn t092_do_sync_emits_regular_function() {
    let verified =
        build_verified_fn_graph("compute", vec![("x", "number")], "number", vec![], vec![]);
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/compute.ts");
    assert!(
        content.contains("function compute"),
        "function missing:\n{content}"
    );
    assert!(
        !content.contains("async function"),
        "should not be async:\n{content}"
    );
}

#[test]
fn t092_do_camel_case_function_name() {
    let verified = build_verified_fn_graph("transfer money safely", vec![], "void", vec![], vec![]);
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/transfer_money_safely.ts");
    assert!(
        content.contains("function transferMoneySafely"),
        "camelCase function name missing:\n{content}"
    );
}

// ── Let tests ─────────────────────────────────────────────────────────────────

#[test]
fn t092_let_emits_const() {
    let verified = build_verified_fn_graph(
        "compute",
        vec![],
        "void",
        vec![],
        vec![let_node("new_balance", "number", "sender.balance - amount")],
    );
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/compute.ts");
    assert!(
        content.contains("const newBalance"),
        "const declaration missing:\n{content}"
    );
}

#[test]
fn t092_let_with_type_calls_factory() {
    let verified = build_verified_fn_graph(
        "create_wallet",
        vec![],
        "void",
        vec![define_type_node("WalletBalance", "number")],
        vec![let_node("balance", "WalletBalance", "100")],
    );
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/create_wallet.ts");
    assert!(
        content.contains("createWalletBalance(100)"),
        "factory call missing:\n{content}"
    );
}

// ── Check tests ───────────────────────────────────────────────────────────────

#[test]
fn t092_check_emits_if_throw() {
    let verified = build_verified_fn_graph(
        "validate",
        vec![],
        "void",
        vec![],
        vec![check_node("amount > 0", "Error", vec![])],
    );
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/validate.ts");
    assert!(
        content.contains("if (!("),
        "if condition missing:\n{content}"
    );
    assert!(content.contains("throw new"), "throw missing:\n{content}");
}

#[test]
fn t092_check_with_carries_passes_fields() {
    let verified = build_verified_fn_graph(
        "validate",
        vec![],
        "void",
        vec![],
        vec![check_node(
            "amount > 0",
            "InsufficientBalanceError",
            vec![("current_balance", "balance")],
        )],
    );
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/validate.ts");
    assert!(
        content.contains("current_balance:"),
        "field key missing:\n{content}"
    );
    assert!(
        content.contains("InsufficientBalanceError"),
        "error type missing:\n{content}"
    );
}

// ── Match tests ───────────────────────────────────────────────────────────────

#[test]
fn t092_match_emits_switch() {
    let verified = build_verified_fn_graph(
        "route",
        vec![],
        "void",
        vec![],
        vec![match_node("status", vec![("'active'", "proceed")], None)],
    );
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/route.ts");
    assert!(
        content.contains("switch (status)"),
        "switch missing:\n{content}"
    );
    assert!(content.contains("case"), "case missing:\n{content}");
}

#[test]
fn t092_match_otherwise_emits_default() {
    let verified = build_verified_fn_graph(
        "route",
        vec![],
        "void",
        vec![],
        vec![match_node(
            "status",
            vec![("'active'", "proceed")],
            Some("raise Error"),
        )],
    );
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/route.ts");
    assert!(
        content.contains("default:"),
        "default case missing:\n{content}"
    );
}

// ── Fetch / Save / Update / Remove tests ─────────────────────────────────────

#[test]
fn t092_fetch_emits_await_find() {
    let verified = build_verified_fn_graph(
        "load_user",
        vec![("user_id", "text")],
        "void",
        vec![describe_type_node("User")],
        vec![fetch_node(
            "user",
            "User",
            "from database where id is user_id",
        )],
    );
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/load_user.ts");
    assert!(
        content.contains("await database.findUser("),
        "await findUser missing:\n{content}"
    );
}

#[test]
fn t092_save_emits_await_save() {
    let verified = build_verified_fn_graph(
        "persist_order",
        vec![],
        "void",
        vec![describe_type_node("Order")],
        vec![save_node("Order", "database")],
    );
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/persist_order.ts");
    assert!(
        content.contains("await database.saveOrder("),
        "await saveOrder missing:\n{content}"
    );
}

#[test]
fn t092_update_emits_await_update() {
    let verified = build_verified_fn_graph(
        "update_balance",
        vec![],
        "void",
        vec![describe_type_node("User")],
        vec![update_node(
            "User",
            "in database where id is user_id set balance = new_balance",
        )],
    );
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/update_balance.ts");
    assert!(
        content.contains("await database.updateUser("),
        "await updateUser missing:\n{content}"
    );
}

#[test]
fn t092_remove_emits_await_remove() {
    let verified = build_verified_fn_graph(
        "delete_session",
        vec![],
        "void",
        vec![describe_type_node("Session")],
        vec![remove_node(
            "Session",
            "from database where id is session_id",
        )],
    );
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/delete_session.ts");
    assert!(
        content.contains("await database.removeSession("),
        "await removeSession missing:\n{content}"
    );
}

// ── Return / Raise tests ──────────────────────────────────────────────────────

#[test]
fn t092_return_emits_return_with_factory() {
    let verified = build_verified_fn_graph(
        "build_result",
        vec![],
        "void",
        vec![describe_type_node("Result")],
        vec![return_node("Result", Some("status = 'ok'"))],
    );
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/build_result.ts");
    assert!(
        content.contains("return createResult("),
        "return createResult missing:\n{content}"
    );
}

#[test]
fn t092_raise_emits_throw() {
    let verified = build_verified_fn_graph(
        "fail_fast",
        vec![],
        "void",
        vec![],
        vec![raise_node("NotFoundError", vec![("id", "user_id")])],
    );
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/fail_fast.ts");
    assert!(
        content.contains("throw new NotFoundError("),
        "throw new missing:\n{content}"
    );
    assert!(content.contains("id:"), "field key missing:\n{content}");
}

// ── Block tests ───────────────────────────────────────────────────────────────

#[test]
fn t092_for_each_emits_for_of() {
    let mut graph = AilGraph::new();
    let root = Node {
        id: NodeId::new(),
        intent: "root".to_owned(),
        pattern: Pattern::Describe,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    let root_id = root.id;
    graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let fn_id = NodeId::new();
    let mut fn_node = Node {
        id: fn_id,
        intent: "process items".to_owned(),
        pattern: Pattern::Do,
        children: None,
        expression: None,
        contracts: vec![
            Contract {
                kind: ContractKind::Before,
                expression: Expression("true == true".to_owned()),
            },
            Contract {
                kind: ContractKind::After,
                expression: Expression("true == true".to_owned()),
            },
        ],
        metadata: NodeMetadata::default(),
    };
    fn_node.metadata.name = Some("process items".to_owned());
    fn_node.metadata.return_type = Some("void".to_owned());
    graph.add_node(fn_node).unwrap();
    graph.add_edge(root_id, fn_id, EdgeKind::Ev).unwrap();

    let foreach_id = NodeId::new();
    let mut foreach_node = Node {
        id: foreach_id,
        intent: "iterate items".to_owned(),
        pattern: Pattern::ForEach,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    foreach_node.metadata.name = Some("item".to_owned());
    foreach_node.metadata.collection = Some("order.items".to_owned());
    graph.add_node(foreach_node).unwrap();
    graph.add_edge(fn_id, foreach_id, EdgeKind::Ev).unwrap();
    graph.get_node_mut(fn_id).unwrap().children = Some(vec![foreach_id]);
    graph.get_node_mut(root_id).unwrap().children = Some(vec![fn_id]);

    let valid = validate_graph(graph).expect("validate");
    let typed = type_check(valid, &[]).expect("type_check");
    let verified = verify(typed).expect("verify");

    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/process_items.ts");
    assert!(
        content.contains("for (const item of order.items)"),
        "for-of missing:\n{content}"
    );
}

#[test]
fn t092_together_emits_transaction() {
    let mut graph = AilGraph::new();
    let root_id = NodeId::new();
    let root = Node {
        id: root_id,
        intent: "root".to_owned(),
        pattern: Pattern::Describe,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let user_type_id = NodeId::new();
    let mut user_type = Node {
        id: user_type_id,
        intent: "User type".to_owned(),
        pattern: Pattern::Describe,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    user_type.metadata.name = Some("User".to_owned());
    graph.add_node(user_type).unwrap();
    graph.add_edge(root_id, user_type_id, EdgeKind::Ev).unwrap();

    let fn_id = NodeId::new();
    let mut fn_node = Node {
        id: fn_id,
        intent: "transfer funds".to_owned(),
        pattern: Pattern::Do,
        children: None,
        expression: None,
        contracts: vec![
            Contract {
                kind: ContractKind::Before,
                expression: Expression("true == true".to_owned()),
            },
            Contract {
                kind: ContractKind::After,
                expression: Expression("true == true".to_owned()),
            },
        ],
        metadata: NodeMetadata::default(),
    };
    fn_node.metadata.name = Some("transfer funds".to_owned());
    fn_node.metadata.return_type = Some("void".to_owned());
    graph.add_node(fn_node).unwrap();
    graph.add_edge(root_id, fn_id, EdgeKind::Ev).unwrap();
    graph.add_edge(user_type_id, fn_id, EdgeKind::Eh).unwrap();

    let update_id = NodeId::new();
    let mut upd_node = Node {
        id: update_id,
        intent: "update sender balance".to_owned(),
        pattern: Pattern::Update,
        children: None,
        expression: Some(Expression(
            "in database where id is sender.id set balance = new_balance".to_owned(),
        )),
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    upd_node.metadata.return_type = Some("User".to_owned());
    graph.add_node(upd_node).unwrap();

    let together_id = NodeId::new();
    let together_node = Node {
        id: together_id,
        intent: "atomic update".to_owned(),
        pattern: Pattern::Together,
        children: Some(vec![update_id]),
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    graph.add_node(together_node).unwrap();
    graph.add_edge(fn_id, together_id, EdgeKind::Ev).unwrap();
    graph
        .add_edge(together_id, update_id, EdgeKind::Ev)
        .unwrap();
    graph.get_node_mut(fn_id).unwrap().children = Some(vec![together_id]);
    graph.get_node_mut(root_id).unwrap().children = Some(vec![user_type_id, fn_id]);

    let valid = validate_graph(graph).expect("validate");
    let typed = type_check(valid, &[]).expect("type_check");
    let verified = verify(typed).expect("verify");

    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/transfer_funds.ts");
    assert!(
        content.contains(".transaction(async (tx) =>"),
        "transaction block missing:\n{content}"
    );
}

#[test]
fn t092_retry_emits_loop_with_delay() {
    let mut graph = AilGraph::new();
    let root_id = NodeId::new();
    let root = Node {
        id: root_id,
        intent: "root".to_owned(),
        pattern: Pattern::Describe,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let exchange_type_id = NodeId::new();
    let mut exchange_type = Node {
        id: exchange_type_id,
        intent: "ExchangeRate type".to_owned(),
        pattern: Pattern::Describe,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    exchange_type.metadata.name = Some("ExchangeRate".to_owned());
    graph.add_node(exchange_type).unwrap();
    graph
        .add_edge(root_id, exchange_type_id, EdgeKind::Ev)
        .unwrap();

    let fn_id = NodeId::new();
    let mut fn_node = Node {
        id: fn_id,
        intent: "fetch rate".to_owned(),
        pattern: Pattern::Do,
        children: None,
        expression: None,
        contracts: vec![
            Contract {
                kind: ContractKind::Before,
                expression: Expression("true == true".to_owned()),
            },
            Contract {
                kind: ContractKind::After,
                expression: Expression("true == true".to_owned()),
            },
        ],
        metadata: NodeMetadata::default(),
    };
    fn_node.metadata.name = Some("fetch rate".to_owned());
    fn_node.metadata.return_type = Some("void".to_owned());
    graph.add_node(fn_node).unwrap();
    graph.add_edge(root_id, fn_id, EdgeKind::Ev).unwrap();
    graph
        .add_edge(exchange_type_id, fn_id, EdgeKind::Eh)
        .unwrap();

    let retry_id = NodeId::new();
    let mut retry_node = Node {
        id: retry_id,
        intent: "retry fetch".to_owned(),
        pattern: Pattern::Retry,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    retry_node.metadata.body_intent = Some("3 times with delay 1 second".to_owned());
    graph.add_node(retry_node).unwrap();
    graph.add_edge(fn_id, retry_id, EdgeKind::Ev).unwrap();

    let inner_fetch = fetch_node("rate", "ExchangeRate", "from api where key is key");
    let inner_id = inner_fetch.id;
    graph.add_node(inner_fetch).unwrap();
    graph.add_edge(retry_id, inner_id, EdgeKind::Ev).unwrap();
    graph.get_node_mut(retry_id).unwrap().children = Some(vec![inner_id]);
    graph.get_node_mut(fn_id).unwrap().children = Some(vec![retry_id]);
    graph.get_node_mut(root_id).unwrap().children = Some(vec![exchange_type_id, fn_id]);

    let valid = validate_graph(graph).expect("validate");
    let typed = type_check(valid, &[]).expect("type_check");
    let verified = verify(typed).expect("verify");

    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/fetch_rate.ts");
    assert!(
        content.contains("for (let _attempt = 1"),
        "retry loop missing:\n{content}"
    );
    assert!(
        content.contains("setTimeout"),
        "setTimeout missing:\n{content}"
    );
}

// ── Nested Do tests ───────────────────────────────────────────────────────────

#[test]
fn t092_nested_do_same_params_inline() {
    // A Do child with the same params as the parent is inlined with a section comment.
    let mut graph = AilGraph::new();
    let root_id = NodeId::new();
    let root = Node {
        id: root_id,
        intent: "root".to_owned(),
        pattern: Pattern::Describe,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let fn_id = NodeId::new();
    let mut fn_node = Node {
        id: fn_id,
        intent: "outer fn".to_owned(),
        pattern: Pattern::Do,
        children: None,
        expression: None,
        contracts: vec![
            Contract {
                kind: ContractKind::Before,
                expression: Expression("true == true".to_owned()),
            },
            Contract {
                kind: ContractKind::After,
                expression: Expression("true == true".to_owned()),
            },
        ],
        metadata: NodeMetadata::default(),
    };
    fn_node.metadata.name = Some("outer fn".to_owned());
    fn_node.metadata.params = vec![Param {
        name: "x".to_owned(),
        type_ref: "number".to_owned(),
    }];
    fn_node.metadata.return_type = Some("void".to_owned());
    graph.add_node(fn_node).unwrap();
    graph.add_edge(root_id, fn_id, EdgeKind::Ev).unwrap();

    // Nested Do with same params.
    let nested_id = NodeId::new();
    let mut nested = Node {
        id: nested_id,
        intent: "inner step".to_owned(),
        pattern: Pattern::Do,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    nested.metadata.name = Some("inner step".to_owned());
    nested.metadata.params = vec![Param {
        name: "x".to_owned(),
        type_ref: "number".to_owned(),
    }];
    nested.metadata.return_type = Some("void".to_owned());
    graph.add_node(nested).unwrap();
    graph.add_edge(fn_id, nested_id, EdgeKind::Ev).unwrap();
    graph.get_node_mut(fn_id).unwrap().children = Some(vec![nested_id]);
    graph.get_node_mut(root_id).unwrap().children = Some(vec![fn_id]);

    let valid = validate_graph(graph).expect("validate");
    let typed = type_check(valid, &[]).expect("type_check");
    let verified = verify(typed).expect("verify");

    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/outer_fn.ts");
    assert!(
        content.contains("// --- inner step ---"),
        "section comment missing:\n{content}"
    );
}

#[test]
fn t092_nested_do_diff_params_helper() {
    // A Do child with different params is emitted as a private helper function.
    let mut graph = AilGraph::new();
    let root_id = NodeId::new();
    let root = Node {
        id: root_id,
        intent: "root".to_owned(),
        pattern: Pattern::Describe,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let fn_id = NodeId::new();
    let mut fn_node = Node {
        id: fn_id,
        intent: "outer fn".to_owned(),
        pattern: Pattern::Do,
        children: None,
        expression: None,
        contracts: vec![
            Contract {
                kind: ContractKind::Before,
                expression: Expression("true == true".to_owned()),
            },
            Contract {
                kind: ContractKind::After,
                expression: Expression("true == true".to_owned()),
            },
        ],
        metadata: NodeMetadata::default(),
    };
    fn_node.metadata.name = Some("outer fn".to_owned());
    fn_node.metadata.params = vec![Param {
        name: "x".to_owned(),
        type_ref: "number".to_owned(),
    }];
    fn_node.metadata.return_type = Some("void".to_owned());
    graph.add_node(fn_node).unwrap();
    graph.add_edge(root_id, fn_id, EdgeKind::Ev).unwrap();

    // Nested Do with different params.
    let nested_id = NodeId::new();
    let mut nested = Node {
        id: nested_id,
        intent: "inner helper".to_owned(),
        pattern: Pattern::Do,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    nested.metadata.name = Some("inner helper".to_owned());
    nested.metadata.params = vec![
        Param {
            name: "a".to_owned(),
            type_ref: "number".to_owned(),
        },
        Param {
            name: "b".to_owned(),
            type_ref: "number".to_owned(),
        },
    ];
    nested.metadata.return_type = Some("void".to_owned());
    graph.add_node(nested).unwrap();
    graph.add_edge(fn_id, nested_id, EdgeKind::Ev).unwrap();
    graph.get_node_mut(fn_id).unwrap().children = Some(vec![nested_id]);
    graph.get_node_mut(root_id).unwrap().children = Some(vec![fn_id]);

    let valid = validate_graph(graph).expect("validate");
    let typed = type_check(valid, &[]).expect("type_check");
    let verified = verify(typed).expect("verify");

    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/outer_fn.ts");
    assert!(
        content.contains("function _innerHelper"),
        "private helper missing:\n{content}"
    );
}

// ── Multi-param formatting ────────────────────────────────────────────────────

#[test]
fn t092_multi_param_formatting() {
    let verified = build_verified_fn_graph(
        "transfer",
        vec![
            ("sender", "text"),
            ("receiver", "text"),
            ("amount", "number"),
        ],
        "void",
        vec![],
        vec![],
    );
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "fn/transfer.ts");
    // With 3 params, each param gets its own line.
    assert!(
        content.contains("  sender:"),
        "sender on own line missing:\n{content}"
    );
    assert!(
        content.contains("  receiver:"),
        "receiver on own line missing:\n{content}"
    );
    assert!(
        content.contains("  amount:"),
        "amount on own line missing:\n{content}"
    );
}

// ── E2E output structure tests ────────────────────────────────────────────────

#[test]
fn e2e_fn_barrel_emitted() {
    let verified = build_verified_fn_graph("noop", vec![], "void", vec![], vec![]);
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let barrel = file_content(&output, "fn/index.ts");
    assert!(
        barrel.contains("export * from './noop';"),
        "barrel re-export missing:\n{barrel}"
    );
}

#[test]
fn e2e_fn_file_ownership_generated() {
    let verified = build_verified_fn_graph("noop", vec![], "void", vec![], vec![]);
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    for file in &output.files {
        assert_eq!(
            file.ownership,
            FileOwnership::Generated,
            "file {} has wrong ownership",
            file.path
        );
    }
}

#[test]
fn e2e_repo_interface_emitted() {
    let verified = build_verified_fn_graph(
        "load_user",
        vec![("user_id", "text")],
        "void",
        vec![describe_type_node("User")],
        vec![fetch_node(
            "user",
            "User",
            "from database where id is user_id",
        )],
    );
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "repos/database_repository.ts");
    assert!(
        content.contains("export interface DatabaseRepository"),
        "interface missing:\n{content}"
    );
}

#[test]
fn e2e_repo_update_two_param_signature() {
    let verified = build_verified_fn_graph(
        "update_balance",
        vec![],
        "void",
        vec![describe_type_node("User")],
        vec![update_node(
            "User",
            "in database where id is user_id set balance = new_balance",
        )],
    );
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    let content = file_content(&output, "repos/database_repository.ts");
    assert!(
        content
            .contains("updateUser(where: Record<string, unknown>, data: Record<string, unknown>)"),
        "update two-param signature missing:\n{content}"
    );
}

#[test]
fn e2e_empty_graph_produces_no_fn_files() {
    // A graph with only type nodes but no Do nodes should produce no fn/ or repos/ files.
    let verified = build_verified_fn_graph_no_do(vec![describe_type_node("User")]);
    let output = emit_ts_function_definitions(&verified, &sync_config()).unwrap();
    assert!(
        output.files.is_empty(),
        "expected no files for graph without Do nodes, got: {:?}",
        output.files.iter().map(|f| &f.path).collect::<Vec<_>>()
    );
}

// Helper for a graph with type nodes but no Do node.
fn build_verified_fn_graph_no_do(type_nodes: Vec<Node>) -> ail_contract::VerifiedGraph {
    let mut graph = AilGraph::new();
    let root = Node {
        id: NodeId::new(),
        intent: "root container".to_owned(),
        pattern: Pattern::Describe,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    let root_id = root.id;
    graph.add_node(root).expect("add root");
    graph.set_root(root_id).expect("set root");

    let mut root_children: Vec<NodeId> = Vec::new();
    for node in type_nodes {
        let node_id = node.id;
        graph.add_node(node).expect("add type node");
        graph.add_edge(root_id, node_id, EdgeKind::Ev).expect("Ev");
        root_children.push(node_id);
    }
    for i in 0..root_children.len().saturating_sub(1) {
        graph
            .add_edge(root_children[i], root_children[i + 1], EdgeKind::Eh)
            .expect("Eh");
    }
    if !root_children.is_empty() {
        graph.get_node_mut(root_id).expect("root").children = Some(root_children);
    }

    let valid = validate_graph(graph).expect("validate");
    let typed = type_check(valid, &[]).expect("type_check");
    verify(typed).expect("verify")
}
