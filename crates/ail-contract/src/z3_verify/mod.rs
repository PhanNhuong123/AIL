//! Z3 contract verification for AIL programs (Phase 3 Task 3.3, Phase 8 Task 8.3).
//!
//! Drives the [`z3_encode`] layer from real [`TypedGraph`] node data.  For
//! every `Do` node the verifier:
//!
//! 1. Asserts type constraints implied by parameter types and checks satisfiability.
//! 2. Asserts `Before` contracts and checks that they are jointly satisfiable.
//! 3. Asserts **promoted facts** from preceding `check` nodes as axioms (v2.0).
//! 4. For each `After` / `Always` contract: proves entailment (`¬post ∧ pre` is
//!    UNSAT) and extracts a counterexample when it is not.
//! 5. Supports **compositional verification**: verified postconditions of child
//!    `Do` nodes are injected as additional known facts when verifying the parent.
//!
//! # Promoted facts (v2.0 — Task 8.3)
//!
//! After a `check X otherwise raise E` node succeeds, condition `X` becomes a
//! verified fact for all subsequent nodes. The CIC engine in `ail-graph`
//! collects these facts into [`ContextPacket::promoted_facts`]. This module
//! parses each raw [`Expression`] into a [`ConstraintExpr`], AND-splits
//! compound conditions, and feeds the results into the Z3 solver as axioms.
//!
//! [`ContextPacket::promoted_facts`]: ail_graph::ContextPacket
//! [`Expression`]: ail_graph::types::Expression
//!
//! # Entry point
//! [`verify_contracts`] — takes a [`TypedGraph`] and returns all [`VerifyError`]
//! values found. An empty `Vec` means all contracts passed Z3 verification.
//!
//! # Timeout
//! Each solver call is bounded by a 30-second per-node timeout set via
//! [`z3::Params`]. A timed-out check produces a [`VerifyError::SolverTimeout`].
//! The timeout is per-node, not global: `n` nodes may each take up to 30 s.
//!
//! # Compositional limitation (v0.1)
//! Child postconditions are asserted in the parent solver using their original
//! variable names. If a child `Do` node uses different parameter names than the
//! parent scope, the encoding will return `UnboundVariable`, which is silently
//! skipped. Full parameter substitution is deferred to v0.2.
//!
//! [`z3_encode`]: crate::z3_encode
//! [`TypedGraph`]: ail_types::TypedGraph
//! [`VerifyError`]: crate::errors::VerifyError

mod context_builder;
mod node_verifier;
mod sort;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use ail_graph::{
    compute_context_packet_for_backend,
    types::{NodeId, Pattern},
    GraphBackend,
};
use ail_types::{parse_constraint_expr, ConstraintExpr, TypedGraph};

use crate::errors::VerifyError;

use node_verifier::verify_do_node;

/// A promoted fact parsed from raw [`Expression`] text into a
/// [`ConstraintExpr`] ready for Z3 encoding, together with the source
/// `Check` node that proved it.
///
/// Compound `and` conditions are AND-split: `check A and B` produces two
/// `ParsedPromotedFact` entries sharing the same `source_node`.
pub(super) struct ParsedPromotedFact {
    pub source_node: NodeId,
    pub constraint: ConstraintExpr,
}

/// Run Z3 contract verification over every `Do` node in `typed_graph`.
///
/// Nodes are processed **bottom-up** (deepest descendants first) so that
/// verified postconditions of child `Do` nodes are available as facts when
/// verifying their ancestors (compositional verification).
///
/// Returns the accumulated list of [`VerifyError`] values. An empty `Vec`
/// means every contract passed Z3 verification.
pub fn verify_contracts(typed_graph: &TypedGraph) -> Vec<VerifyError> {
    let graph = typed_graph.graph();

    // ── Collect all Do nodes ──────────────────────────────────────────────────
    let do_nodes: Vec<_> = graph
        .all_nodes_vec()
        .into_iter()
        .filter(|n| n.pattern == Pattern::Do)
        .collect();

    if do_nodes.is_empty() {
        return Vec::new();
    }

    // ── Compute depth for each Do node ────────────────────────────────────────
    // Depth 0 = root. Deeper nodes are verified first (bottom-up).
    let mut depth_map: HashMap<NodeId, usize> = HashMap::new();
    for node in &do_nodes {
        let depth = compute_depth(node.id, graph);
        depth_map.insert(node.id, depth);
    }

    // Sort descending by depth: deepest first.
    let mut ordered = do_nodes.clone();
    ordered.sort_by(|a, b| {
        depth_map
            .get(&b.id)
            .unwrap_or(&0)
            .cmp(depth_map.get(&a.id).unwrap_or(&0))
    });

    // ── Create a single shared Z3 context ────────────────────────────────────
    // The 30-second timeout is set via Config and applies to each solver call.
    // In z3 0.12 the timeout is per-context; all solver calls within the same
    // context share the budget. For Phase 3.3 this is acceptable — v0.2 can
    // create per-node contexts if finer-grained control is needed.
    let mut cfg = z3::Config::new();
    cfg.set_param_value("timeout", "30000"); // 30 000 ms = 30 s
    let z3_ctx = z3::Context::new(&cfg);

    // ── Process nodes bottom-up ───────────────────────────────────────────────
    // `verified_posts`: maps NodeId → parsed After/Always ConstraintExpr values
    // that were successfully verified for that node. Used for compositional
    // verification of ancestor nodes.
    let mut verified_posts: HashMap<NodeId, Vec<ConstraintExpr>> = HashMap::new();
    let mut all_errors: Vec<VerifyError> = Vec::new();

    for node in &ordered {
        // Collect verified postconditions from direct Do children.
        let child_posts = collect_child_posts(node.id, graph, &verified_posts);

        // Collect promoted facts from preceding check nodes (v2.0 task 8.3).
        let promoted = collect_promoted_facts(node.id, graph);

        let errors = verify_do_node(node, graph, &child_posts, &promoted, &z3_ctx);

        // Block postcondition propagation on node-fatal errors: these
        // cause verify_do_node to return early without checking post-
        // conditions, so propagating them would be unsound.
        // EncodingFailed is non-fatal (individual contract encoding
        // failure; the verifier continues to the next contract).
        let has_node_fatal_error = errors
            .iter()
            .any(|e| !matches!(e, VerifyError::EncodingFailed { .. }));

        all_errors.extend(errors);

        if !has_node_fatal_error {
            let posts = parse_after_contracts(node);
            verified_posts.insert(node.id, posts);
        }
    }

    all_errors
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Compute the graph depth of `node_id` by walking `parent` to the root.
/// Returns 0 if the node is the root.
fn compute_depth(node_id: NodeId, graph: &dyn GraphBackend) -> usize {
    let mut depth = 0;
    let mut cursor = node_id;
    // walk upward through Ev (structural parent) edges
    while let Ok(Some(parent_id)) = graph.parent(cursor) {
        depth += 1;
        cursor = parent_id;
    }
    depth
}

/// Collect the verified postconditions of all direct `Do`-typed children of
/// `parent_id`. Non-Do children and unverified Do children are ignored.
fn collect_child_posts(
    parent_id: NodeId,
    graph: &dyn GraphBackend,
    verified_posts: &HashMap<NodeId, Vec<ConstraintExpr>>,
) -> Vec<ConstraintExpr> {
    let Ok(children) = graph.children(parent_id) else {
        return Vec::new();
    };

    let mut posts = Vec::new();
    for child_id in children {
        // AilGraph: never errors; SqliteGraph: degrade gracefully on missing node.
        let Some(child_node) = graph.get_node(child_id).ok().flatten() else {
            continue;
        };
        if child_node.pattern != Pattern::Do {
            continue;
        }
        if let Some(child_verified) = verified_posts.get(&child_id) {
            posts.extend_from_slice(child_verified);
        }
    }
    posts
}

/// Parse the `After` and `Always` contracts of `node` into `ConstraintExpr`
/// values. Contracts that fail to parse are silently skipped (parse errors were
/// already caught by Phase 3.1 static checks).
fn parse_after_contracts(node: &ail_graph::types::Node) -> Vec<ConstraintExpr> {
    node.contracts
        .iter()
        .filter(|c| {
            c.kind == ail_graph::types::ContractKind::After
                || c.kind == ail_graph::types::ContractKind::Always
        })
        .filter_map(|c| parse_constraint_expr(c.expression.as_ref()).ok())
        .collect()
}

/// Collect promoted facts for a `Do` node from the CIC context packet.
///
/// Computes the [`ContextPacket`] via the backend-agnostic CIC engine, parses
/// each `PromotedFact.condition` into a [`ConstraintExpr`], and AND-splits
/// compound expressions (e.g. `"A and B"` → two entries). Facts that fail to
/// parse are silently skipped — impure bare function calls are already
/// filtered by `ail-graph`'s `promotion.rs`.
fn collect_promoted_facts(node_id: NodeId, graph: &dyn GraphBackend) -> Vec<ParsedPromotedFact> {
    let Ok(packet) = compute_context_packet_for_backend(graph, node_id) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for pf in &packet.promoted_facts {
        let Ok(constraint) = parse_constraint_expr(pf.condition.as_ref()) else {
            continue;
        };
        // AND-split: `check A and B` → two separate axioms.
        match constraint {
            ConstraintExpr::And(children) => {
                for child in children {
                    out.push(ParsedPromotedFact {
                        source_node: pf.source_node,
                        constraint: child,
                    });
                }
            }
            other => {
                out.push(ParsedPromotedFact {
                    source_node: pf.source_node,
                    constraint: other,
                });
            }
        }
    }
    out
}
