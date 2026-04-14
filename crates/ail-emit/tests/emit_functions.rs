use ail_contract::verify;
use ail_emit::{emit_function_definitions, EmitConfig, EmitError};
use ail_graph::{
    validate_graph, AilGraph, Contract, ContractKind, EdgeKind, Expression, Node, NodeId,
    NodeMetadata, Param, Pattern,
};
use ail_types::type_check;

// ── Test helpers ─────────────────────────────────────────────────────────────

/// Build a verified graph containing a top-level Do function node with the given
/// type definition nodes and leaf children.
///
/// Structure: root Describe → [type_nodes...] → Do function → children.
/// Type nodes are added as siblings of the Do function (children of root).
fn build_verified_fn_graph(
    fn_name: &str,
    params: Vec<(&str, &str)>,
    return_type: &str,
    type_nodes: Vec<Node>,
    children: Vec<Node>,
) -> ail_contract::VerifiedGraph {
    let mut graph = AilGraph::new();

    // Root container.
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

    // Add type definition nodes as children of root.
    let mut root_children: Vec<NodeId> = Vec::new();
    for node in type_nodes {
        let node_id = node.id;
        graph.add_node(node).expect("add type node");
        graph
            .add_edge(root_id, node_id, EdgeKind::Ev)
            .expect("Ev root→type");
        root_children.push(node_id);
    }

    // Top-level Do function node.
    let fn_node_id = NodeId::new();
    let mut fn_node = Node {
        id: fn_node_id,
        intent: fn_name.to_owned(),
        pattern: Pattern::Do,
        children: None,
        expression: None,
        // "true == true" is the minimal valid contract expression.
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

    // Add children, connecting them to the Do node.
    let mut child_ids = Vec::new();
    for node in children {
        let child_id = node.id;
        graph.add_node(node).expect("add child node");
        graph
            .add_edge(fn_node_id, child_id, EdgeKind::Ev)
            .expect("Ev fn→child");
        child_ids.push(child_id);
    }

    // Sibling Eh chain for children.
    for i in 0..child_ids.len().saturating_sub(1) {
        graph
            .add_edge(child_ids[i], child_ids[i + 1], EdgeKind::Eh)
            .expect("Eh sibling edge");
    }

    // Update fn_node's children list.
    if !child_ids.is_empty() {
        let fn_node_mut = graph.get_node_mut(fn_node_id).expect("fn node exists");
        fn_node_mut.children = Some(child_ids.clone());
    }

    // Update root's children.
    {
        let root_mut = graph.get_node_mut(root_id).expect("root exists");
        root_mut.children = Some(root_children);
    }

    let valid = validate_graph(graph).expect("validation should pass");
    let typed = type_check(valid, &[]).expect("type check should pass");
    verify(typed).expect("verification should pass")
}

// ── Type node constructors ────────────────────────────────────────────────────

/// Minimal Define node: `define Name : base_type`.
/// Registers `name` as a valid user-defined type.
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

/// Minimal Describe node: `describe Name`.
/// Registers `name` as a valid user-defined composite type with no fields.
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

// ── Leaf node constructors ────────────────────────────────────────────────────

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

/// Update node: entity type stored in `metadata.return_type` (not `metadata.name`)
/// so that sibling update nodes with the same entity type don't trigger
/// `DuplicateNameInScope`.
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

/// Remove node: entity type stored in `metadata.return_type` (same reason as
/// update_node).
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

/// Return node: `metadata.name` holds the constructor type; `metadata.return_type`
/// is intentionally NOT set to avoid a spurious `UnresolvedTypeReference` when the
/// "type name" is actually a variable (e.g. `return_node("result", None)`).
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
    EmitConfig { async_mode: false }
}
fn async_config() -> EmitConfig {
    EmitConfig { async_mode: true }
}

// ── Integration tests ─────────────────────────────────────────────────────────

#[test]
fn emit_empty_graph_no_functions() {
    let verified = build_verified_fn_graph("noop", vec![], "void", vec![], vec![]);
    let config = sync_config();
    let output = emit_function_definitions(&verified, &config).expect("emit should succeed");
    // The function has no children, so it still emits (with pass).
    assert_eq!(output.files.len(), 1);
}

#[test]
fn emit_functions_file_path_is_correct() {
    let verified = build_verified_fn_graph("noop", vec![], "void", vec![], vec![]);
    let output = emit_function_definitions(&verified, &sync_config()).expect("emit ok");
    assert_eq!(output.files[0].path, "generated/functions.py");
}

#[test]
fn emit_do_sync_function_definition() {
    let verified = build_verified_fn_graph(
        "transfer_money",
        vec![("sender_id", "UserId"), ("amount", "PositiveAmount")],
        "TransferResult",
        vec![define_type_node("UserId", "text"), describe_type_node("TransferResult")],
        vec![],
    );
    let output = emit_function_definitions(&verified, &sync_config()).expect("emit ok");
    let content = &output.files[0].content;

    assert!(content.contains("def transfer_money("));
    assert!(!content.contains("async def"));
    assert!(content.contains("sender_id: UserId"));
    assert!(content.contains("amount: PositiveAmount"));
    assert!(content.contains("-> TransferResult:"));
}

#[test]
fn emit_do_async_function_definition() {
    let verified = build_verified_fn_graph(
        "transfer_money",
        vec![("sender_id", "UserId")],
        "TransferResult",
        vec![define_type_node("UserId", "text"), describe_type_node("TransferResult")],
        vec![],
    );
    let output = emit_function_definitions(&verified, &async_config()).expect("emit ok");
    let content = &output.files[0].content;
    assert!(content.contains("async def transfer_money("));
}

#[test]
fn emit_let_binding_in_function() {
    let verified = build_verified_fn_graph(
        "compute",
        vec![("x", "number")],
        "number",
        vec![],
        // No return_node — just testing the let binding line.
        vec![let_node("result", "number", "x * 2")],
    );
    let output = emit_function_definitions(&verified, &sync_config()).expect("emit ok");
    let content = &output.files[0].content;
    assert!(content.contains("result: float = x * 2"));
}

#[test]
fn emit_check_guard_in_function() {
    let verified = build_verified_fn_graph(
        "validate",
        vec![("sender_id", "text"), ("receiver_id", "text")],
        "void",
        vec![],
        vec![check_node(
            "sender_id is not receiver_id",
            "InvalidTransferError",
            vec![("user_id", "sender_id")],
        )],
    );
    let output = emit_function_definitions(&verified, &sync_config()).expect("emit ok");
    let content = &output.files[0].content;
    assert!(content.contains("if not ("));
    assert!(content.contains("raise InvalidTransferError(user_id=sender_id)"));
}

#[test]
fn emit_fetch_as_repo_get_sync() {
    let verified = build_verified_fn_graph(
        "load_user",
        vec![("user_id", "UserId")],
        "User",
        vec![define_type_node("UserId", "text"), describe_type_node("User")],
        // No placeholder return_node — test only checks the fetch line.
        vec![fetch_node("user", "User", "from database where id is user_id")],
    );
    let output = emit_function_definitions(&verified, &sync_config()).expect("emit ok");
    let content = &output.files[0].content;
    assert!(content.contains("user = repo.get(User, {\"id\": user_id})"));
    assert!(!content.contains("await"));
}

#[test]
fn emit_fetch_as_repo_get_async() {
    let verified = build_verified_fn_graph(
        "load_user",
        vec![("user_id", "UserId")],
        "User",
        vec![define_type_node("UserId", "text"), describe_type_node("User")],
        vec![fetch_node("user", "User", "from database where id is user_id")],
    );
    let output = emit_function_definitions(&verified, &async_config()).expect("emit ok");
    let content = &output.files[0].content;
    assert!(content.contains("user = await repo.get(User, {\"id\": user_id})"));
    assert!(content.contains("import asyncio"));
}

#[test]
fn emit_save_as_repo_save() {
    let verified = build_verified_fn_graph(
        "persist_user",
        vec![],
        "void",
        vec![],
        vec![save_node("user", "database")],
    );
    let output = emit_function_definitions(&verified, &sync_config()).expect("emit ok");
    let content = &output.files[0].content;
    assert!(content.contains("repo.save(user)"));
}

#[test]
fn emit_update_as_repo_update() {
    let verified = build_verified_fn_graph(
        "update_balance",
        vec![],
        "void",
        vec![describe_type_node("User")],
        vec![update_node(
            "User",
            "in database where id is sender.id set balance = new_balance",
        )],
    );
    let output = emit_function_definitions(&verified, &sync_config()).expect("emit ok");
    let content = &output.files[0].content;
    assert!(content.contains("repo.update(User,"));
    assert!(content.contains("\"id\": sender.id"));
    assert!(content.contains("\"balance\": new_balance"));
}

#[test]
fn emit_remove_as_repo_delete() {
    let verified = build_verified_fn_graph(
        "cleanup",
        vec![("token", "text")],
        "void",
        vec![describe_type_node("Session")],
        vec![remove_node("Session", "from store where token is token")],
    );
    let output = emit_function_definitions(&verified, &sync_config()).expect("emit ok");
    let content = &output.files[0].content;
    assert!(content.contains("repo.delete(Session,"));
}

#[test]
fn emit_return_constructs_type() {
    let verified = build_verified_fn_graph(
        "build_result",
        vec![("a", "text"), ("b", "number")],
        "MyResult",
        vec![describe_type_node("MyResult")],
        vec![return_node("MyResult", Some("a = a, b = b"))],
    );
    let output = emit_function_definitions(&verified, &sync_config()).expect("emit ok");
    let content = &output.files[0].content;
    assert!(content.contains("return MyResult(a=a, b=b)"));
}

#[test]
fn emit_raise_exception_in_function() {
    let verified = build_verified_fn_graph(
        "validate_amount",
        vec![("amount", "number")],
        "void",
        vec![],
        vec![raise_node("NegativeAmountError", vec![("amount", "amount")])],
    );
    let output = emit_function_definitions(&verified, &sync_config()).expect("emit ok");
    let content = &output.files[0].content;
    assert!(content.contains("raise NegativeAmountError(amount=amount)"));
}

#[test]
fn emit_match_branches_in_function() {
    let verified = build_verified_fn_graph(
        "handle_status",
        vec![("status", "text")],
        "void",
        vec![],
        vec![match_node(
            "status",
            vec![("\"active\"", "pass"), ("\"suspended\"", "pass")],
            Some("raise UnknownStatusError()"),
        )],
    );
    let output = emit_function_definitions(&verified, &sync_config()).expect("emit ok");
    let content = &output.files[0].content;
    assert!(content.contains("match status:"));
    assert!(content.contains("case \"active\":"));
    assert!(content.contains("case _:"));
    assert!(content.contains("raise UnknownStatusError()"));
}

#[test]
fn emit_together_transaction_block() {
    // Build a graph with a Together node containing two Update children.
    // update_node uses metadata.return_type so sibling names don't conflict.
    let update1 = update_node(
        "User",
        "in database where id is sender.id set balance = new_sender_balance",
    );
    let update2 = update_node(
        "User",
        "in database where id is receiver.id set balance = new_receiver_balance",
    );

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

    // Add "User" type definition as sibling of fn node.
    let user_type = describe_type_node("User");
    let user_type_id = user_type.id;
    graph.add_node(user_type).unwrap();
    graph.add_edge(root_id, user_type_id, EdgeKind::Ev).unwrap();

    let fn_id = NodeId::new();
    let mut fn_node = Node {
        id: fn_id,
        intent: "persist_changes".to_owned(),
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
    fn_node.metadata.name = Some("persist_changes".to_owned());
    fn_node.metadata.return_type = Some("void".to_owned());
    graph.add_node(fn_node).unwrap();
    graph.add_edge(root_id, fn_id, EdgeKind::Ev).unwrap();
    graph.add_edge(user_type_id, fn_id, EdgeKind::Eh).unwrap();

    // Together node.
    let together_id = NodeId::new();
    let together_node = Node {
        id: together_id,
        intent: "persist atomically".to_owned(),
        pattern: Pattern::Together,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    graph.add_node(together_node).unwrap();
    graph.add_edge(fn_id, together_id, EdgeKind::Ev).unwrap();

    let u1_id = update1.id;
    let u2_id = update2.id;
    graph.add_node(update1).unwrap();
    graph.add_node(update2).unwrap();
    graph.add_edge(together_id, u1_id, EdgeKind::Ev).unwrap();
    graph.add_edge(together_id, u2_id, EdgeKind::Ev).unwrap();
    graph.add_edge(u1_id, u2_id, EdgeKind::Eh).unwrap();

    {
        let t = graph.get_node_mut(together_id).unwrap();
        t.children = Some(vec![u1_id, u2_id]);
    }
    {
        let f = graph.get_node_mut(fn_id).unwrap();
        f.children = Some(vec![together_id]);
    }
    {
        let r = graph.get_node_mut(root_id).unwrap();
        r.children = Some(vec![user_type_id, fn_id]);
    }

    let valid = validate_graph(graph).expect("validation");
    let typed = type_check(valid, &[]).expect("type check");
    let verified = verify(typed).expect("verify");

    let output = emit_function_definitions(&verified, &sync_config()).expect("emit ok");
    let content = &output.files[0].content;

    assert!(content.contains("async with transaction():"));
    assert!(content.contains("import asyncio"));
    assert!(content.contains("from ail_runtime import transaction"));
}

#[test]
fn emit_retry_sync_loop() {
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

    // Type node for the inner fetch's return type.
    let exchange_type = describe_type_node("ExchangeRate");
    let exchange_id = exchange_type.id;
    graph.add_node(exchange_type).unwrap();
    graph.add_edge(root_id, exchange_id, EdgeKind::Ev).unwrap();

    let fn_id = NodeId::new();
    let mut fn_node = Node {
        id: fn_id,
        intent: "fetch_with_retry".to_owned(),
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
    fn_node.metadata.name = Some("fetch_with_retry".to_owned());
    fn_node.metadata.return_type = Some("void".to_owned());
    graph.add_node(fn_node).unwrap();
    graph.add_edge(root_id, fn_id, EdgeKind::Ev).unwrap();
    graph.add_edge(exchange_id, fn_id, EdgeKind::Eh).unwrap();

    // Retry node: retry spec stored in body_intent (not expression) because
    // structural nodes cannot carry an expression when they have children.
    let retry_id = NodeId::new();
    let mut retry_node = Node {
        id: retry_id,
        intent: "retry fetch".to_owned(),
        pattern: Pattern::Retry,
        children: None,
        expression: None, // no expression on structural node
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    retry_node.metadata.body_intent = Some("3 times with delay 1 second".to_owned());
    graph.add_node(retry_node).unwrap();
    graph.add_edge(fn_id, retry_id, EdgeKind::Ev).unwrap();

    let inner_fetch = fetch_node("rate", "ExchangeRate", "from api where currency is currency");
    let inner_id = inner_fetch.id;
    graph.add_node(inner_fetch).unwrap();
    graph.add_edge(retry_id, inner_id, EdgeKind::Ev).unwrap();

    {
        let r = graph.get_node_mut(retry_id).unwrap();
        r.children = Some(vec![inner_id]);
    }
    {
        let f = graph.get_node_mut(fn_id).unwrap();
        f.children = Some(vec![retry_id]);
    }
    {
        let r = graph.get_node_mut(root_id).unwrap();
        r.children = Some(vec![exchange_id, fn_id]);
    }

    let valid = validate_graph(graph).expect("validation");
    let typed = type_check(valid, &[]).expect("type check");
    let verified = verify(typed).expect("verify");

    let output = emit_function_definitions(&verified, &sync_config()).expect("emit ok");
    let content = &output.files[0].content;

    assert!(content.contains("for _retry_i in range(3):"));
    assert!(content.contains("try:"));
    assert!(content.contains("break"));
    assert!(content.contains("except Exception:"));
    assert!(content.contains("time.sleep("));
    assert!(content.contains("import time"));
}

#[test]
fn emit_retry_async_loop() {
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

    let data_type = describe_type_node("Data");
    let data_type_id = data_type.id;
    graph.add_node(data_type).unwrap();
    graph.add_edge(root_id, data_type_id, EdgeKind::Ev).unwrap();

    let fn_id = NodeId::new();
    let mut fn_node = Node {
        id: fn_id,
        intent: "fetch_async_retry".to_owned(),
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
    fn_node.metadata.name = Some("fetch_async_retry".to_owned());
    fn_node.metadata.return_type = Some("void".to_owned());
    graph.add_node(fn_node).unwrap();
    graph.add_edge(root_id, fn_id, EdgeKind::Ev).unwrap();
    graph.add_edge(data_type_id, fn_id, EdgeKind::Eh).unwrap();

    let retry_id = NodeId::new();
    let mut retry_node = Node {
        id: retry_id,
        intent: "retry async fetch".to_owned(),
        pattern: Pattern::Retry,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    retry_node.metadata.body_intent = Some("2 times".to_owned());
    graph.add_node(retry_node).unwrap();
    graph.add_edge(fn_id, retry_id, EdgeKind::Ev).unwrap();

    let inner_fetch = fetch_node("data", "Data", "from api where key is key");
    let inner_id = inner_fetch.id;
    graph.add_node(inner_fetch).unwrap();
    graph.add_edge(retry_id, inner_id, EdgeKind::Ev).unwrap();

    {
        let r = graph.get_node_mut(retry_id).unwrap();
        r.children = Some(vec![inner_id]);
    }
    {
        let f = graph.get_node_mut(fn_id).unwrap();
        f.children = Some(vec![retry_id]);
    }
    {
        let r = graph.get_node_mut(root_id).unwrap();
        r.children = Some(vec![data_type_id, fn_id]);
    }

    let valid = validate_graph(graph).expect("validation");
    let typed = type_check(valid, &[]).expect("type check");
    let verified = verify(typed).expect("verify");

    let output = emit_function_definitions(&verified, &async_config()).expect("emit ok");
    let content = &output.files[0].content;

    assert!(content.contains("await asyncio.sleep("));
    assert!(content.contains("import asyncio"));
    assert!(!content.contains("import time"));
}

#[test]
fn emit_do_with_nested_section_comment() {
    // A Do node containing another Do (a section) that contains a leaf.
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

    // Outer function.
    let fn_id = NodeId::new();
    let mut fn_node = Node {
        id: fn_id,
        intent: "outer_function".to_owned(),
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
    fn_node.metadata.name = Some("outer_function".to_owned());
    fn_node.metadata.return_type = Some("void".to_owned());
    graph.add_node(fn_node).unwrap();
    graph.add_edge(root_id, fn_id, EdgeKind::Ev).unwrap();

    // Nested section Do (no params/return_type → section).
    let section_id = NodeId::new();
    let mut section_node = Node {
        id: section_id,
        intent: "persist changes".to_owned(),
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
    section_node.metadata.name = Some("persist changes".to_owned());
    graph.add_node(section_node).unwrap();
    graph.add_edge(fn_id, section_id, EdgeKind::Ev).unwrap();

    // Leaf inside section.
    let leaf = save_node("data", "database");
    let leaf_id = leaf.id;
    graph.add_node(leaf).unwrap();
    graph.add_edge(section_id, leaf_id, EdgeKind::Ev).unwrap();

    {
        let s = graph.get_node_mut(section_id).unwrap();
        s.children = Some(vec![leaf_id]);
    }
    {
        let f = graph.get_node_mut(fn_id).unwrap();
        f.children = Some(vec![section_id]);
    }
    {
        let r = graph.get_node_mut(root_id).unwrap();
        r.children = Some(vec![fn_id]);
    }

    let valid = validate_graph(graph).expect("validation");
    let typed = type_check(valid, &[]).expect("type check");
    let verified = verify(typed).expect("verify");

    let output = emit_function_definitions(&verified, &sync_config()).expect("emit ok");
    let content = &output.files[0].content;

    // The section should appear as a comment, not as a separate function.
    assert!(content.contains("# --- persist changes ---"));
    assert!(content.contains("repo.save(data)"));
    // Only one function definition.
    assert_eq!(content.matches("def ").count(), 1);
}

#[test]
fn emit_accumulated_function_errors() {
    // A Do node missing its name should produce an error.
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

    // Do node with no name.
    let fn_id = NodeId::new();
    let fn_node = Node {
        id: fn_id,
        intent: "unnamed function".to_owned(),
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
        metadata: NodeMetadata::default(), // name is None
    };
    graph.add_node(fn_node).unwrap();
    graph.add_edge(root_id, fn_id, EdgeKind::Ev).unwrap();
    {
        let r = graph.get_node_mut(root_id).unwrap();
        r.children = Some(vec![fn_id]);
    }

    let valid = validate_graph(graph).expect("validation");
    let typed = type_check(valid, &[]).expect("type check");
    let verified = verify(typed).expect("verify");

    let errors = emit_function_definitions(&verified, &sync_config()).unwrap_err();
    assert!(!errors.is_empty());
    assert!(matches!(errors[0], EmitError::DoNodeMissingName { .. }));
}

#[test]
fn emit_wallet_transfer_function() {
    // Build the full wallet transfer_money function.
    // Return type is a single type to satisfy the type checker (union types are
    // tested at the unit level in function::tests::resolve_return_type_union).
    let verified = build_verified_fn_graph(
        "transfer_money",
        vec![
            ("sender_id", "UserId"),
            ("receiver_id", "UserId"),
            ("amount", "PositiveAmount"),
        ],
        "TransferResult",
        vec![
            define_type_node("UserId", "text"),
            describe_type_node("User"),
            define_type_node("WalletBalance", "number"),
            describe_type_node("TransferResult"),
        ],
        vec![
            // 01: validate
            check_node(
                "sender_id is not receiver_id",
                "InvalidTransferError",
                vec![("user_id", "sender_id")],
            ),
            // 02: fetch sender
            fetch_node("sender", "User", "from database where id is sender_id"),
            // 03: fetch receiver
            fetch_node("receiver", "User", "from database where id is receiver_id"),
            // 04: compute sender balance
            let_node("new_sender_balance", "WalletBalance", "sender.balance - amount"),
            // 05: compute receiver balance
            let_node(
                "new_receiver_balance",
                "WalletBalance",
                "receiver.balance + amount",
            ),
            // 06: persist — two update nodes with the SAME entity type "User".
            // Using metadata.return_type (not metadata.name) avoids DuplicateNameInScope.
            update_node(
                "User",
                "in database where id is sender.id set balance = new_sender_balance",
            ),
            update_node(
                "User",
                "in database where id is receiver.id set balance = new_receiver_balance",
            ),
            // 07: return result
            return_node(
                "TransferResult",
                Some("sender = sender, receiver = receiver, amount = amount"),
            ),
        ],
    );

    let output =
        emit_function_definitions(&verified, &sync_config()).expect("emit should succeed");
    let content = &output.files[0].content;

    // Function signature.
    assert!(content.contains("def transfer_money("), "missing function def");
    assert!(content.contains("sender_id: UserId"), "missing sender_id param");
    assert!(content.contains("receiver_id: UserId"), "missing receiver_id param");
    assert!(content.contains("amount: PositiveAmount"), "missing amount param");
    assert!(content.contains("-> TransferResult:"), "missing return type");

    // Check guard.
    assert!(content.contains("if not ("), "missing check guard");
    assert!(
        content.contains("raise InvalidTransferError(user_id=sender_id)"),
        "missing raise"
    );

    // Fetch calls.
    assert!(
        content.contains("repo.get(User, {\"id\": sender_id})"),
        "missing fetch sender"
    );
    assert!(
        content.contains("repo.get(User, {\"id\": receiver_id})"),
        "missing fetch receiver"
    );

    // Let bindings.
    assert!(
        content.contains("new_sender_balance: WalletBalance = sender.balance - amount"),
        "missing let sender balance"
    );
    assert!(
        content.contains("new_receiver_balance: WalletBalance = receiver.balance + amount"),
        "missing let receiver balance"
    );

    // Update calls.
    assert!(content.contains("repo.update(User,"), "missing update calls");
    assert!(
        content.contains("\"balance\": new_sender_balance"),
        "missing sender balance update"
    );

    // Return.
    assert!(
        content.contains("return TransferResult(sender=sender, receiver=receiver, amount=amount)"),
        "missing return"
    );
}

#[test]
fn emit_foreach_loop_in_function() {
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
        intent: "process_items".to_owned(),
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
    fn_node.metadata.name = Some("process_items".to_owned());
    fn_node.metadata.return_type = Some("void".to_owned());
    graph.add_node(fn_node).unwrap();
    graph.add_edge(root_id, fn_id, EdgeKind::Ev).unwrap();

    // ForEach: no expression on structural node — use metadata.collection + metadata.name.
    let foreach_id = NodeId::new();
    let mut foreach_node = Node {
        id: foreach_id,
        intent: "iterate items".to_owned(),
        pattern: Pattern::ForEach,
        children: None,
        expression: None, // no expression on structural node
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    foreach_node.metadata.name = Some("item".to_owned());
    foreach_node.metadata.collection = Some("order.items".to_owned());
    graph.add_node(foreach_node).unwrap();
    graph.add_edge(fn_id, foreach_id, EdgeKind::Ev).unwrap();

    let inner = let_node("total", "number", "total + item.price");
    let inner_id = inner.id;
    graph.add_node(inner).unwrap();
    graph.add_edge(foreach_id, inner_id, EdgeKind::Ev).unwrap();

    {
        let fe = graph.get_node_mut(foreach_id).unwrap();
        fe.children = Some(vec![inner_id]);
    }
    {
        let f = graph.get_node_mut(fn_id).unwrap();
        f.children = Some(vec![foreach_id]);
    }
    {
        let r = graph.get_node_mut(root_id).unwrap();
        r.children = Some(vec![fn_id]);
    }

    let valid = validate_graph(graph).expect("validation");
    let typed = type_check(valid, &[]).expect("type check");
    let verified = verify(typed).expect("verify");

    let output = emit_function_definitions(&verified, &sync_config()).expect("emit ok");
    let content = &output.files[0].content;

    assert!(content.contains("for item in order.items:"));
    assert!(content.contains("total: float = total + item.price"));
}

// ── Python syntax verification ─────────────────────────────────────────────────

#[test]
fn emit_generated_python_functions_valid_syntax() {
    let verified = build_verified_fn_graph(
        "transfer_money",
        vec![
            ("sender_id", "UserId"),
            ("receiver_id", "UserId"),
            ("amount", "PositiveAmount"),
        ],
        "TransferResult",
        vec![
            define_type_node("UserId", "text"),
            describe_type_node("User"),
            define_type_node("WalletBalance", "number"),
            describe_type_node("TransferResult"),
        ],
        vec![
            check_node(
                "sender_id is not receiver_id",
                "InvalidTransferError",
                vec![("user_id", "sender_id")],
            ),
            fetch_node("sender", "User", "from database where id is sender_id"),
            fetch_node("receiver", "User", "from database where id is receiver_id"),
            let_node("new_sender_balance", "WalletBalance", "sender.balance - amount"),
            let_node(
                "new_receiver_balance",
                "WalletBalance",
                "receiver.balance + amount",
            ),
            update_node(
                "User",
                "in database where id is sender.id set balance = new_sender_balance",
            ),
            update_node(
                "User",
                "in database where id is receiver.id set balance = new_receiver_balance",
            ),
            return_node(
                "TransferResult",
                Some("sender = sender, receiver = receiver, amount = amount"),
            ),
        ],
    );

    let output = emit_function_definitions(&verified, &sync_config()).expect("emit ok");
    let code = &output.files[0].content;

    // Try "python3" first (standard on Linux/macOS), then "python" (Windows/venv).
    let result = std::process::Command::new("python3")
        .args(["-c", "import sys, ast; ast.parse(sys.stdin.read())"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .or_else(|_| {
            std::process::Command::new("python")
                .args(["-c", "import sys, ast; ast.parse(sys.stdin.read())"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
        });

    let mut child = match result {
        Ok(child) => child,
        Err(_) => {
            eprintln!("Python not available, skipping syntax check");
            return;
        }
    };

    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(code.as_bytes())
        .expect("write to stdin");

    let output = child.wait_with_output().expect("wait for python");

    assert!(
        output.status.success(),
        "Generated Python has syntax errors:\n{}\n\nGenerated code:\n{}",
        String::from_utf8_lossy(&output.stderr),
        code
    );
}
