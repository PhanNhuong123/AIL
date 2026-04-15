use ail_graph::{
    validate_graph, AilGraph, Contract, ContractKind, EdgeKind, Expression, Node, NodeId, Param,
    Pattern, ValidationError,
};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_node(intent: &str, pattern: Pattern) -> Node {
    Node::new(NodeId::new(), intent, pattern)
}

fn make_named_node(intent: &str, pattern: Pattern, name: &str) -> Node {
    let mut node = Node::new(NodeId::new(), intent, pattern);
    node.metadata.name = Some(name.to_owned());
    node
}

fn before_contract() -> Contract {
    Contract {
        kind: ContractKind::Before,
        expression: Expression("x > 0".to_owned()),
    }
}

fn after_contract() -> Contract {
    Contract {
        kind: ContractKind::After,
        expression: Expression("result >= 0".to_owned()),
    }
}

/// Minimal valid graph: one root Do node with pre + post contracts.
fn minimal_valid_graph() -> AilGraph {
    let mut graph = AilGraph::new();
    let mut root = make_named_node("transfer money", Pattern::Do, "transfer_money");
    root.contracts.push(before_contract());
    root.contracts.push(after_contract());
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();
    graph
}

// ─── v001: non-empty intent ───────────────────────────────────────────────────

#[test]
fn v001_rejects_node_with_empty_intent() {
    let mut graph = AilGraph::new();
    let mut root = make_node("", Pattern::Do);
    root.contracts.push(before_contract());
    root.contracts.push(after_contract());
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let errors = validate_graph(graph).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ValidationError::EmptyIntent { .. })),
        "expected EmptyIntent error, got: {errors:?}"
    );
}

#[test]
fn v001_accepts_node_with_nonempty_intent() {
    let result = validate_graph(minimal_valid_graph());
    assert!(
        result.is_ok(),
        "expected Ok(ValidGraph), got: {:?}",
        result.unwrap_err()
    );
}

// ─── v002: Ev forms a tree ────────────────────────────────────────────────────

#[test]
fn v002_rejects_node_with_multiple_ev_parents() {
    let mut graph = AilGraph::new();
    let mut root = make_named_node("root function", Pattern::Do, "root_fn");
    root.contracts.push(before_contract());
    root.contracts.push(after_contract());
    root.children = Some(vec![]);
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let mut parent2 = make_node("second parent", Pattern::Do);
    parent2.children = Some(vec![]);
    let parent2_id = graph.add_node(parent2).unwrap();
    graph.add_edge(root_id, parent2_id, EdgeKind::Ev).unwrap();

    let mut child = make_node("shared child", Pattern::Let);
    child.expression = Some(Expression("x + 1".to_owned()));
    let child_id = graph.add_node(child).unwrap();

    // Add two Ev parents → should fail
    graph.add_edge(root_id, child_id, EdgeKind::Ev).unwrap();
    graph.add_edge(parent2_id, child_id, EdgeKind::Ev).unwrap();

    let errors = validate_graph(graph).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ValidationError::EvMultipleParents { .. })),
        "expected EvMultipleParents error, got: {errors:?}"
    );
}

#[test]
fn v002_rejects_ev_cycle() {
    let mut graph = AilGraph::new();
    let a = graph.add_node(make_node("node a", Pattern::Do)).unwrap();
    graph.set_root(a).unwrap();
    let b = graph.add_node(make_node("node b", Pattern::Let)).unwrap();
    let c = graph.add_node(make_node("node c", Pattern::Let)).unwrap();
    graph.add_edge(a, b, EdgeKind::Ev).unwrap();
    graph.add_edge(b, c, EdgeKind::Ev).unwrap();
    graph.add_edge(c, a, EdgeKind::Ev).unwrap(); // cycle back to root

    let errors = validate_graph(graph).unwrap_err();
    // Either cycle detected or multiple parents (since root gets a second parent)
    assert!(
        errors.iter().any(|e| {
            matches!(
                e,
                ValidationError::EvCycleDetected { .. } | ValidationError::EvMultipleParents { .. }
            )
        }),
        "expected cycle or multiple parents error, got: {errors:?}"
    );
}

// ─── v003: all nodes reachable ────────────────────────────────────────────────

#[test]
fn v003_rejects_unreachable_node() {
    let mut graph = minimal_valid_graph();
    // Add a disconnected node that is never linked by an Ev edge.
    let _orphan = graph
        .add_node(make_node("orphan node", Pattern::Define))
        .unwrap();

    let errors = validate_graph(graph).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ValidationError::UnreachableNode { .. })),
        "expected UnreachableNode error, got: {errors:?}"
    );
}

#[test]
fn v003_accepts_fully_connected_graph() {
    let mut graph = AilGraph::new();
    let mut root = make_named_node("root function", Pattern::Do, "root_fn");
    root.contracts.push(before_contract());
    root.contracts.push(after_contract());
    root.children = Some(vec![]);
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let mut child = make_node("compute value", Pattern::Let);
    child.expression = Some(Expression("x + 1".to_owned()));
    let child_id = graph.add_node(child).unwrap();
    graph.add_edge(root_id, child_id, EdgeKind::Ev).unwrap();

    let result = validate_graph(graph);
    assert!(
        result.is_ok(),
        "expected valid graph, got: {:?}",
        result.unwrap_err()
    );
}

// ─── v004: only leaves carry expressions ──────────────────────────────────────

#[test]
fn v004_rejects_expression_on_structural_node() {
    let mut graph = AilGraph::new();
    let mut root = make_named_node("root function", Pattern::Do, "root_fn");
    root.contracts.push(before_contract());
    root.contracts.push(after_contract());
    root.children = Some(vec![]); // structural node
    root.expression = Some(Expression("x + 1".to_owned())); // should not have expression
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let errors = validate_graph(graph).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ValidationError::ExpressionOnStructuralNode { .. })),
        "expected ExpressionOnStructuralNode error, got: {errors:?}"
    );
}

#[test]
fn v004_accepts_expression_on_leaf() {
    let mut graph = AilGraph::new();
    let mut root = make_named_node("root function", Pattern::Do, "root_fn");
    root.contracts.push(before_contract());
    root.contracts.push(after_contract());
    root.children = Some(vec![]);
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    // Leaf node with expression — valid
    let mut leaf = make_node("compute balance", Pattern::Let);
    leaf.expression = Some(Expression("balance - amount".to_owned())); // leaf, OK
    let leaf_id = graph.add_node(leaf).unwrap();
    graph.add_edge(root_id, leaf_id, EdgeKind::Ev).unwrap();

    let result = validate_graph(graph);
    assert!(
        result.is_ok(),
        "expected valid graph, got: {:?}",
        result.unwrap_err()
    );
}

// ─── v005: top-level Do has pre + post contracts ──────────────────────────────

#[test]
fn v005_rejects_do_without_pre_contract() {
    let mut graph = AilGraph::new();
    let mut root = make_named_node("transfer money", Pattern::Do, "transfer_money");
    root.contracts.push(after_contract()); // After only — missing Before
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let errors = validate_graph(graph).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ValidationError::MissingPreContract { .. })),
        "expected MissingPreContract error, got: {errors:?}"
    );
}

#[test]
fn v005_rejects_do_without_post_contract() {
    let mut graph = AilGraph::new();
    let mut root = make_named_node("transfer money", Pattern::Do, "transfer_money");
    root.contracts.push(before_contract()); // Before only — missing After
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let errors = validate_graph(graph).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ValidationError::MissingPostContract { .. })),
        "expected MissingPostContract error, got: {errors:?}"
    );
}

#[test]
fn v005_accepts_do_with_both_contracts() {
    let result = validate_graph(minimal_valid_graph());
    assert!(
        result.is_ok(),
        "expected valid graph, got: {:?}",
        result.unwrap_err()
    );
}

#[test]
fn v005_accepts_nested_do_without_contracts() {
    // A Do node nested inside another Do does not need its own contracts.
    let mut graph = AilGraph::new();
    let mut root = make_named_node("outer function", Pattern::Do, "outer_fn");
    root.contracts.push(before_contract());
    root.contracts.push(after_contract());
    root.children = Some(vec![]);
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    // Nested Do with no contracts — should be accepted (not top-level)
    let mut nested = make_named_node("inner step", Pattern::Do, "inner_step");
    nested.children = Some(vec![]);
    let nested_id = graph.add_node(nested).unwrap();
    graph.add_edge(root_id, nested_id, EdgeKind::Ev).unwrap();

    let mut leaf = make_node("compute value", Pattern::Let);
    leaf.expression = Some(Expression("x * 2".to_owned()));
    let leaf_id = graph.add_node(leaf).unwrap();
    graph.add_edge(nested_id, leaf_id, EdgeKind::Ev).unwrap();

    let result = validate_graph(graph);
    assert!(
        result.is_ok(),
        "nested Do without contracts should be valid, got: {:?}",
        result.unwrap_err()
    );
}

// ─── v006: type references resolve ───────────────────────────────────────────

#[test]
fn v006_rejects_unresolved_type_reference() {
    let mut graph = AilGraph::new();
    let mut root = make_named_node("do something", Pattern::Do, "do_something");
    root.contracts.push(before_contract());
    root.contracts.push(after_contract());
    root.metadata.params = vec![Param {
        name: "amount".to_owned(),
        type_ref: "NonExistentType".to_owned(), // not defined anywhere
    }];
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let errors = validate_graph(graph).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ValidationError::UnresolvedTypeReference { .. })),
        "expected UnresolvedTypeReference error, got: {errors:?}"
    );
}

#[test]
fn v006_accepts_resolved_type_reference() {
    let mut graph = AilGraph::new();

    // Define a type that will be referenced.
    let mut amount_type =
        make_named_node("represents a monetary amount", Pattern::Describe, "Amount");
    amount_type.metadata.fields = vec![];
    let type_id = graph.add_node(amount_type).unwrap();

    let mut root = make_named_node("do something", Pattern::Do, "do_something");
    root.contracts.push(before_contract());
    root.contracts.push(after_contract());
    root.metadata.params = vec![Param {
        name: "amount".to_owned(),
        type_ref: "Amount".to_owned(), // resolves to the Describe node above
    }];
    root.children = Some(vec![]);
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    // Connect the type node via an Ed edge and also make it reachable via Ev.
    graph.add_edge(root_id, type_id, EdgeKind::Ev).unwrap();
    graph.add_edge(root_id, type_id, EdgeKind::Ed).unwrap();

    let result = validate_graph(graph);
    assert!(
        result.is_ok(),
        "expected valid graph, got: {:?}",
        result.unwrap_err()
    );
}

#[test]
fn v006_accepts_builtin_type_names() {
    let mut graph = AilGraph::new();
    let mut root = make_named_node("do something", Pattern::Do, "do_something");
    root.contracts.push(before_contract());
    root.contracts.push(after_contract());
    root.metadata.params = vec![
        Param {
            name: "a".to_owned(),
            type_ref: "text".to_owned(),
        },
        Param {
            name: "b".to_owned(),
            type_ref: "integer".to_owned(),
        },
    ];
    root.metadata.return_type = Some("boolean".to_owned());
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let result = validate_graph(graph);
    assert!(
        result.is_ok(),
        "built-in type names should be accepted, got: {:?}",
        result.unwrap_err()
    );
}

#[test]
fn v006_accepts_qualified_type_refs_without_error() {
    // Qualified names (containing '.') are skipped — no false positive.
    let mut graph = AilGraph::new();
    let mut root = make_named_node("do something", Pattern::Do, "do_something");
    root.contracts.push(before_contract());
    root.contracts.push(after_contract());
    root.metadata.params = vec![Param {
        name: "invoice".to_owned(),
        type_ref: "billing.Invoice".to_owned(), // qualified — skip validation
    }];
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let result = validate_graph(graph);
    assert!(
        result.is_ok(),
        "qualified type refs should not produce errors, got: {:?}",
        result.unwrap_err()
    );
}

// ─── v007: no duplicate names in scope ───────────────────────────────────────

#[test]
fn v007_rejects_duplicate_names_in_scope() {
    let mut graph = AilGraph::new();
    let mut root = make_named_node("root function", Pattern::Do, "root_fn");
    root.contracts.push(before_contract());
    root.contracts.push(after_contract());
    root.children = Some(vec![]);
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    // Two children with the same name in the same scope.
    let child1 = make_named_node("describe user v1", Pattern::Describe, "User");
    let child1_id = graph.add_node(child1).unwrap();
    let child2 = make_named_node("describe user v2", Pattern::Describe, "User");
    let child2_id = graph.add_node(child2).unwrap();

    graph.add_edge(root_id, child1_id, EdgeKind::Ev).unwrap();
    graph.add_edge(root_id, child2_id, EdgeKind::Ev).unwrap();

    let errors = validate_graph(graph).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ValidationError::DuplicateNameInScope { .. })),
        "expected DuplicateNameInScope error, got: {errors:?}"
    );
}

#[test]
fn v007_accepts_same_name_in_different_scopes() {
    let mut graph = AilGraph::new();
    let mut root = make_named_node("root function", Pattern::Do, "root_fn");
    root.contracts.push(before_contract());
    root.contracts.push(after_contract());
    root.children = Some(vec![]);
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    // Two sibling sub-modules, each containing a "User" type.
    let mut module_a = make_named_node("billing module", Pattern::Do, "billing");
    module_a.children = Some(vec![]);
    let module_a_id = graph.add_node(module_a).unwrap();
    graph.add_edge(root_id, module_a_id, EdgeKind::Ev).unwrap();

    let mut module_b = make_named_node("auth module", Pattern::Do, "auth");
    module_b.children = Some(vec![]);
    let module_b_id = graph.add_node(module_b).unwrap();
    graph.add_edge(root_id, module_b_id, EdgeKind::Ev).unwrap();

    // Same name "User" in different scopes (module_a and module_b).
    let user_a = make_named_node("billing user", Pattern::Describe, "User");
    let user_a_id = graph.add_node(user_a).unwrap();
    graph
        .add_edge(module_a_id, user_a_id, EdgeKind::Ev)
        .unwrap();

    let user_b = make_named_node("auth user", Pattern::Describe, "User");
    let user_b_id = graph.add_node(user_b).unwrap();
    graph
        .add_edge(module_b_id, user_b_id, EdgeKind::Ev)
        .unwrap();

    let result = validate_graph(graph);
    assert!(
        result.is_ok(),
        "same name in different scopes should be valid, got: {:?}",
        result.unwrap_err()
    );
}

#[test]
fn v007_accepts_same_name_in_different_branches() {
    // Same name under sibling branches (different parents) is allowed.
    let mut graph = AilGraph::new();
    let mut root = make_named_node("outer fn", Pattern::Do, "outer_fn");
    root.contracts.push(before_contract());
    root.contracts.push(after_contract());
    root.children = Some(vec![]);
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let mut branch_a = make_named_node("execute branch", Pattern::Do, "execute");
    branch_a.children = Some(vec![]);
    let branch_a_id = graph.add_node(branch_a).unwrap();
    graph.add_edge(root_id, branch_a_id, EdgeKind::Ev).unwrap();

    let mut branch_b = make_named_node("rollback branch", Pattern::Do, "rollback");
    branch_b.children = Some(vec![]);
    let branch_b_id = graph.add_node(branch_b).unwrap();
    graph.add_edge(root_id, branch_b_id, EdgeKind::Ev).unwrap();

    // Same binding name "new_balance" in each branch — different scopes.
    let let_a = make_named_node("compute balance in execute", Pattern::Let, "new_balance");
    let let_a_id = graph.add_node(let_a).unwrap();
    graph.add_edge(branch_a_id, let_a_id, EdgeKind::Ev).unwrap();

    let let_b = make_named_node("compute balance in rollback", Pattern::Let, "new_balance");
    let let_b_id = graph.add_node(let_b).unwrap();
    graph.add_edge(branch_b_id, let_b_id, EdgeKind::Ev).unwrap();

    let result = validate_graph(graph);
    assert!(
        result.is_ok(),
        "same name in sibling branches should be valid, got: {:?}",
        result.unwrap_err()
    );
}

// ─── v008: following template phases ─────────────────────────────────────────

#[test]
fn v008_rejects_do_missing_template_phase() {
    let mut graph = AilGraph::new();

    // Template: defines phases "validate", "execute", "return".
    let mut template = make_named_node("command flow template", Pattern::Do, "command_flow");
    template.children = Some(vec![]);
    let template_id = graph.add_node(template).unwrap();

    let phase_validate = make_named_node("validate the input", Pattern::Do, "validate");
    let phase_validate_id = graph.add_node(phase_validate).unwrap();
    graph
        .add_edge(template_id, phase_validate_id, EdgeKind::Ev)
        .unwrap();

    let phase_execute = make_named_node("execute the logic", Pattern::Do, "execute");
    let phase_execute_id = graph.add_node(phase_execute).unwrap();
    graph
        .add_edge(template_id, phase_execute_id, EdgeKind::Ev)
        .unwrap();

    let phase_return = make_named_node("return the result", Pattern::Do, "return_result");
    let phase_return_id = graph.add_node(phase_return).unwrap();
    graph
        .add_edge(template_id, phase_return_id, EdgeKind::Ev)
        .unwrap();

    // Do node following the template, but missing the "return_result" phase.
    let mut root = make_named_node("transfer money", Pattern::Do, "transfer_money");
    root.contracts.push(before_contract());
    root.contracts.push(after_contract());
    root.children = Some(vec![]);
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    // Link root to template via Ed (following).
    graph.add_edge(root_id, template_id, EdgeKind::Ed).unwrap();

    // Root only implements "validate" and "execute", not "return_result".
    let impl_validate = make_named_node("validates inputs", Pattern::Do, "validate");
    let impl_validate_id = graph.add_node(impl_validate).unwrap();
    graph
        .add_edge(root_id, impl_validate_id, EdgeKind::Ev)
        .unwrap();

    let impl_execute = make_named_node("executes logic", Pattern::Do, "execute");
    let impl_execute_id = graph.add_node(impl_execute).unwrap();
    graph
        .add_edge(root_id, impl_execute_id, EdgeKind::Ev)
        .unwrap();

    // Make template reachable (attach via Ev from a sub-node or add as sibling).
    // For this test we attach template directly to root as an Ev child so it's reachable.
    graph.add_edge(root_id, template_id, EdgeKind::Ev).unwrap();

    // Also make template phases reachable through template.
    let _ = phase_validate_id;
    let _ = phase_execute_id;
    let _ = phase_return_id;

    let errors = validate_graph(graph).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ValidationError::MissingTemplatePhase { .. })),
        "expected MissingTemplatePhase error, got: {errors:?}"
    );
}

#[test]
fn v008_accepts_do_covering_all_template_phases() {
    let mut graph = AilGraph::new();

    // Template with two required phases.
    let mut template = make_named_node("simple flow template", Pattern::Do, "simple_flow");
    template.children = Some(vec![]);
    let template_id = graph.add_node(template).unwrap();

    let t_validate = make_named_node("validate", Pattern::Do, "validate");
    let t_validate_id = graph.add_node(t_validate).unwrap();
    graph
        .add_edge(template_id, t_validate_id, EdgeKind::Ev)
        .unwrap();

    let t_execute = make_named_node("execute", Pattern::Do, "execute");
    let t_execute_id = graph.add_node(t_execute).unwrap();
    graph
        .add_edge(template_id, t_execute_id, EdgeKind::Ev)
        .unwrap();
    let _ = t_execute_id;
    let _ = t_validate_id;

    // Do node that implements all template phases.
    let mut root = make_named_node("process order", Pattern::Do, "process_order");
    root.contracts.push(before_contract());
    root.contracts.push(after_contract());
    root.children = Some(vec![]);
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    // Link root to template via Ed (following) and Ev (reachability).
    graph.add_edge(root_id, template_id, EdgeKind::Ed).unwrap();
    graph.add_edge(root_id, template_id, EdgeKind::Ev).unwrap();

    // Implement both phases.
    let impl_validate = make_named_node("validates inputs", Pattern::Do, "validate");
    let impl_validate_id = graph.add_node(impl_validate).unwrap();
    graph
        .add_edge(root_id, impl_validate_id, EdgeKind::Ev)
        .unwrap();

    let impl_execute = make_named_node("executes logic", Pattern::Do, "execute");
    let impl_execute_id = graph.add_node(impl_execute).unwrap();
    graph
        .add_edge(root_id, impl_execute_id, EdgeKind::Ev)
        .unwrap();

    let result = validate_graph(graph);
    assert!(
        result.is_ok(),
        "all template phases covered should be valid, got: {:?}",
        result.unwrap_err()
    );
}

// ─── Integration: error accumulation ─────────────────────────────────────────

#[test]
fn validate_collects_all_errors() {
    let mut graph = AilGraph::new();
    // Root node with empty intent AND missing contracts — two distinct errors.
    let mut root = Node::new(NodeId::new(), "", Pattern::Do); // empty intent
                                                              // No contracts → also missing pre + post.
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let errors = validate_graph(graph).unwrap_err();
    let has_empty_intent = errors
        .iter()
        .any(|e| matches!(e, ValidationError::EmptyIntent { .. }));
    let has_missing_pre = errors
        .iter()
        .any(|e| matches!(e, ValidationError::MissingPreContract { .. }));
    let has_missing_post = errors
        .iter()
        .any(|e| matches!(e, ValidationError::MissingPostContract { .. }));
    assert!(
        has_empty_intent && has_missing_pre && has_missing_post,
        "expected all three errors accumulated, got: {errors:?}"
    );
}

// ─── v009: using-Do node constraints ─────────────────────────────────────────

/// Build a graph where a Do node has `using_pattern_name` set.
/// Optionally add children and/or an Ed edge to a target Do node.
fn using_do_graph(with_children: bool, with_ed_edge: bool) -> (AilGraph, NodeId) {
    let mut graph = AilGraph::new();

    // Shared-pattern Do node (the referenced one).
    let mut shared = make_named_node(
        "save entity to database",
        Pattern::Do,
        "save_entity_to_database",
    );
    shared.contracts.push(before_contract());
    shared.contracts.push(after_contract());
    let shared_id = graph.add_node(shared).unwrap();

    // The using-Do node.
    let mut root = make_named_node("save sender balance", Pattern::Do, "save_sender_balance");
    root.contracts.push(before_contract());
    root.contracts.push(after_contract());
    root.metadata.using_pattern_name = Some("save_entity_to_database".to_owned());
    root.metadata.using_params = vec![("entity".to_owned(), "sender".to_owned())];

    if with_children {
        root.children = Some(vec![]); // v009 should reject this
    }

    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    // Shared is reachable via Ev from root (v009 checks node.children field, not Ev edges).
    graph.add_edge(root_id, shared_id, EdgeKind::Ev).unwrap();

    if with_ed_edge {
        graph.add_edge(root_id, shared_id, EdgeKind::Ed).unwrap();
    }

    (graph, root_id)
}

#[test]
fn v009_rejects_using_do_with_children() {
    let (graph, _) = using_do_graph(true, true);
    let errors = validate_graph(graph).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ValidationError::UsingDoHasChildren { .. })),
        "expected UsingDoHasChildren error, got: {errors:?}"
    );
}

#[test]
fn v009_rejects_using_do_missing_ed_edge() {
    let (graph, _) = using_do_graph(false, false); // no Ed edge
    let errors = validate_graph(graph).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ValidationError::UsingDoMissingEdge { .. })),
        "expected UsingDoMissingEdge error, got: {errors:?}"
    );
}

#[test]
fn v009_accepts_valid_using_do_leaf_with_ed_edge() {
    let (graph, _) = using_do_graph(false, true);
    let result = validate_graph(graph);
    assert!(
        result.is_ok(),
        "valid using-Do should pass validation, got: {:?}",
        result.unwrap_err()
    );
}

#[test]
fn v009_accumulates_children_and_missing_edge_errors() {
    // A using-Do that BOTH has children AND lacks an Ed edge — both errors reported.
    let (graph, _) = using_do_graph(true, false);
    let errors = validate_graph(graph).unwrap_err();
    let has_children_err = errors
        .iter()
        .any(|e| matches!(e, ValidationError::UsingDoHasChildren { .. }));
    let has_edge_err = errors
        .iter()
        .any(|e| matches!(e, ValidationError::UsingDoMissingEdge { .. }));
    assert!(
        has_children_err && has_edge_err,
        "expected both UsingDoHasChildren and UsingDoMissingEdge, got: {errors:?}"
    );
}

#[test]
fn v009_ignores_do_without_using_pattern_name() {
    // A normal Do node (no using_pattern_name) must not trigger v009 errors.
    let result = validate_graph(minimal_valid_graph());
    assert!(
        result.is_ok(),
        "normal Do without using_pattern_name must not trigger v009, got: {:?}",
        result.unwrap_err()
    );
}

#[test]
fn v009_ignores_other_pattern_types_with_ed_edges() {
    // An Ed edge from a non-Do node should not trigger v009.
    let mut graph = AilGraph::new();
    let mut root = make_named_node("transfer money", Pattern::Do, "transfer_money");
    root.contracts.push(before_contract());
    root.contracts.push(after_contract());
    root.children = Some(vec![]);
    let root_id = graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    // Describe node with an Ed edge — not a using-Do, should be fine.
    let desc = make_named_node("user type", Pattern::Describe, "User");
    let desc_id = graph.add_node(desc).unwrap();
    graph.add_edge(root_id, desc_id, EdgeKind::Ev).unwrap();
    graph.add_edge(root_id, desc_id, EdgeKind::Ed).unwrap();

    let result = validate_graph(graph);
    assert!(
        result.is_ok(),
        "non-Do nodes with Ed edges must not trigger v009, got: {:?}",
        result.unwrap_err()
    );
}

// ─── Integration: ValidGraph API ─────────────────────────────────────────────

#[test]
fn validate_well_formed_graph_returns_valid_graph() {
    let result = validate_graph(minimal_valid_graph());
    assert!(
        result.is_ok(),
        "expected Ok(ValidGraph), got: {:?}",
        result.unwrap_err()
    );
}

#[test]
fn validate_valid_graph_preserves_graph_data() {
    let graph = minimal_valid_graph();
    let node_count = graph.node_ids().count();

    let valid = validate_graph(graph).unwrap();
    assert_eq!(
        valid.graph().all_node_ids().unwrap().len(),
        node_count,
        "ValidGraph.graph() must expose the same nodes"
    );

    let inner = valid.into_inner();
    assert_eq!(
        inner.node_ids().count(),
        node_count,
        "into_inner() must preserve all nodes"
    );
}
