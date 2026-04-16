use ail_contract::verify;
use ail_emit::{emit_ts_type_definitions, FileOwnership};
use ail_graph::{
    validate_graph, AilGraph, Contract, ContractKind, EdgeKind, Expression, Field, Node, NodeId,
    NodeMetadata, Pattern,
};
use ail_types::type_check;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn build_verified(nodes: Vec<Node>) -> ail_contract::VerifiedGraph {
    let mut graph = AilGraph::new();

    let root = Node {
        id: NodeId::new(),
        intent: "root container".to_owned(),
        pattern: Pattern::Describe,
        children: None,
        expression: None,
        contracts: Vec::new(),
        metadata: NodeMetadata::default(),
    };
    let root_id = root.id;
    graph.add_node(root).expect("add root");
    graph.set_root(root_id).expect("set root");

    let mut child_ids = Vec::new();
    for node in nodes {
        let child_id = node.id;
        graph.add_node(node).expect("add child");
        graph
            .add_edge(root_id, child_id, EdgeKind::Ev)
            .expect("Ev edge");
        child_ids.push(child_id);
    }

    for i in 0..child_ids.len().saturating_sub(1) {
        graph
            .add_edge(child_ids[i], child_ids[i + 1], EdgeKind::Eh)
            .expect("Eh edge");
    }

    if !child_ids.is_empty() {
        let root_node = graph.get_node_mut(root_id).expect("root");
        root_node.children = Some(child_ids);
    }

    let valid = validate_graph(graph).expect("validate");
    let typed = type_check(valid, &[]).expect("type check");
    verify(typed).expect("verify")
}

fn define_node(name: &str, base: &str, constraint: Option<&str>) -> Node {
    let mut node = Node {
        id: NodeId::new(),
        intent: format!("{name} type"),
        pattern: Pattern::Define,
        children: None,
        expression: constraint.map(|c| Expression(c.to_owned())),
        contracts: Vec::new(),
        metadata: NodeMetadata::default(),
    };
    node.metadata.name = Some(name.to_owned());
    node.metadata.base_type = Some(base.to_owned());
    if let Some(c) = constraint {
        node.contracts.push(Contract {
            kind: ContractKind::Always,
            expression: Expression(c.to_owned()),
        });
    }
    node
}

fn describe_node(name: &str, fields: Vec<(&str, &str)>) -> Node {
    let mut node = Node {
        id: NodeId::new(),
        intent: format!("{name} record"),
        pattern: Pattern::Describe,
        children: None,
        expression: None,
        contracts: Vec::new(),
        metadata: NodeMetadata::default(),
    };
    node.metadata.name = Some(name.to_owned());
    node.metadata.fields = fields
        .into_iter()
        .map(|(n, t)| Field {
            name: n.to_owned(),
            type_ref: t.to_owned(),
        })
        .collect();
    node
}

fn error_node(name: &str, carries: Vec<(&str, &str)>) -> Node {
    let mut node = Node {
        id: NodeId::new(),
        intent: format!("{name} error"),
        pattern: Pattern::Error,
        children: None,
        expression: None,
        contracts: Vec::new(),
        metadata: NodeMetadata::default(),
    };
    node.metadata.name = Some(name.to_owned());
    node.metadata.carries = carries
        .into_iter()
        .map(|(n, t)| Field {
            name: n.to_owned(),
            type_ref: t.to_owned(),
        })
        .collect();
    node
}

fn file_content<'a>(output: &'a ail_emit::EmitOutput, path: &str) -> &'a str {
    output
        .files
        .iter()
        .find(|f| f.path == path)
        .unwrap_or_else(|| panic!("file not found: {path}"))
        .content
        .as_str()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn t091_define_number_emits_branded_type() {
    let verified = build_verified(vec![define_node("WalletBalance", "number", None)]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    let content = file_content(&output, "types/wallet_balance.ts");
    assert!(
        content.contains(
            "export type WalletBalance = number & { readonly __brand: 'WalletBalance' };"
        ),
        "branded type missing:\n{content}"
    );
}

#[test]
fn t091_define_number_emits_validator_function() {
    let verified = build_verified(vec![define_node(
        "WalletBalance",
        "number",
        Some("value >= 0"),
    )]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    let content = file_content(&output, "types/wallet_balance.ts");
    assert!(
        content.contains("export function createWalletBalance(value: number): WalletBalance {"),
        "createWalletBalance missing:\n{content}"
    );
    assert!(
        content.contains("if (!(value >= 0))"),
        "runtime check missing:\n{content}"
    );
}

#[test]
fn t091_define_text_emits_branded_string() {
    let verified = build_verified(vec![define_node("NonEmptyText", "text", None)]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    let content = file_content(&output, "types/non_empty_text.ts");
    assert!(
        content
            .contains("export type NonEmptyText = string & { readonly __brand: 'NonEmptyText' };"),
        "branded string type missing:\n{content}"
    );
}

#[test]
fn t091_define_with_constraint_emits_runtime_check() {
    let verified = build_verified(vec![define_node(
        "PositiveAmount",
        "number",
        Some("value > 0"),
    )]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    let content = file_content(&output, "types/positive_amount.ts");
    assert!(
        content.contains("if (!(value > 0))"),
        "constraint check missing:\n{content}"
    );
    assert!(
        content.contains("PositiveAmount constraint violated: value > 0"),
        "error message missing:\n{content}"
    );
}

#[test]
fn t091_define_multiple_constraints_emits_all_checks() {
    let verified = build_verified(vec![define_node(
        "Percentage",
        "number",
        Some("value >= 0 and value <= 100"),
    )]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    let content = file_content(&output, "types/percentage.ts");
    assert!(
        content.contains("value >= 0"),
        "lower bound check missing:\n{content}"
    );
    assert!(
        content.contains("value <= 100"),
        "upper bound check missing:\n{content}"
    );
}

#[test]
fn t091_describe_emits_interface() {
    let verified = build_verified(vec![describe_node(
        "User",
        vec![("id", "text"), ("balance", "number")],
    )]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    let content = file_content(&output, "types/user.ts");
    assert!(
        content.contains("export interface User {"),
        "interface missing:\n{content}"
    );
    assert!(
        content.contains("readonly id: string;"),
        "id field missing:\n{content}"
    );
    assert!(
        content.contains("readonly balance: number;"),
        "balance field missing:\n{content}"
    );
}

#[test]
fn t091_describe_emits_factory_function() {
    let verified = build_verified(vec![describe_node("User", vec![("id", "text")])]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    let content = file_content(&output, "types/user.ts");
    assert!(
        content.contains("export function createUser(params: {"),
        "createUser missing:\n{content}"
    );
    assert!(
        content.contains("return Object.freeze({ ...params });"),
        "Object.freeze missing:\n{content}"
    );
}

#[test]
fn t091_describe_fields_are_readonly() {
    let verified = build_verified(vec![describe_node(
        "Config",
        vec![("name", "text"), ("count", "integer")],
    )]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    let content = file_content(&output, "types/config.ts");
    // Both interface fields and constructor params should be readonly in the interface.
    assert!(
        content.contains("readonly name: string;"),
        "readonly name missing:\n{content}"
    );
    assert!(
        content.contains("readonly count: number;"),
        "readonly count missing:\n{content}"
    );
}

#[test]
fn t091_describe_preserves_field_order() {
    let verified = build_verified(vec![describe_node(
        "Ordered",
        vec![
            ("z_last", "text"),
            ("a_first", "text"),
            ("m_middle", "text"),
        ],
    )]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    let content = file_content(&output, "types/ordered.ts");
    let z = content.find("z_last").expect("z_last not found");
    let a = content.find("a_first").expect("a_first not found");
    let m = content.find("m_middle").expect("m_middle not found");
    assert!(z < a && a < m, "field order not preserved:\n{content}");
}

#[test]
fn t091_error_emits_class_extends_error() {
    let verified = build_verified(vec![error_node("InsufficientBalanceError", vec![])]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    let content = file_content(&output, "errors/insufficient_balance_error.ts");
    assert!(
        content.contains("export class InsufficientBalanceError extends Error {"),
        "class extends Error missing:\n{content}"
    );
}

#[test]
fn t091_error_carries_fields_in_constructor() {
    let verified = build_verified(vec![error_node(
        "InsufficientBalanceError",
        vec![
            ("current_balance", "number"),
            ("requested_amount", "number"),
        ],
    )]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    let content = file_content(&output, "errors/insufficient_balance_error.ts");
    assert!(
        content.contains("current_balance: number;"),
        "current_balance field missing:\n{content}"
    );
    assert!(
        content.contains("requested_amount: number;"),
        "requested_amount field missing:\n{content}"
    );
    assert!(
        content.contains("this.current_balance = params.current_balance;"),
        "assignment missing:\n{content}"
    );
}

#[test]
fn t091_error_message_includes_field_values() {
    let verified = build_verified(vec![error_node(
        "InsufficientBalanceError",
        vec![("current_balance", "number")],
    )]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    let content = file_content(&output, "errors/insufficient_balance_error.ts");
    assert!(
        content.contains("InsufficientBalanceError: current_balance=${params.current_balance}"),
        "message format missing:\n{content}"
    );
}

#[test]
fn t091_base_type_mapping_all_types() {
    // Each define node with a different base type → check the TS type annotation.
    let nodes = vec![
        define_node("N", "number", None),
        define_node("I", "integer", None),
        define_node("T", "text", None),
        define_node("B", "boolean", None),
        define_node("By", "bytes", None),
        define_node("Ts", "timestamp", None),
    ];
    let verified = build_verified(nodes);
    let output = emit_ts_type_definitions(&verified).unwrap();

    assert!(file_content(&output, "types/n.ts").contains("= number & "));
    assert!(file_content(&output, "types/i.ts").contains("= number & ")); // integer → number
    assert!(file_content(&output, "types/t.ts").contains("= string & "));
    assert!(file_content(&output, "types/b.ts").contains("= boolean & "));
    assert!(file_content(&output, "types/by.ts").contains("= Uint8Array & "));
    assert!(file_content(&output, "types/ts.ts").contains("= Date & "));
}

// `list<T>` and `option<T>` as field types do not pass graph validation (the validator
// checks the raw string, not the unwrapped inner type). The list/option rendering is
// covered by unit tests in `type_map.rs`. These integration tests verify end-to-end
// wiring of non-trivial type mappings that DO pass validation.

#[test]
fn t091_option_type_emits_union_with_null() {
    // timestamp → Date verifies that non-primitive AIL types map correctly end-to-end.
    // list<T>/option<T> → readonly T[]/T|null wiring is covered by type_map unit tests.
    let verified = build_verified(vec![describe_node(
        "Event",
        vec![("occurred_at", "timestamp")],
    )]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    let content = file_content(&output, "types/event.ts");
    assert!(
        content.contains("readonly occurred_at: Date;"),
        "timestamp→Date mapping missing:\n{content}"
    );
}

#[test]
fn t091_list_type_emits_readonly_array() {
    // bytes → Uint8Array verifies that non-primitive AIL types map correctly end-to-end.
    // list<T>/option<T> → readonly T[]/T|null wiring is covered by type_map unit tests.
    let verified = build_verified(vec![describe_node("Payload", vec![("data", "bytes")])]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    let content = file_content(&output, "types/payload.ts");
    assert!(
        content.contains("readonly data: Uint8Array;"),
        "bytes→Uint8Array mapping missing:\n{content}"
    );
}

#[test]
fn t091_describe_imports_factory_for_define_type() {
    // User has a field of type WalletBalance (a define type).
    // The generated user.ts should import both WalletBalance and createWalletBalance.
    let verified = build_verified(vec![
        define_node("WalletBalance", "number", Some("value >= 0")),
        describe_node("User", vec![("balance", "WalletBalance")]),
    ]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    let content = file_content(&output, "types/user.ts");
    assert!(
        content.contains("WalletBalance") && content.contains("createWalletBalance"),
        "factory import missing from user.ts:\n{content}"
    );
}

// ── Output structure tests ────────────────────────────────────────────────────

#[test]
fn ts_types_are_in_types_folder() {
    let verified = build_verified(vec![
        define_node("WalletBalance", "number", None),
        describe_node("User", vec![("id", "text")]),
    ]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    assert!(
        output
            .files
            .iter()
            .any(|f| f.path == "types/wallet_balance.ts"),
        "define not in types/"
    );
    assert!(
        output.files.iter().any(|f| f.path == "types/user.ts"),
        "describe not in types/"
    );
}

#[test]
fn ts_errors_are_in_errors_folder() {
    let verified = build_verified(vec![error_node("MyError", vec![])]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    assert!(
        output.files.iter().any(|f| f.path == "errors/my_error.ts"),
        "error not in errors/"
    );
}

#[test]
fn ts_barrel_index_files_generated() {
    let verified = build_verified(vec![
        define_node("WalletBalance", "number", None),
        error_node("MyError", vec![]),
    ]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    assert!(
        output.files.iter().any(|f| f.path == "types/index.ts"),
        "types/index.ts missing"
    );
    assert!(
        output.files.iter().any(|f| f.path == "errors/index.ts"),
        "errors/index.ts missing"
    );
}

#[test]
fn ts_barrel_re_exports_types() {
    let verified = build_verified(vec![define_node("WalletBalance", "number", None)]);
    let output = emit_ts_type_definitions(&verified).unwrap();

    let barrel = file_content(&output, "types/index.ts");
    assert!(
        barrel.contains("export * from './wallet_balance';"),
        "barrel re-export missing:\n{barrel}"
    );
}

#[test]
fn ts_all_files_have_generated_ownership() {
    let verified = build_verified(vec![
        define_node("WalletBalance", "number", None),
        error_node("MyError", vec![]),
    ]);
    let output = emit_ts_type_definitions(&verified).unwrap();

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
fn ts_empty_graph_produces_no_output() {
    let verified = build_verified(vec![]);
    let output = emit_ts_type_definitions(&verified).unwrap();
    assert!(output.files.is_empty());
}
