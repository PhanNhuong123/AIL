use std::collections::HashSet;

use crate::errors::GraphError;
use crate::graph::AilGraph;
use crate::types::{Node, NodeId, Param, Pattern};

use super::promotion::{extract_promoted_fact, PromotedFact};
use super::type_resolution::{find_type_node_by_name, unfold_type_constraints};
use super::{ContextPacket, PacketConstraint, ScopeVariable, ScopeVariableKind};

impl AilGraph {
    /// Compute the [`ContextPacket`] for a node.
    ///
    /// The function is **pure** (borrows `&self` only), **deterministic**
    /// (same graph + same node id always produce the same packet), and
    /// therefore safely **cacheable** by callers. Callers who need memoisation
    /// should wrap the graph in an external cache keyed by `NodeId`.
    ///
    /// Errors:
    /// - [`GraphError::NodeNotFound`] if `node_id` is unknown.
    ///
    /// Unresolved type references and dangling Ed targets are silently
    /// ignored — those are validation / type-check concerns, not CIC
    /// concerns.
    pub fn compute_context_packet(&self, node_id: NodeId) -> Result<ContextPacket, GraphError> {
        // Fail fast if the node is unknown.
        self.get_node(node_id)?;

        let mut packet = ContextPacket::empty_for(node_id);

        // Root-to-current path of node ids (inclusive on both ends).
        let path = self.build_ancestor_path(node_id)?;

        // intent_chain — root-to-current, inclusive on both ends.
        for ancestor_id in &path {
            let ancestor = self.get_node(*ancestor_id)?;
            packet.intent_chain.push(ancestor.intent.clone());
        }

        // inherited_constraints (Rule 1 DOWN) — ancestors only. Per spec:
        //   child.inherited = parent.inherited ∪ parent.own
        // so the current node's own contracts are NOT part of its inherited
        // set. Callers that need the node's own obligations read them via
        // `graph.get_node(id).contracts`.
        if path.len() > 1 {
            for ancestor_id in &path[..path.len() - 1] {
                let ancestor = self.get_node(*ancestor_id)?;
                for contract in &ancestor.contracts {
                    packet
                        .inherited_constraints
                        .push(PacketConstraint::from_contract(*ancestor_id, contract));
                }
            }
        }

        // scope: Rule 1 DOWN (params) + Rule 3 ACROSS (prev-sibling outputs)
        // walked at every ancestor level for depth-aware uncle visibility.
        packet.scope = self.assemble_scope(&path)?;

        // type_constraints (Rule 4 DIAGONAL, type branch) — resolved from
        // each scope variable's type_ref, deduplicated across variables that
        // share a type.
        packet.type_constraints = self.collect_type_constraints(&packet.scope);

        // call_contracts (Rule 4 DIAGONAL, call branch) — outgoing Ed edges
        // at the current node and every ancestor level.
        packet.call_contracts = self.collect_call_contracts(&path)?;

        // template_constraints: empty in Phase 1.
        // TODO(phase-4): handle `Following` / `Using` template references
        // once those pattern variants exist in the enum.

        // verified_facts: empty in Phase 1.
        // Rule 2 UP promotes verified postconditions only after Phase 3 Z3
        // verification runs; populated by `ail-contract`, not here.

        // promoted_facts (Phase 8): check conditions proved before this node.
        // Collected depth-aware across all ancestor levels (same walk as scope).
        packet.promoted_facts = self.collect_promoted_facts(&path)?;

        // must_produce: return type of the nearest enclosing `Do` ancestor.
        packet.must_produce = self.nearest_enclosing_return_type(&path)?;

        Ok(packet)
    }

    // ─── helpers ───────────────────────────────────────────────────────────

    /// Walk `parent_of` upward from `node_id` to the root and return the
    /// resulting path **in root-first order** (root is element 0, `node_id`
    /// is the last element).
    fn build_ancestor_path(&self, node_id: NodeId) -> Result<Vec<NodeId>, GraphError> {
        let mut path = vec![node_id];
        let mut cursor = node_id;
        while let Some(parent_id) = self.parent_of(cursor)? {
            path.push(parent_id);
            cursor = parent_id;
        }
        path.reverse();
        Ok(path)
    }

    /// Build the full scope seen by the target node.
    ///
    /// Iterates root-to-current. At each ancestor:
    /// 1. If the ancestor is a `Do`, append its params as `Parameter`
    ///    scope variables (Rule 1 DOWN contribution of that level).
    /// 2. Walk the ancestor's prev-sibling chain **earliest-first** and, for
    ///    every `Let` / `Fetch` / `ForEach` whose `metadata.name` is set,
    ///    append a scope variable of the matching kind (Rule 3 ACROSS
    ///    contribution of that level — uncle visibility).
    fn assemble_scope(&self, path: &[NodeId]) -> Result<Vec<ScopeVariable>, GraphError> {
        let mut scope: Vec<ScopeVariable> = Vec::new();

        for level_id in path {
            let level_node = self.get_node(*level_id)?;

            // Rule 1 DOWN — params of this ancestor if it is a Do.
            if matches!(level_node.pattern, Pattern::Do) {
                for param in &level_node.metadata.params {
                    scope.push(param_to_scope_variable(*level_id, param));
                }
            }

            // Rule 3 ACROSS — prev siblings of this ancestor, earliest first.
            let prev_chain = self.collect_prev_sibling_chain(*level_id)?;
            for sibling_id in prev_chain {
                let sibling = self.get_node(sibling_id)?;
                if let Some(var) = sibling_scope_variable(sibling) {
                    scope.push(var);
                }
            }
            // TODO(phase-2+): also collect from `Match` branch bindings and
            // `Check` type refinements once the language supports them.
        }

        Ok(scope)
    }

    /// Walk `prev_sibling_of` backward from `node_id` until it terminates,
    /// then return the chain in **earliest-first** order (not including
    /// `node_id` itself).
    fn collect_prev_sibling_chain(&self, node_id: NodeId) -> Result<Vec<NodeId>, GraphError> {
        let mut chain: Vec<NodeId> = Vec::new();
        let mut cursor = node_id;
        while let Some(prev_id) = self.prev_sibling_of(cursor)? {
            chain.push(prev_id);
            cursor = prev_id;
        }
        chain.reverse();
        Ok(chain)
    }

    /// Resolve each scope variable's type_ref to a type node and unfold its
    /// constraints (recursively through record fields).
    ///
    /// The `visited` set is **shared across all scope variables** so a type
    /// reached by more than one variable — or reached transitively through a
    /// field of an earlier variable's type — is walked at most once per
    /// packet. This is both the cycle guard and the cross-variable dedup.
    /// An extra outer dedup on `(origin_node, expression)` defends against
    /// two type nodes that happen to declare the same constraint text.
    fn collect_type_constraints(&self, scope: &[ScopeVariable]) -> Vec<PacketConstraint> {
        let mut out: Vec<PacketConstraint> = Vec::new();
        let mut visited: HashSet<NodeId> = HashSet::new();

        for var in scope {
            let Some(type_node_id) = find_type_node_by_name(self, &var.type_ref) else {
                continue;
            };
            let unfolded = unfold_type_constraints(self, type_node_id, &mut visited);
            for constraint in unfolded {
                if !out.iter().any(|existing| {
                    existing.origin_node == constraint.origin_node
                        && existing.expression == constraint.expression
                }) {
                    out.push(constraint);
                }
            }
        }

        out
    }

    /// Collect call contracts from outgoing Ed edges at the current node and
    /// every ancestor. Only Ed targets whose pattern is `Do` contribute.
    /// Deduplicated by `(callee, expression)`.
    fn collect_call_contracts(&self, path: &[NodeId]) -> Result<Vec<PacketConstraint>, GraphError> {
        let mut out: Vec<PacketConstraint> = Vec::new();

        for level_id in path {
            let targets = self.outgoing_diagonal_refs_of(*level_id)?;
            for target_id in targets {
                let target = self.get_node(target_id)?;
                if !matches!(target.pattern, Pattern::Do) {
                    continue;
                }
                for contract in &target.contracts {
                    let new_constraint = PacketConstraint::from_contract(target_id, contract);
                    if !out.iter().any(|existing| {
                        existing.origin_node == new_constraint.origin_node
                            && existing.expression == new_constraint.expression
                    }) {
                        out.push(new_constraint);
                    }
                }
            }
        }

        Ok(out)
    }

    /// Collect all promoted facts visible at the end of `path`.
    ///
    /// Implements the depth-aware walk described in doc 22 §8.1:
    ///
    /// - At each ancestor level (root-to-current, inclusive), collect Check
    ///   siblings that appear **before** that ancestor in execution order
    ///   (Rule P1 and the depth-aware generalisation of Rule P2).
    /// - For `Do` siblings, recurse into their body to collect inner Check
    ///   nodes (Rule P2 UP: checks inside completed Dos promote outward).
    /// - Skip `ForEach` and `Match` siblings — no promotion across loop or
    ///   branch boundaries (issue 8.1-B, Rule P4).
    ///
    /// Facts are accumulated in declaration order (earliest-first per level,
    /// root level first) to ensure deterministic ordering.
    fn collect_promoted_facts(&self, path: &[NodeId]) -> Result<Vec<PromotedFact>, GraphError> {
        let mut facts: Vec<PromotedFact> = Vec::new();

        for level_id in path {
            let prev_chain = self.collect_prev_sibling_chain(*level_id)?;
            for sib_id in prev_chain {
                let sib = self.get_node(sib_id)?;
                match sib.pattern {
                    Pattern::Check => {
                        if let Some(fact) = extract_promoted_fact(sib_id, sib) {
                            facts.push(fact);
                        }
                    }
                    Pattern::Do => {
                        // Rule P2 UP: checks inside a completed Do sibling
                        // promote outward. Recurse but stop at ForEach/Match.
                        facts.extend(self.collect_facts_from_do_body(sib_id)?);
                    }
                    // ForEach and Match: stop — no promotion across boundaries.
                    _ => {}
                }
            }
        }

        Ok(facts)
    }

    /// Recursively collect promoted facts from inside a `Do` body.
    ///
    /// Walks children in order. Recurses into nested `Do` children but stops
    /// at `ForEach` and `Match` (loop-variable and branch-scope boundaries).
    fn collect_facts_from_do_body(&self, do_id: NodeId) -> Result<Vec<PromotedFact>, GraphError> {
        let mut facts: Vec<PromotedFact> = Vec::new();
        let children = self.children_of(do_id)?;
        for child_id in children {
            let child = self.get_node(child_id)?;
            match child.pattern {
                Pattern::Check => {
                    if let Some(fact) = extract_promoted_fact(child_id, child) {
                        facts.push(fact);
                    }
                }
                Pattern::Do => {
                    facts.extend(self.collect_facts_from_do_body(child_id)?);
                }
                // ForEach and Match: stop — no promotion across loop/branch boundaries.
                _ => {}
            }
        }
        Ok(facts)
    }

    /// Return the `return_type` of the deepest `Do` ancestor (including the
    /// node itself if it is a `Do`). `None` if no enclosing `Do` carries a
    /// return type.
    fn nearest_enclosing_return_type(&self, path: &[NodeId]) -> Result<Option<String>, GraphError> {
        for ancestor_id in path.iter().rev() {
            let ancestor = self.get_node(*ancestor_id)?;
            if matches!(ancestor.pattern, Pattern::Do) {
                if let Some(return_type) = &ancestor.metadata.return_type {
                    return Ok(Some(return_type.clone()));
                }
            }
        }
        Ok(None)
    }
}

// ─── free helpers ──────────────────────────────────────────────────────────

fn param_to_scope_variable(origin_node: NodeId, param: &Param) -> ScopeVariable {
    ScopeVariable {
        name: param.name.clone(),
        type_ref: param.type_ref.clone(),
        origin_node,
        kind: ScopeVariableKind::Parameter,
    }
}

/// If `sibling` is a pattern that produces a named binding, build the
/// corresponding [`ScopeVariable`]. Returns `None` for patterns that do not
/// introduce bindings or that lack a `metadata.name`.
fn sibling_scope_variable(sibling: &Node) -> Option<ScopeVariable> {
    let kind = match sibling.pattern {
        Pattern::Let => ScopeVariableKind::LetBinding,
        Pattern::Fetch => ScopeVariableKind::FetchResult,
        Pattern::ForEach => ScopeVariableKind::LoopVariable,
        _ => return None,
    };
    let name = sibling.metadata.name.as_ref()?.clone();
    // Phase 1 has no parsed let/fetch expressions, so the type_ref for a
    // sibling-introduced binding is not derivable from metadata yet. We store
    // an empty string; downstream phases will refine this once the parser
    // populates binding type information.
    let type_ref = sibling.metadata.return_type.clone().unwrap_or_default();
    Some(ScopeVariable {
        name,
        type_ref,
        origin_node: sibling.id,
        kind,
    })
}
