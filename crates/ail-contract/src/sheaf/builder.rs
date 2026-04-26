//! Čech nerve builder for Phase 17 sheaf consistency.
//!
//! ## Algorithm summary
//!
//! (a) Only `Do` nodes receive sections — structural nodes carry no local
//!     contracts that need overlap checking.
//! (b) Parent-child overlaps are **direct only**: we record the pair
//!     `(parent_Do, child_Do)` but never `(grandparent_Do, grandchild_Do)`.
//!     Transitivity is a Phase 17.2 concern.
//! (c) Sibling overlaps require at least one **shared variable name** between
//!     the two sections' combined `constraints ∪ inherited` variable sets.
//!     Fully disjoint sibling pairs are not interesting for obstruction detection.
//! (d) Both `sections` and `overlaps` are sorted before being returned so that
//!     callers see a stable, hash-independent ordering regardless of graph
//!     insertion order.
//!
//! ## AND-split asymmetry note
//!
//! Node-local contracts (`parse_node_contracts`) are kept **un-split** — a
//! compound `And` in a contract is stored as a single `ConstraintExpr::And`.
//! Inherited constraints (`collect_inherited_for_node`) are AND-split into
//! individual conjuncts so that Phase 17.2 can reason about each conjunct
//! independently.  `extract_vars` recurses into `And` nodes in both cases, so
//! variable extraction is robust whether or not the expression has been split.

use std::collections::HashMap;
use std::collections::HashSet;

use ail_graph::cic::compute_context_packet_for_backend;
use ail_graph::{GraphBackend, NodeId, Pattern};
use ail_types::{parse_constraint_expr, ConstraintExpr};

use crate::types::VerifiedGraph;

use super::types::{CechNerve, SheafOverlap, SheafSection};

/// Build the Čech nerve over a [`VerifiedGraph`].
///
/// Returns a [`CechNerve`] whose `sections` and `overlaps` satisfy the four
/// determinism and coverage invariants documented in `sheaf/mod.rs`.
pub fn build_nerve(verified: &VerifiedGraph) -> CechNerve {
    let graph = verified.graph();

    // 1. Build sections — only for Do nodes.
    let mut sections: Vec<SheafSection> = graph
        .all_nodes_vec()
        .into_iter()
        .filter(|node| node.pattern == Pattern::Do)
        .map(|node| build_section_from_node(node, graph))
        .collect();

    // 2. Sort sections first — deterministic order for the overlap pass.
    sections.sort_by(|a, b| a.node_id.to_string().cmp(&b.node_id.to_string()));

    // 3. Build a NodeId -> &SheafSection lookup.
    let section_by_id: HashMap<NodeId, &SheafSection> =
        sections.iter().map(|s| (s.node_id, s)).collect();

    let mut overlaps: Vec<SheafOverlap> = Vec::new();

    // 4. Parent-child overlaps — direct only.
    for section in &sections {
        let Ok(Some(parent_id)) = graph.parent(section.node_id) else {
            continue;
        };
        let Some(parent_section) = section_by_id.get(&parent_id) else {
            continue;
        };
        overlaps.push(SheafOverlap {
            node_a: parent_id,
            node_b: section.node_id,
            combined: build_combined(parent_section, section),
        });
    }

    // 5. Sibling overlaps — variable-shared only.
    for section in &sections {
        let Ok(sibs) = graph.siblings_before(section.node_id) else {
            continue;
        };
        for sib_id in sibs {
            let Some(sib_section) = section_by_id.get(&sib_id) else {
                continue;
            };
            let mut vars_sib = extract_vars(&sib_section.constraints);
            vars_sib.extend(extract_vars(&sib_section.inherited));
            let mut vars_s = extract_vars(&section.constraints);
            vars_s.extend(extract_vars(&section.inherited));
            if vars_sib.is_disjoint(&vars_s) {
                continue;
            }
            overlaps.push(SheafOverlap {
                node_a: sib_id,
                node_b: section.node_id,
                combined: build_combined(sib_section, section),
            });
        }
    }

    // 6. Sort overlaps for collection-level determinism.
    overlaps.sort_by(|a, b| {
        (a.node_a.to_string(), a.node_b.to_string())
            .cmp(&(b.node_a.to_string(), b.node_b.to_string()))
    });

    CechNerve { sections, overlaps }
}

// ─── private helpers ──────────────────────────────────────────────────────────

fn build_section_from_node(node: ail_graph::Node, graph: &dyn GraphBackend) -> SheafSection {
    let constraints = parse_node_contracts(&node);
    let inherited = collect_inherited_for_node(node.id, graph);
    SheafSection {
        node_id: node.id,
        constraints,
        inherited,
    }
}

/// Parse each contract on `node` into a [`ConstraintExpr`], skipping on error.
///
/// All contract kinds (Before/After/Always) are included. Contracts are kept
/// **un-split** — a compound `And` at the top level stays as a single
/// `ConstraintExpr::And`. This differs from `collect_inherited_for_node` which
/// AND-splits its output. `extract_vars` recurses into `And` nodes so variable
/// extraction is correct for both representations.
fn parse_node_contracts(node: &ail_graph::Node) -> Vec<ConstraintExpr> {
    node.contracts
        .iter()
        .filter_map(|c| parse_constraint_expr(c.expression.as_ref()).ok())
        .collect()
}

fn collect_inherited_for_node(node_id: NodeId, graph: &dyn GraphBackend) -> Vec<ConstraintExpr> {
    let Ok(packet) = compute_context_packet_for_backend(graph, node_id) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for pc in &packet.inherited_constraints {
        if let Ok(expr) = parse_constraint_expr(pc.expression.as_ref()) {
            out.extend(and_split(expr));
        }
    }
    for pf in &packet.promoted_facts {
        if let Ok(expr) = parse_constraint_expr(pf.condition.as_ref()) {
            out.extend(and_split(expr));
        }
    }
    out
}

fn and_split(expr: ConstraintExpr) -> Vec<ConstraintExpr> {
    // Recursive flatten so that any nested `And(And(...))` produced by future
    // programmatic AST construction is fully expanded. The parser already
    // flattens consecutive `and` tokens (see `ail_types::expr::parser::parse_and`),
    // so this branch is a no-op for parser-produced expressions.
    match expr {
        ConstraintExpr::And(children) => children.into_iter().flat_map(and_split).collect(),
        other => vec![other],
    }
}

/// Collect the top-level variable names referenced across all `exprs`.
///
/// Excludes `old(...)` references and quantifier-bound variables, delegating
/// to `crate::checks::scope::collect_top_level_refs` which handles both cases.
/// Returns a `HashSet` so callers can use `is_disjoint` for sibling filtering.
pub(super) fn extract_vars(exprs: &[ConstraintExpr]) -> HashSet<String> {
    let mut out = HashSet::new();
    for expr in exprs {
        out.extend(crate::checks::scope::collect_top_level_refs(expr));
    }
    out
}

fn build_combined(a: &SheafSection, b: &SheafSection) -> Vec<ConstraintExpr> {
    let mut out = Vec::new();
    out.extend(a.constraints.iter().cloned());
    out.extend(a.inherited.iter().cloned());
    out.extend(b.constraints.iter().cloned());
    out.extend(b.inherited.iter().cloned());
    out
}
