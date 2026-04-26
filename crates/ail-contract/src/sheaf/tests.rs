//! Unit tests for Čech nerve construction (Phase 17 task 17.1).
//!
//! Fixture helpers are re-implemented here rather than imported from
//! `z3_verify/tests.rs` because those helpers are gated by
//! `#[cfg(feature = "z3-verify")]` and unavailable on default features.

use ail_graph::types::EdgeKind;
use ail_graph::{
    validate_graph, AilGraph, Contract, ContractKind, Expression, GraphBackend, Node, NodeId,
    Param, Pattern,
};
use ail_types::{type_check, TypedGraph};

use crate::sheaf::build_nerve;
use crate::verify::verify;

use super::builder::extract_vars;

// ─── Fixture helpers ──────────────────────────────────────────────────────────

fn make_typed(graph: AilGraph) -> TypedGraph {
    let valid = validate_graph(graph).expect("test graph must be valid");
    type_check(valid, &[]).expect("test graph must type-check")
}

fn before(expr: &str) -> Contract {
    Contract {
        kind: ContractKind::Before,
        expression: Expression(expr.to_string()),
    }
}

fn after(expr: &str) -> Contract {
    Contract {
        kind: ContractKind::After,
        expression: Expression(expr.to_string()),
    }
}

fn always(expr: &str) -> Contract {
    Contract {
        kind: ContractKind::Always,
        expression: Expression(expr.to_string()),
    }
}

fn param(name: &str, type_ref: &str) -> Param {
    Param {
        name: name.to_string(),
        type_ref: type_ref.to_string(),
    }
}

/// A root Describe node with no children.
fn empty_describe_graph() -> AilGraph {
    let mut graph = AilGraph::new();
    let mut root = Node::new(NodeId::new(), "root", Pattern::Describe);
    root.children = Some(vec![]);
    let id = graph.add_node(root).unwrap();
    graph.set_root(id).unwrap();
    graph
}

/// Add a Do child to an existing structural node, wire Ev edge, push child
/// into parent's `children` list. Returns the new child's `NodeId`.
fn add_do_child(
    graph: &mut AilGraph,
    parent_id: NodeId,
    label: &str,
    name: &str,
    params: Vec<Param>,
    return_type: Option<&str>,
    contracts: Vec<Contract>,
) -> NodeId {
    let mut n = Node::new(NodeId::new(), label, Pattern::Do);
    n.metadata.name = Some(name.to_string());
    n.metadata.params = params;
    n.metadata.return_type = return_type.map(str::to_string);
    n.contracts = contracts;
    let id = graph.add_node(n).unwrap();
    graph.add_edge(parent_id, id, EdgeKind::Ev).unwrap();
    graph
        .get_node_mut(parent_id)
        .unwrap()
        .children
        .as_mut()
        .unwrap()
        .push(id);
    id
}

/// Add a structural (non-leaf) Do node — sets `children = Some(vec![])`.
fn add_do_parent(
    graph: &mut AilGraph,
    parent_id: NodeId,
    label: &str,
    name: &str,
    params: Vec<Param>,
    contracts: Vec<Contract>,
) -> NodeId {
    let mut n = Node::new(NodeId::new(), label, Pattern::Do);
    n.metadata.name = Some(name.to_string());
    n.metadata.params = params;
    n.contracts = contracts;
    n.children = Some(vec![]);
    let id = graph.add_node(n).unwrap();
    graph.add_edge(parent_id, id, EdgeKind::Ev).unwrap();
    graph
        .get_node_mut(parent_id)
        .unwrap()
        .children
        .as_mut()
        .unwrap()
        .push(id);
    id
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[test]
fn sheaf_empty_graph_returns_empty_nerve() {
    let graph = empty_describe_graph();
    let typed = make_typed(graph);
    let verified = verify(typed).expect("test graph must verify");
    let nerve = build_nerve(&verified);
    assert!(nerve.sections.is_empty(), "no Do nodes → no sections");
    assert!(nerve.overlaps.is_empty(), "no Do nodes → no overlaps");
}

#[test]
fn sheaf_single_do_node_has_one_section_no_overlaps() {
    let mut graph = empty_describe_graph();
    let root_id = graph.root_nodes().unwrap()[0];
    let _do_id = add_do_child(
        &mut graph,
        root_id,
        "transfer",
        "transfer",
        vec![param("amount", "NonNegativeInteger")],
        None,
        vec![before("amount >= 0"), after("amount >= 0")],
    );
    let typed = make_typed(graph);
    let verified = verify(typed).expect("test graph must verify");
    let nerve = build_nerve(&verified);
    assert_eq!(nerve.sections.len(), 1);
    assert!(!nerve.sections[0].constraints.is_empty());
    assert!(nerve.overlaps.is_empty());
}

#[test]
fn sheaf_parent_child_do_nodes_produce_one_overlap() {
    let mut graph = empty_describe_graph();
    let root_id = graph.root_nodes().unwrap()[0];

    // Parent Do (structural — has children).
    let parent_id = add_do_parent(
        &mut graph,
        root_id,
        "outer op",
        "outer_op",
        vec![param("amount", "NonNegativeInteger")],
        vec![before("amount >= 0"), after("amount >= 0")],
    );

    // Child Do (leaf under parent).
    let child_id = add_do_child(
        &mut graph,
        parent_id,
        "inner op",
        "inner_op",
        vec![param("amount", "NonNegativeInteger")],
        None,
        vec![before("amount >= 0"), after("amount >= 0")],
    );

    let typed = make_typed(graph);
    let verified = verify(typed).expect("test graph must verify");
    let nerve = build_nerve(&verified);

    assert_eq!(nerve.overlaps.len(), 1, "exactly one parent-child overlap");
    let ov = &nerve.overlaps[0];
    assert_eq!(ov.node_a, parent_id);
    assert_eq!(ov.node_b, child_id);
    assert!(!ov.combined.is_empty());
}

#[test]
fn sheaf_three_generation_chain_has_two_overlaps_not_three() {
    let mut graph = empty_describe_graph();
    let root_id = graph.root_nodes().unwrap()[0];

    let gp_id = add_do_parent(
        &mut graph,
        root_id,
        "grandparent",
        "grandparent",
        vec![param("x", "NonNegativeInteger")],
        vec![before("x >= 0"), after("x >= 0")],
    );

    let p_id = add_do_parent(
        &mut graph,
        gp_id,
        "parent",
        "parent_fn",
        vec![param("x", "NonNegativeInteger")],
        vec![before("x >= 0"), after("x >= 0")],
    );

    let c_id = add_do_child(
        &mut graph,
        p_id,
        "child",
        "child_fn",
        vec![param("x", "NonNegativeInteger")],
        None,
        vec![before("x >= 0"), after("x >= 0")],
    );

    let typed = make_typed(graph);
    let verified = verify(typed).expect("test graph must verify");
    let nerve = build_nerve(&verified);

    assert_eq!(
        nerve.overlaps.len(),
        2,
        "two direct overlaps (gp,p) and (p,c)"
    );

    let has_gp_p = nerve
        .overlaps
        .iter()
        .any(|o| o.node_a == gp_id && o.node_b == p_id);
    let has_p_c = nerve
        .overlaps
        .iter()
        .any(|o| o.node_a == p_id && o.node_b == c_id);
    let has_gp_c = nerve
        .overlaps
        .iter()
        .any(|o| o.node_a == gp_id && o.node_b == c_id);

    assert!(has_gp_p, "overlap (grandparent, parent) must exist");
    assert!(has_p_c, "overlap (parent, child) must exist");
    assert!(
        !has_gp_c,
        "transitive overlap (grandparent, child) must NOT exist"
    );
}

#[test]
fn sheaf_siblings_with_shared_var_produce_overlap() {
    let mut graph = empty_describe_graph();
    let root_id = graph.root_nodes().unwrap()[0];

    let earlier_id = add_do_child(
        &mut graph,
        root_id,
        "first op",
        "first_op",
        vec![param("amount", "NonNegativeInteger")],
        None,
        vec![before("amount >= 0"), after("amount >= 0")],
    );

    let later_id = add_do_child(
        &mut graph,
        root_id,
        "second op",
        "second_op",
        vec![param("amount", "NonNegativeInteger")],
        None,
        vec![before("amount >= 0"), after("amount >= 0")],
    );

    // Wire Eh edge: earlier → later (sibling chain).
    graph.add_edge(earlier_id, later_id, EdgeKind::Eh).unwrap();

    let typed = make_typed(graph);
    let verified = verify(typed).expect("test graph must verify");
    let nerve = build_nerve(&verified);

    // Both Dos are direct children of Describe (not Do), so there are no
    // parent-child Do overlaps. All overlaps here are sibling overlaps.
    let sib_overlap = nerve
        .overlaps
        .iter()
        .find(|o| o.node_a == earlier_id && o.node_b == later_id);
    assert!(
        sib_overlap.is_some(),
        "sibling overlap must exist for shared var `amount`; overlaps: {:?}",
        nerve.overlaps
    );
}

#[test]
fn sheaf_siblings_without_shared_var_produce_no_overlap() {
    let mut graph = empty_describe_graph();
    let root_id = graph.root_nodes().unwrap()[0];

    let a_id = add_do_child(
        &mut graph,
        root_id,
        "op a",
        "op_a",
        vec![param("amount", "NonNegativeInteger")],
        None,
        vec![before("amount >= 0"), after("amount >= 0")],
    );

    let b_id = add_do_child(
        &mut graph,
        root_id,
        "op b",
        "op_b",
        vec![param("user_id", "NonNegativeInteger")],
        None,
        vec![before("user_id >= 0"), after("user_id >= 0")],
    );

    // Wire Eh edge: a → b.
    graph.add_edge(a_id, b_id, EdgeKind::Eh).unwrap();

    let typed = make_typed(graph);
    let verified = verify(typed).expect("test graph must verify");
    let nerve = build_nerve(&verified);

    let has_sib = nerve
        .overlaps
        .iter()
        .any(|o| (o.node_a == a_id && o.node_b == b_id) || (o.node_a == b_id && o.node_b == a_id));
    assert!(
        !has_sib,
        "disjoint-var siblings must NOT produce a sibling overlap; overlaps: {:?}",
        nerve.overlaps
    );
}

#[test]
fn sheaf_output_is_deterministic_and_sorted() {
    let mut graph = empty_describe_graph();
    let root_id = graph.root_nodes().unwrap()[0];

    // Parent Do.
    let parent_id = add_do_parent(
        &mut graph,
        root_id,
        "parent op",
        "parent_op",
        vec![param("amount", "NonNegativeInteger")],
        vec![before("amount >= 0"), after("amount >= 0")],
    );

    // Two child Dos that share `amount`.
    let child1_id = add_do_child(
        &mut graph,
        parent_id,
        "child one",
        "child_one",
        vec![param("amount", "NonNegativeInteger")],
        None,
        vec![before("amount >= 0"), after("amount >= 0")],
    );
    let child2_id = add_do_child(
        &mut graph,
        parent_id,
        "child two",
        "child_two",
        vec![param("amount", "NonNegativeInteger")],
        None,
        vec![before("amount >= 0"), after("amount >= 0")],
    );
    graph.add_edge(child1_id, child2_id, EdgeKind::Eh).unwrap();

    let typed = make_typed(graph);
    let verified = verify(typed).expect("test graph must verify");

    let n1 = build_nerve(&verified);
    let n2 = build_nerve(&verified);

    assert_eq!(n1, n2, "build_nerve must be deterministic");

    // sections sorted by node_id string.
    let sections_sorted = n1
        .sections
        .windows(2)
        .all(|w| w[0].node_id.to_string() <= w[1].node_id.to_string());
    assert!(sections_sorted, "sections must be sorted by node_id string");

    // overlaps sorted by (node_a, node_b) string pair.
    let overlaps_sorted = n1.overlaps.windows(2).all(|w| {
        (w[0].node_a.to_string(), w[0].node_b.to_string())
            <= (w[1].node_a.to_string(), w[1].node_b.to_string())
    });
    assert!(
        overlaps_sorted,
        "overlaps must be sorted by (node_a, node_b) strings"
    );
}

#[test]
fn sheaf_inherited_constraints_appear_in_section() {
    let mut graph = empty_describe_graph();
    let root_id = graph.root_nodes().unwrap()[0];

    // Parent Do with an `always` contract.
    let parent_id = add_do_parent(
        &mut graph,
        root_id,
        "outer scope",
        "outer_scope",
        vec![param("x", "NonNegativeInteger")],
        vec![before("x >= 0"), always("x >= 0"), after("x >= 0")],
    );

    // Child Do that will inherit the parent's always contract.
    let child_id = add_do_child(
        &mut graph,
        parent_id,
        "inner scope",
        "inner_scope",
        vec![param("x", "NonNegativeInteger")],
        None,
        vec![before("x >= 0"), after("x >= 0")],
    );

    let typed = make_typed(graph);
    let verified = verify(typed).expect("test graph must verify");
    let nerve = build_nerve(&verified);

    let child_section = nerve
        .sections
        .iter()
        .find(|s| s.node_id == child_id)
        .expect("child section must exist");

    assert!(
        !child_section.inherited.is_empty(),
        "child section must inherit parent's always constraint"
    );
}

#[test]
fn sheaf_promoted_fact_appears_in_inherited() {
    let mut graph = empty_describe_graph();
    let root_id = graph.root_nodes().unwrap()[0];

    // Parent Do wrapping both Check and inner Do.
    let parent_id = add_do_parent(
        &mut graph,
        root_id,
        "parent scope",
        "parent_scope",
        vec![param("x", "NonNegativeInteger")],
        vec![before("x >= 0"), after("x >= 0")],
    );

    // Check node — leaf child of parent, first in sibling order.
    let check_id = {
        let mut n = Node::new(NodeId::new(), "check x positive", Pattern::Check);
        n.expression = Some(Expression("x > 0".to_string()));
        graph.add_node(n).unwrap()
    };
    graph.add_edge(parent_id, check_id, EdgeKind::Ev).unwrap();
    graph
        .get_node_mut(parent_id)
        .unwrap()
        .children
        .as_mut()
        .unwrap()
        .push(check_id);

    // Inner Do — sibling after Check.
    let inner_id = {
        let mut n = Node::new(NodeId::new(), "inner do", Pattern::Do);
        n.metadata.name = Some("inner_do".to_string());
        n.metadata.params = vec![param("x", "NonNegativeInteger")];
        n.contracts = vec![before("x >= 0"), after("x >= 0")];
        let id = graph.add_node(n).unwrap();
        graph.add_edge(parent_id, id, EdgeKind::Ev).unwrap();
        graph.add_edge(check_id, id, EdgeKind::Eh).unwrap();
        graph
            .get_node_mut(parent_id)
            .unwrap()
            .children
            .as_mut()
            .unwrap()
            .push(id);
        id
    };

    let typed = make_typed(graph);
    let verified = verify(typed).expect("test graph must verify");
    let nerve = build_nerve(&verified);

    let inner_section = nerve
        .sections
        .iter()
        .find(|s| s.node_id == inner_id)
        .expect("inner Do section must exist");

    assert!(
        !inner_section.inherited.is_empty(),
        "inner Do section must contain promoted fact from check sibling; inherited: {:?}",
        inner_section.inherited
    );
}

#[test]
fn sheaf_compound_promoted_fact_is_and_split() {
    let mut graph = empty_describe_graph();
    let root_id = graph.root_nodes().unwrap()[0];

    // Parent Do.
    let parent_id = add_do_parent(
        &mut graph,
        root_id,
        "parent scope",
        "parent_scope",
        vec![param("x", "NonNegativeInteger")],
        vec![before("x >= 0"), after("x >= 0")],
    );

    // Check node with compound expression.
    let check_id = {
        let mut n = Node::new(NodeId::new(), "check x in range", Pattern::Check);
        n.expression = Some(Expression("x > 0 and x < 100".to_string()));
        graph.add_node(n).unwrap()
    };
    graph.add_edge(parent_id, check_id, EdgeKind::Ev).unwrap();
    graph
        .get_node_mut(parent_id)
        .unwrap()
        .children
        .as_mut()
        .unwrap()
        .push(check_id);

    // Inner Do — sibling after Check.
    let inner_id = {
        let mut n = Node::new(NodeId::new(), "inner do", Pattern::Do);
        n.metadata.name = Some("inner_do".to_string());
        n.metadata.params = vec![param("x", "NonNegativeInteger")];
        n.contracts = vec![before("x >= 0"), after("x >= 0")];
        let id = graph.add_node(n).unwrap();
        graph.add_edge(parent_id, id, EdgeKind::Ev).unwrap();
        graph.add_edge(check_id, id, EdgeKind::Eh).unwrap();
        graph
            .get_node_mut(parent_id)
            .unwrap()
            .children
            .as_mut()
            .unwrap()
            .push(id);
        id
    };

    let typed = make_typed(graph);
    let verified = verify(typed).expect("test graph must verify");
    let nerve = build_nerve(&verified);

    let inner_section = nerve
        .sections
        .iter()
        .find(|s| s.node_id == inner_id)
        .expect("inner Do section must exist");

    assert!(
        inner_section.inherited.len() >= 2,
        "compound promoted fact `x > 0 and x < 100` must be AND-split into >= 2 conjuncts; \
         got {} inherited: {:?}",
        inner_section.inherited.len(),
        inner_section.inherited
    );
}

#[test]
fn sheaf_old_ref_does_not_inflate_var_set() {
    // Build a SheafSection manually to test extract_vars with old() exclusion.
    // A contract `after("old(balance) >= 0")` references `balance` only inside
    // old() so collect_top_level_refs must exclude it.
    // A contract `before("balance >= 0")` contributes `balance` normally.
    // The net var set for both together should be {"balance"} — not doubled.
    use ail_types::parse_constraint_expr;

    let before_expr = parse_constraint_expr("balance >= 0").expect("parseable");
    let after_expr = parse_constraint_expr("old(balance) >= 0").expect("parseable");

    let constraints = vec![before_expr, after_expr];
    let vars = extract_vars(&constraints);

    // `balance` from Before contributes once. The old() ref in After must NOT
    // add a second distinct entry. HashSet ensures uniqueness — size must be 1.
    assert_eq!(
        vars.len(),
        1,
        "var set must contain exactly {{balance}}; got: {:?}",
        vars
    );
    assert!(
        vars.contains("balance"),
        "var set must contain `balance`; got: {:?}",
        vars
    );
}
