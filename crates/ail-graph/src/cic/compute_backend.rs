//! Backend-agnostic CIC packet computation.
//!
//! [`compute_context_packet_for_backend`] mirrors
//! [`crate::graph::AilGraph::compute_context_packet`] but operates entirely
//! through the [`GraphBackend`] trait, making it usable by the SQLite backend
//! and any future implementations without access to petgraph internals.

use std::collections::HashSet;

use crate::errors::GraphError;
use crate::graph::GraphBackend;
use crate::types::{NodeId, Pattern};

use super::{ContextPacket, PacketConstraint, ScopeVariable, ScopeVariableKind};

/// Compute the [`ContextPacket`] for `node_id` using any [`GraphBackend`].
///
/// Implements the same four-rule CIC algorithm as
/// [`crate::graph::AilGraph::compute_context_packet`] but works through the
/// `GraphBackend` trait so SQLite-backed graphs can call it without petgraph.
///
/// Errors with [`GraphError::NodeNotFound`] if `node_id` is unknown.
///
/// Unresolved type references and dangling Ed targets are silently ignored —
/// those are validation / type-check concerns, not CIC concerns.
pub fn compute_context_packet_for_backend(
    backend: &dyn GraphBackend,
    node_id: NodeId,
) -> Result<ContextPacket, GraphError> {
    if backend.get_node(node_id)?.is_none() {
        return Err(GraphError::NodeNotFound(node_id));
    }

    let mut packet = ContextPacket::empty_for(node_id);
    let path = build_path(backend, node_id)?;

    // intent_chain — root-to-current, inclusive on both ends.
    for id in &path {
        let node = backend.get_node(*id)?.ok_or(GraphError::NodeNotFound(*id))?;
        packet.intent_chain.push(node.intent.clone());
    }

    // inherited_constraints (Rule 1 DOWN) — ancestors only, not the current node.
    if path.len() > 1 {
        for id in &path[..path.len() - 1] {
            let contracts = backend.contracts(*id)?;
            for c in &contracts {
                packet
                    .inherited_constraints
                    .push(PacketConstraint::from_contract(*id, c));
            }
        }
    }

    // scope: Rule 1 DOWN (params) + Rule 3 ACROSS (prev-sibling outputs).
    packet.scope = assemble_scope(backend, &path)?;

    // type_constraints (Rule 4 DIAGONAL, type branch).
    packet.type_constraints = collect_type_constraints(backend, &packet.scope);

    // call_contracts (Rule 4 DIAGONAL, call branch) — outgoing Ed edges only.
    packet.call_contracts = collect_call_contracts(backend, &path)?;

    // must_produce — return type from the nearest enclosing Do ancestor.
    packet.must_produce = nearest_return_type(backend, &path)?;

    Ok(packet)
}

// ─── private helpers ──────────────────────────────────────────────────────────

/// Build the root-to-current ancestor path (inclusive on both ends).
fn build_path(backend: &dyn GraphBackend, node_id: NodeId) -> Result<Vec<NodeId>, GraphError> {
    let mut path = vec![node_id];
    let mut cursor = node_id;
    while let Some(p) = backend.parent(cursor)? {
        path.push(p);
        cursor = p;
    }
    path.reverse();
    Ok(path)
}

/// Assemble the scope visible at the end of `path`.
///
/// For each ancestor level: add `Do` params (Rule 1 DOWN), then add `Let`,
/// `Fetch`, and `ForEach` bindings from previous siblings at that level
/// (Rule 3 ACROSS — uncle visibility).
fn assemble_scope(
    backend: &dyn GraphBackend,
    path: &[NodeId],
) -> Result<Vec<ScopeVariable>, GraphError> {
    let mut scope: Vec<ScopeVariable> = Vec::new();

    for level_id in path {
        let level = backend
            .get_node(*level_id)?
            .ok_or(GraphError::NodeNotFound(*level_id))?;

        // Rule 1 DOWN — params of this ancestor if it is a Do.
        if matches!(level.pattern, Pattern::Do) {
            for param in &level.metadata.params {
                scope.push(ScopeVariable {
                    name: param.name.clone(),
                    type_ref: param.type_ref.clone(),
                    origin_node: *level_id,
                    kind: ScopeVariableKind::Parameter,
                });
            }
        }

        // Rule 3 ACROSS — prev siblings of this ancestor, earliest first.
        let prev_chain = backend.siblings_before(*level_id)?;
        for sib_id in prev_chain {
            let sib = backend
                .get_node(sib_id)?
                .ok_or(GraphError::NodeNotFound(sib_id))?;
            let kind = match sib.pattern {
                Pattern::Let => ScopeVariableKind::LetBinding,
                Pattern::Fetch => ScopeVariableKind::FetchResult,
                Pattern::ForEach => ScopeVariableKind::LoopVariable,
                _ => continue,
            };
            let Some(name) = sib.metadata.name.as_ref() else {
                continue;
            };
            let type_ref = sib.metadata.return_type.clone().unwrap_or_default();
            scope.push(ScopeVariable {
                name: name.clone(),
                type_ref,
                origin_node: sib_id,
                kind,
            });
        }
    }

    Ok(scope)
}

/// Resolve each scope variable's `type_ref` to a type node and collect its
/// constraints, recursively through record fields.
///
/// The shared `visited` set deduplicates types reached by multiple scope
/// variables or transitively through field chains.
fn collect_type_constraints(
    backend: &dyn GraphBackend,
    scope: &[ScopeVariable],
) -> Vec<PacketConstraint> {
    let mut out: Vec<PacketConstraint> = Vec::new();
    let mut visited: HashSet<NodeId> = HashSet::new();

    for var in scope {
        let Some(type_id) = find_type_by_name(backend, &var.type_ref) else {
            continue;
        };
        let unfolded = unfold_type_constraints(backend, type_id, &mut visited);
        for c in unfolded {
            if !out
                .iter()
                .any(|e| e.origin_node == c.origin_node && e.expression == c.expression)
            {
                out.push(c);
            }
        }
    }

    out
}

/// Search for a type-defining node whose `metadata.name` equals `name`.
fn find_type_by_name(backend: &dyn GraphBackend, name: &str) -> Option<NodeId> {
    let candidates = backend.find_by_name(name).ok()?;
    for id in candidates {
        let node = backend.get_node(id).ok()??;
        if matches!(
            node.pattern,
            Pattern::Define | Pattern::Describe | Pattern::Error
        ) {
            return Some(id);
        }
    }
    None
}

/// Walk a type node and collect all constraints, recursively unfolding fields.
fn unfold_type_constraints(
    backend: &dyn GraphBackend,
    type_id: NodeId,
    visited: &mut HashSet<NodeId>,
) -> Vec<PacketConstraint> {
    if !visited.insert(type_id) {
        return Vec::new();
    }
    let Ok(Some(node)) = backend.get_node(type_id) else {
        return Vec::new();
    };

    let mut out: Vec<PacketConstraint> = node
        .contracts
        .iter()
        .map(|c| PacketConstraint::from_contract(type_id, c))
        .collect();

    if matches!(node.pattern, Pattern::Describe) {
        for field in &node.metadata.fields {
            if let Some(field_type_id) = find_type_by_name(backend, &field.type_ref) {
                let nested = unfold_type_constraints(backend, field_type_id, visited);
                out.extend(nested);
            }
        }
    }

    out
}

/// Collect call contracts from **outgoing** Ed edges at each ancestor level.
fn collect_call_contracts(
    backend: &dyn GraphBackend,
    path: &[NodeId],
) -> Result<Vec<PacketConstraint>, GraphError> {
    let mut out: Vec<PacketConstraint> = Vec::new();

    for level_id in path {
        let targets = backend.outgoing_diagonal_refs(*level_id)?;
        for target_id in targets {
            let Some(target) = backend.get_node(target_id)? else {
                continue;
            };
            if !matches!(target.pattern, Pattern::Do) {
                continue;
            }
            let contracts = backend.contracts(target_id)?;
            for c in &contracts {
                let new_c = PacketConstraint::from_contract(target_id, c);
                if !out.iter().any(|e| {
                    e.origin_node == new_c.origin_node && e.expression == new_c.expression
                }) {
                    out.push(new_c);
                }
            }
        }
    }

    Ok(out)
}

/// Return the `return_type` of the deepest `Do` ancestor that carries one.
fn nearest_return_type(
    backend: &dyn GraphBackend,
    path: &[NodeId],
) -> Result<Option<String>, GraphError> {
    for id in path.iter().rev() {
        let Some(node) = backend.get_node(*id)? else {
            continue;
        };
        if matches!(node.pattern, Pattern::Do) {
            if let Some(rt) = &node.metadata.return_type {
                return Ok(Some(rt.clone()));
            }
        }
    }
    Ok(None)
}
