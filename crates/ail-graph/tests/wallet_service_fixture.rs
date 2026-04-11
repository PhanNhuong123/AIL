use std::path::PathBuf;

use ail_graph::{
    validate_graph, AilGraph, AilGraphBuilder, Contract, ContractKind, EdgeKind, Expression, Field,
    NodeId, Param, Pattern, ScopeVariableKind,
};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/wallet_service")
}

fn load_wallet_service_fixture() -> AilGraph {
    let mut graph = AilGraphBuilder::build_from_directory(&fixture_path()).unwrap();
    hydrate_wallet_service_fixture(&mut graph);
    graph
}

fn hydrate_wallet_service_fixture(graph: &mut AilGraph) {
    mark_structural_nodes(graph);

    for intent in [
        "wallet_service",
        "concepts",
        "errors",
        "fn",
        "templates",
        "command_flow",
    ] {
        set_container(graph, intent);
    }

    set_define(
        graph,
        "user_id",
        "UserId",
        "text",
        "value matches /usr_[a-z0-9]+/",
    );
    set_define(
        graph,
        "wallet_balance",
        "WalletBalance",
        "number",
        "value >= 0",
    );
    set_define(
        graph,
        "positive_amount",
        "PositiveAmount",
        "number",
        "value > 0",
    );
    set_define(
        graph,
        "user_status",
        "UserStatus",
        "text",
        r#"value in {"active", "locked"}"#,
    );

    set_describe(
        graph,
        "user",
        "User",
        vec![
            field("id", "UserId"),
            field("balance", "WalletBalance"),
            field("status", "UserStatus"),
        ],
    );
    set_describe(
        graph,
        "transfer_result",
        "TransferResult",
        vec![
            field("sender", "User"),
            field("receiver", "User"),
            field("amount", "PositiveAmount"),
        ],
    );
    set_error(
        graph,
        "insufficient_balance_error",
        "InsufficientBalanceError",
        vec![
            field("current_balance", "WalletBalance"),
            field("requested_amount", "PositiveAmount"),
        ],
    );

    set_transfer_money(graph);

    set_transfer_child(graph, "01_validate", Pattern::Check, "validate", None, None);
    set_transfer_child(
        graph,
        "02_fetch_sender",
        Pattern::Fetch,
        "sender",
        Some("User"),
        Some("fetch sender from database"),
    );
    set_transfer_child(
        graph,
        "03_fetch_receiver",
        Pattern::Fetch,
        "receiver",
        Some("User"),
        Some("fetch receiver from database"),
    );
    set_transfer_child(
        graph,
        "04_compute_sender_balance",
        Pattern::Let,
        "new_sender_balance",
        Some("WalletBalance"),
        Some("sender.balance - amount"),
    );
    set_transfer_child(
        graph,
        "05_compute_receiver_balance",
        Pattern::Let,
        "new_receiver_balance",
        Some("WalletBalance"),
        Some("receiver.balance + amount"),
    );
    set_persist(graph);
    set_persist_child(
        graph,
        "01_save_sender",
        Pattern::Update,
        "save_sender",
        None,
        Some("sender.balance = new_sender_balance"),
    );
    set_persist_child(
        graph,
        "02_save_receiver",
        Pattern::Update,
        "save_receiver",
        None,
        Some("receiver.balance = new_receiver_balance"),
    );
    set_transfer_child(
        graph,
        "07_return_result",
        Pattern::Return,
        "return_result",
        Some("TransferResult"),
        Some("TransferResult(sender, receiver, amount)"),
    );

    for (intent, name) in [
        ("01_validate", "validate"),
        ("02_fetch_sender", "sender"),
        ("03_fetch_receiver", "receiver"),
        ("04_compute_sender_balance", "new_sender_balance"),
        ("05_compute_receiver_balance", "new_receiver_balance"),
        ("06_persist", "persist"),
        ("07_return_result", "return_result"),
    ] {
        set_template_phase(graph, intent, name);
    }

    let transfer_money = find_node(graph, "transfer_money");
    let command_flow = find_node(graph, "command_flow");
    graph
        .add_edge(transfer_money, command_flow, EdgeKind::Ed)
        .unwrap();
}

fn mark_structural_nodes(graph: &mut AilGraph) {
    let node_ids: Vec<NodeId> = graph.node_ids().collect();
    let structural: Vec<(NodeId, Vec<NodeId>)> = node_ids
        .into_iter()
        .filter_map(|id| {
            let children = graph.children_of(id).unwrap();
            (!children.is_empty()).then_some((id, children))
        })
        .collect();

    for (id, children) in structural {
        graph.get_node_mut(id).unwrap().children = Some(children);
    }
}

fn set_container(graph: &mut AilGraph, intent: &str) {
    let id = find_node(graph, intent);
    let node = graph.get_node_mut(id).unwrap();
    node.pattern = Pattern::Describe;
    node.metadata.name = None;
}

fn set_define(graph: &mut AilGraph, intent: &str, name: &str, base_type: &str, constraint: &str) {
    let id = find_node(graph, intent);
    let node = graph.get_node_mut(id).unwrap();
    node.pattern = Pattern::Define;
    node.metadata.name = Some(name.to_owned());
    node.metadata.base_type = Some(base_type.to_owned());
    node.contracts
        .push(contract(ContractKind::Always, constraint));
}

fn set_describe(graph: &mut AilGraph, intent: &str, name: &str, fields: Vec<Field>) {
    let id = find_node(graph, intent);
    let node = graph.get_node_mut(id).unwrap();
    node.pattern = Pattern::Describe;
    node.metadata.name = Some(name.to_owned());
    node.metadata.fields = fields;
}

fn set_error(graph: &mut AilGraph, intent: &str, name: &str, carries: Vec<Field>) {
    let id = find_node(graph, intent);
    let node = graph.get_node_mut(id).unwrap();
    node.pattern = Pattern::Error;
    node.metadata.name = Some(name.to_owned());
    node.metadata.carries = carries;
}

fn set_transfer_money(graph: &mut AilGraph) {
    let id = find_node(graph, "transfer_money");
    let node = graph.get_node_mut(id).unwrap();
    node.pattern = Pattern::Do;
    node.metadata.name = Some("transfer_money".to_owned());
    node.metadata.params = vec![
        param("sender_id", "UserId"),
        param("receiver_id", "UserId"),
        param("amount", "PositiveAmount"),
    ];
    node.metadata.return_type = Some("TransferResult".to_owned());
    node.contracts = vec![
        contract(ContractKind::Before, "sender_id is not receiver_id"),
        contract(ContractKind::Before, "amount > 0"),
        contract(ContractKind::After, "result.amount is amount"),
        contract(ContractKind::After, "result.sender.balance >= 0"),
    ];
}

fn set_persist(graph: &mut AilGraph) {
    let id = find_transfer_child(graph, "06_persist");
    let node = graph.get_node_mut(id).unwrap();
    node.pattern = Pattern::Do;
    node.metadata.name = Some("persist".to_owned());
}

fn set_transfer_child(
    graph: &mut AilGraph,
    intent: &str,
    pattern: Pattern,
    name: &str,
    return_type: Option<&str>,
    expression: Option<&str>,
) {
    let id = find_transfer_child(graph, intent);
    set_leaf_by_id(graph, id, pattern, name, return_type, expression);
}

fn set_persist_child(
    graph: &mut AilGraph,
    intent: &str,
    pattern: Pattern,
    name: &str,
    return_type: Option<&str>,
    expression: Option<&str>,
) {
    let id = find_persist_child(graph, intent);
    set_leaf_by_id(graph, id, pattern, name, return_type, expression);
}

fn set_leaf_by_id(
    graph: &mut AilGraph,
    id: NodeId,
    pattern: Pattern,
    name: &str,
    return_type: Option<&str>,
    expression: Option<&str>,
) {
    let node = graph.get_node_mut(id).unwrap();
    node.pattern = pattern;
    node.metadata.name = Some(name.to_owned());
    node.metadata.return_type = return_type.map(str::to_owned);
    node.expression = expression.map(|e| Expression(e.to_owned()));
}

fn set_template_phase(graph: &mut AilGraph, intent: &str, name: &str) {
    let id = find_template_phase_node(graph, intent);
    let node = graph.get_node_mut(id).unwrap();
    node.pattern = Pattern::Describe;
    node.metadata.name = Some(name.to_owned());
}

fn contract(kind: ContractKind, expression: &str) -> Contract {
    Contract {
        kind,
        expression: Expression(expression.to_owned()),
    }
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

fn find_node(graph: &AilGraph, intent: &str) -> NodeId {
    let matches: Vec<NodeId> = graph
        .all_nodes()
        .filter(|node| node.intent == intent)
        .map(|node| node.id)
        .collect();
    assert_eq!(matches.len(), 1, "expected one node with intent {intent}");
    matches[0]
}

fn find_transfer_child(graph: &AilGraph, intent: &str) -> NodeId {
    let transfer_money = find_node(graph, "transfer_money");
    find_child_by_intent(graph, transfer_money, intent)
}

fn find_persist_child(graph: &AilGraph, intent: &str) -> NodeId {
    let persist = find_transfer_child(graph, "06_persist");
    find_child_by_intent(graph, persist, intent)
}

fn find_template_phase_node(graph: &AilGraph, intent: &str) -> NodeId {
    let command_flow = find_node(graph, "command_flow");
    find_child_by_intent(graph, command_flow, intent)
}

fn find_child_by_intent(graph: &AilGraph, parent_id: NodeId, intent: &str) -> NodeId {
    let matches: Vec<NodeId> = graph
        .children_of(parent_id)
        .unwrap()
        .into_iter()
        .filter(|id| graph.get_node(*id).unwrap().intent == intent)
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "expected one child node with intent {intent}"
    );
    matches[0]
}

#[test]
fn fixture_files_load_into_graph() {
    let graph = AilGraphBuilder::build_from_directory(&fixture_path()).unwrap();

    assert_eq!(graph.node_count(), 30);
    assert_eq!(
        graph.get_node(graph.root_id().unwrap()).unwrap().intent,
        "wallet_service"
    );
    assert!(graph
        .all_nodes()
        .any(|node| node.intent == "insufficient_balance_error"));
    assert!(graph
        .all_nodes()
        .any(|node| node.intent == "07_return_result"));
}

#[test]
fn wallet_service_fixture_builds_and_validates() {
    let graph = load_wallet_service_fixture();
    let valid = validate_graph(graph);

    assert!(
        valid.is_ok(),
        "wallet_service fixture should validate, got {:?}",
        valid.unwrap_err()
    );
}

#[test]
fn wallet_service_fixture_cic_packets_cover_several_depths() {
    let graph = load_wallet_service_fixture();
    let valid = validate_graph(graph).unwrap();
    let graph = valid.graph();

    let transfer_money = find_node(graph, "transfer_money");
    let validate = find_transfer_child(graph, "01_validate");
    let save_sender = find_persist_child(graph, "01_save_sender");
    let return_result = find_transfer_child(graph, "07_return_result");

    let transfer_packet = graph.compute_context_packet(transfer_money).unwrap();
    assert_eq!(transfer_packet.scope.len(), 3);
    assert!(transfer_packet
        .type_constraints
        .iter()
        .any(|constraint| constraint.expression.0 == "value > 0"));
    assert_eq!(
        transfer_packet.must_produce,
        Some("TransferResult".to_owned())
    );

    let validate_packet = graph.compute_context_packet(validate).unwrap();
    let inherited: Vec<&str> = validate_packet
        .inherited_constraints
        .iter()
        .map(|constraint| constraint.expression.0.as_str())
        .collect();
    assert!(inherited.contains(&"sender_id is not receiver_id"));
    assert!(inherited.contains(&"result.sender.balance >= 0"));

    let save_sender_packet = graph.compute_context_packet(save_sender).unwrap();
    let scope_names: Vec<&str> = save_sender_packet
        .scope
        .iter()
        .map(|variable| variable.name.as_str())
        .collect();
    assert!(scope_names.contains(&"sender"));
    assert!(scope_names.contains(&"receiver"));
    assert!(scope_names.contains(&"new_sender_balance"));
    assert!(scope_names.contains(&"new_receiver_balance"));
    assert!(save_sender_packet.scope.iter().any(|variable| {
        variable.name == "new_sender_balance" && variable.kind == ScopeVariableKind::LetBinding
    }));
    assert!(save_sender_packet
        .type_constraints
        .iter()
        .any(|constraint| constraint.expression.0 == "value >= 0"));
    assert_eq!(
        save_sender_packet.must_produce,
        Some("TransferResult".to_owned())
    );

    let return_packet = graph.compute_context_packet(return_result).unwrap();
    assert!(return_packet.scope.iter().any(
        |variable| variable.name == "sender" && variable.kind == ScopeVariableKind::FetchResult
    ));
    assert_eq!(
        return_packet.must_produce,
        Some("TransferResult".to_owned())
    );
}
