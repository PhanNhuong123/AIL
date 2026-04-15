//! Shared node/edge builders for `ail-graph` integration tests.
//!
//! Include with `mod helpers;` in each test file. All items are `pub` so
//! callers can import selectively: `use helpers::{make_node, make_child, ...}`.
#![allow(dead_code)]

use ail_graph::{
    AilGraph, Contract, ContractKind, EdgeKind, Expression, Field, Node, NodeId, Param, Pattern,
};

/// Create a standalone node (no edges yet). Sets `metadata.name` when `Some`.
pub fn make_node(
    graph: &mut AilGraph,
    pattern: Pattern,
    intent: &str,
    name: Option<&str>,
) -> NodeId {
    let mut node = Node::new(NodeId::new(), intent, pattern);
    if let Some(n) = name {
        node.metadata.name = Some(n.to_string());
    }
    graph.add_node(node).unwrap()
}

/// Create `child`, attach it under `parent` via an Ev edge, return its id.
pub fn make_child(
    graph: &mut AilGraph,
    parent: NodeId,
    pattern: Pattern,
    intent: &str,
    name: Option<&str>,
) -> NodeId {
    let id = make_node(graph, pattern, intent, name);
    graph.add_edge(parent, id, EdgeKind::Ev).unwrap();
    id
}

/// Create `next` as a child of `parent` and attach an Eh edge `prev → next`,
/// so `next` is both a structural child of `parent` and the sibling after
/// `prev` in execution order.
pub fn make_sibling_after(
    graph: &mut AilGraph,
    prev: NodeId,
    parent: NodeId,
    pattern: Pattern,
    intent: &str,
    name: Option<&str>,
) -> NodeId {
    let id = make_child(graph, parent, pattern, intent, name);
    graph.add_edge(prev, id, EdgeKind::Eh).unwrap();
    id
}

pub fn add_contract(graph: &mut AilGraph, node_id: NodeId, kind: ContractKind, expr: &str) {
    let node = graph.get_node_mut(node_id).unwrap();
    node.contracts.push(Contract {
        kind,
        expression: Expression(expr.to_string()),
    });
}

pub fn add_param(graph: &mut AilGraph, node_id: NodeId, name: &str, type_ref: &str) {
    let node = graph.get_node_mut(node_id).unwrap();
    node.metadata.params.push(Param {
        name: name.to_string(),
        type_ref: type_ref.to_string(),
    });
}

pub fn add_field(graph: &mut AilGraph, node_id: NodeId, name: &str, type_ref: &str) {
    let node = graph.get_node_mut(node_id).unwrap();
    node.metadata.fields.push(Field {
        name: name.to_string(),
        type_ref: type_ref.to_string(),
    });
}

pub fn set_return_type(graph: &mut AilGraph, node_id: NodeId, type_ref: &str) {
    let node = graph.get_node_mut(node_id).unwrap();
    node.metadata.return_type = Some(type_ref.to_string());
}

/// Set the raw expression on a node (used to attach a condition to a `Check`).
pub fn set_expression(graph: &mut AilGraph, node_id: NodeId, expr: &str) {
    let node = graph.get_node_mut(node_id).unwrap();
    node.expression = Some(Expression(expr.to_string()));
}
