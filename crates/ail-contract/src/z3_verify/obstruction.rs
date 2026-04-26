//! H1 obstruction detection over Čech nerve overlaps.
//!
//! For each `SheafOverlap` produced by `build_nerve`, run a Z3 satisfiability
//! check on the union of node A's and node B's contract constraints. When the
//! combined set is UNSAT, extract the minimal conflicting subset via tracked
//! assertions (`Solver::assert_and_track`) and attribute each conflict back to
//! the originating node.
//!
//! AIL-C016: H1 obstructions are pair-attributed diagnostics. They are NOT a
//! variant of `VerifyError` (which is single-node-attributed), and the obstruction
//! detector is NOT itself a `VerifyError`-producing pass.

use std::collections::HashMap;

use ail_graph::{
    types::{Node, NodeId},
    GraphBackend,
};
use ail_types::ConstraintExpr;
use serde::{Deserialize, Serialize};
use z3::{ast::Bool, SatResult, Solver};

use crate::sheaf::{CechNerve, SheafOverlap, SheafSection};
use crate::types::VerifiedGraph;
use crate::z3_encode::{encode_constraint, EncodeContext};

use super::context_builder::populate_encode_context;
use super::sort::sort_for_type_ref;

/// Result of checking one `SheafOverlap` for an H1 obstruction.
///
/// `overlap_index` is the zero-based index in `CechNerve.overlaps`, providing
/// a stable back-reference. `node_a` and `node_b` mirror `SheafOverlap.node_a`
/// / `node_b` for caller convenience.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ObstructionResult {
    pub overlap_index: usize,
    pub node_a: NodeId,
    pub node_b: NodeId,
    pub status: ObstructionStatus,
}

/// Outcome of the Z3 check for one overlap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObstructionStatus {
    /// The combined constraints are jointly satisfiable.
    Consistent,

    /// The combined constraints are UNSAT. The Z3 unsat-core has been split
    /// per side. At least one list is non-empty (the all-empty case escalates
    /// to `Unknown` because it carries no attribution); either side may be
    /// empty individually when the contradiction is one-sided. Both lists are
    /// sorted by `to_string()` for run-to-run determinism.
    Contradictory {
        conflicting_a: Vec<ConstraintExpr>,
        conflicting_b: Vec<ConstraintExpr>,
    },

    /// Z3 returned Unknown, OR the encoder rejected one or more constraints,
    /// OR sort-conflict pre-check failed, OR a graph node was missing.
    /// `reason` carries the overlap identity and the cause for diagnostics.
    /// No soundness claim is made — the caller MUST treat this as inconclusive.
    Unknown { reason: String },
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Which side of the overlap a tracked label belongs to.
#[derive(Clone, Copy)]
enum Side {
    A,
    B,
}

/// Metadata for a single tracked assertion label.
struct LabelEntry {
    side: Side,
    /// Index into the deduplicated per-side constraint list.
    ci: usize,
}

/// Check whether two nodes share a param name with conflicting Z3 sorts
/// (e.g. `amount: integer` vs `amount: number`). Returns an error message
/// if a conflict is found, otherwise `None`.
fn check_sort_conflict(node_a: &Node, node_b: &Node, graph: &dyn GraphBackend) -> Option<String> {
    for pa in &node_a.metadata.params {
        for pb in &node_b.metadata.params {
            if pa.name != pb.name {
                continue;
            }
            let sa = sort_for_type_ref(&pa.type_ref, graph);
            let sb = sort_for_type_ref(&pb.type_ref, graph);
            if sa != sb {
                return Some(format!(
                    "sort conflict on param `{}`: {:?} vs {:?}",
                    pa.name, sa, sb
                ));
            }
        }
    }
    None
}

/// Deduplicate constraints by structural `PartialEq` within one side.
/// Cross-side duplicates are intentionally kept — each side is attributed
/// independently.
fn dedup_constraints(constraints: &[ConstraintExpr]) -> Vec<ConstraintExpr> {
    let mut out: Vec<ConstraintExpr> = Vec::with_capacity(constraints.len());
    for c in constraints {
        if !out.iter().any(|existing| existing == c) {
            out.push(c.clone());
        }
    }
    out
}

/// Split unsat-core labels back into per-side constraint lists.
///
/// Returns `(conflicting_a, conflicting_b)` sorted by `to_string()`.
fn attribute_core(
    core_labels: &[Bool<'_>],
    label_map: &HashMap<String, LabelEntry>,
    side_a: &[ConstraintExpr],
    side_b: &[ConstraintExpr],
) -> (Vec<ConstraintExpr>, Vec<ConstraintExpr>) {
    let mut ca: Vec<ConstraintExpr> = Vec::new();
    let mut cb: Vec<ConstraintExpr> = Vec::new();

    for lbl_bool in core_labels {
        let key = lbl_bool.to_string();
        let Some(entry) = label_map.get(&key) else {
            continue;
        };
        match entry.side {
            Side::A => {
                if let Some(c) = side_a.get(entry.ci) {
                    if !ca.iter().any(|x| x == c) {
                        ca.push(c.clone());
                    }
                }
            }
            Side::B => {
                if let Some(c) = side_b.get(entry.ci) {
                    if !cb.iter().any(|x| x == c) {
                        cb.push(c.clone());
                    }
                }
            }
        }
    }

    ca.sort_by_key(|c| c.to_string());
    cb.sort_by_key(|c| c.to_string());
    (ca, cb)
}

/// Detect H1 obstructions in a Čech nerve via Z3 tracked assertions.
///
/// For each `SheafOverlap`, this:
/// 1. Performs a sort-conflict pre-check on top-level params (no Z3).
/// 2. Builds an `EncodeContext` populated from BOTH nodes (record-field sort
///    divergence is OUT OF SCOPE for v17.2 — only top-level param sort is
///    checked).
/// 3. Encodes per-side constraints (deduped within each side by structural
///    `ConstraintExpr` equality) into Z3 Bools tagged with side-attributing
///    labels (`lbl_a_{idx}_{ci}` / `lbl_b_{idx}_{ci}`).
/// 4. Runs `Solver::check()` and branches on `SatResult`.
/// 5. On UNSAT with full encoding, extracts `Solver::get_unsat_core()` and
///    splits by label-to-side mapping.
///
/// Returns one `ObstructionResult` per overlap, in the same index order as
/// `nerve.overlaps`. Output is deterministic for a given `(nerve, verified)`
/// pair within a fixed Z3 build (`conflicting_a` and `conflicting_b` are sorted
/// by `to_string()`).
///
/// **Never panics.** Timeouts, missing graph nodes, encode errors, and sort
/// conflicts all produce `ObstructionStatus::Unknown` with a descriptive reason.
///
/// **AIL-C016**: pair-attributed obstruction diagnostic, distinct from the
/// per-node `VerifyError` family.
pub fn detect_obstructions(nerve: &CechNerve, verified: &VerifiedGraph) -> Vec<ObstructionResult> {
    // Step 0 — early exit on empty overlaps.
    if nerve.overlaps.is_empty() {
        return Vec::new();
    }

    // Build shared Z3 context with 30 s timeout.
    let mut cfg = z3::Config::new();
    cfg.set_param_value("timeout", "30000");
    let z3_ctx = z3::Context::new(&cfg);

    // Build section lookup map for O(1) access by node id.
    let section_map: HashMap<NodeId, &SheafSection> =
        nerve.sections.iter().map(|s| (s.node_id, s)).collect();

    let graph = verified.graph();

    nerve
        .overlaps
        .iter()
        .enumerate()
        .map(|(idx, overlap)| check_overlap(idx, overlap, &section_map, graph, &z3_ctx))
        .collect()
}

/// Run the Z3 satisfiability check for one overlap. Returns a fully populated
/// `ObstructionResult`.
fn check_overlap(
    idx: usize,
    overlap: &SheafOverlap,
    section_map: &HashMap<NodeId, &SheafSection>,
    graph: &dyn GraphBackend,
    z3_ctx: &z3::Context,
) -> ObstructionResult {
    let unknown = |reason: String| ObstructionResult {
        overlap_index: idx,
        node_a: overlap.node_a,
        node_b: overlap.node_b,
        status: ObstructionStatus::Unknown { reason },
    };

    // Step 1a — look up sections and graph nodes.
    let Some(section_a) = section_map.get(&overlap.node_a) else {
        return unknown(format!(
            "overlap {idx}: section missing for node_a {}",
            overlap.node_a
        ));
    };
    let Some(section_b) = section_map.get(&overlap.node_b) else {
        return unknown(format!(
            "overlap {idx}: section missing for node_b {}",
            overlap.node_b
        ));
    };

    let node_a: Node = match graph.get_node(overlap.node_a) {
        Ok(Some(n)) => n,
        _ => {
            return unknown(format!(
                "overlap {idx}: graph node missing for node_a {}",
                overlap.node_a
            ))
        }
    };
    let node_b: Node = match graph.get_node(overlap.node_b) {
        Ok(Some(n)) => n,
        _ => {
            return unknown(format!(
                "overlap {idx}: graph node missing for node_b {}",
                overlap.node_b
            ))
        }
    };

    // Step 1b — sort-conflict pre-check.
    if let Some(conflict_msg) = check_sort_conflict(&node_a, &node_b, graph) {
        return unknown(format!("overlap {idx}: {conflict_msg}"));
    }

    // Step 1c — build merged EncodeContext from both nodes.
    let mut enc = EncodeContext::new(z3_ctx);
    populate_encode_context(&mut enc, &node_a, graph);
    populate_encode_context(&mut enc, &node_b, graph);

    // Step 1d — per-side dedup (no cross-side dedup).
    let all_a: Vec<ConstraintExpr> = section_a
        .constraints
        .iter()
        .chain(section_a.inherited.iter())
        .cloned()
        .collect();
    let all_b: Vec<ConstraintExpr> = section_b
        .constraints
        .iter()
        .chain(section_b.inherited.iter())
        .cloned()
        .collect();

    let side_a = dedup_constraints(&all_a);
    let side_b = dedup_constraints(&all_b);

    // Step 1e — fresh solver; track-and-assert with side-attributing labels.
    let solver = Solver::new(z3_ctx);
    // `label_map` maps the Z3 Bool label string back to (side, constraint index).
    let mut label_map: HashMap<String, LabelEntry> = HashMap::new();
    let mut encode_errors: Vec<String> = Vec::new();

    for (ci, constraint) in side_a.iter().enumerate() {
        let label_name = format!("lbl_a_{idx}_{ci}");
        match encode_constraint(constraint, &enc) {
            Ok(encoded) => {
                let lbl = Bool::new_const(z3_ctx, label_name.as_str());
                solver.assert_and_track(&encoded, &lbl);
                label_map.insert(lbl.to_string(), LabelEntry { side: Side::A, ci });
            }
            Err(e) => {
                encode_errors.push(format!("side_a[{ci}]: {e:?}"));
            }
        }
    }

    for (ci, constraint) in side_b.iter().enumerate() {
        let label_name = format!("lbl_b_{idx}_{ci}");
        match encode_constraint(constraint, &enc) {
            Ok(encoded) => {
                let lbl = Bool::new_const(z3_ctx, label_name.as_str());
                solver.assert_and_track(&encoded, &lbl);
                label_map.insert(lbl.to_string(), LabelEntry { side: Side::B, ci });
            }
            Err(e) => {
                encode_errors.push(format!("side_b[{ci}]: {e:?}"));
            }
        }
    }

    // Step 1f — branch on solver result.
    let status = match solver.check() {
        SatResult::Sat => {
            if encode_errors.is_empty() {
                ObstructionStatus::Consistent
            } else {
                ObstructionStatus::Unknown {
                    reason: format!(
                        "overlap {idx}: SAT result with partial encoding — {}",
                        encode_errors.join("; ")
                    ),
                }
            }
        }
        SatResult::Unsat => {
            if !encode_errors.is_empty() {
                ObstructionStatus::Unknown {
                    reason: format!(
                        "overlap {idx}: UNSAT result with partial encoding — {}",
                        encode_errors.join("; ")
                    ),
                }
            } else {
                let core = solver.get_unsat_core();
                let (conflicting_a, conflicting_b) =
                    attribute_core(&core, &label_map, &side_a, &side_b);
                // Defensive guard: an empty UNSAT core (Z3 proved UNSAT via theory
                // propagation without using any tracked label) carries no per-side
                // attribution. Reporting `Contradictory { [], [] }` would violate
                // AIL-C016's pair-attributed contract; surface as Unknown instead.
                if conflicting_a.is_empty() && conflicting_b.is_empty() {
                    ObstructionStatus::Unknown {
                        reason: format!(
                            "overlap {idx}: UNSAT core empty — no tracked labels attributable"
                        ),
                    }
                } else {
                    ObstructionStatus::Contradictory {
                        conflicting_a,
                        conflicting_b,
                    }
                }
            }
        }
        SatResult::Unknown => ObstructionStatus::Unknown {
            reason: format!("overlap {idx}: Z3 returned Unknown (timeout or resource limit)"),
        },
    };

    ObstructionResult {
        overlap_index: idx,
        node_a: overlap.node_a,
        node_b: overlap.node_b,
        status,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use ail_graph::types::EdgeKind;
    use ail_graph::{
        validate_graph, AilGraph, Contract, ContractKind, Expression, GraphBackend, Node, NodeId,
        Param, Pattern,
    };
    use ail_types::{parse_constraint_expr, type_check, TypedGraph};

    use crate::sheaf::{build_nerve, CechNerve, SheafOverlap, SheafSection};
    use crate::types::VerifiedGraph;
    use crate::verify::verify;
    use crate::z3_verify::obstruction::{
        detect_obstructions, ObstructionResult, ObstructionStatus,
    };

    // ── Fixture helpers ───────────────────────────────────────────────────────

    fn make_typed(graph: AilGraph) -> TypedGraph {
        let valid = validate_graph(graph).expect("test graph must be valid");
        type_check(valid, &[]).expect("test graph must type-check")
    }

    fn make_verified(graph: AilGraph) -> VerifiedGraph {
        let typed = make_typed(graph);
        verify(typed).expect("test graph must verify")
    }

    fn param(name: &str, type_ref: &str) -> Param {
        Param {
            name: name.to_string(),
            type_ref: type_ref.to_string(),
        }
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

    /// Build a minimal root Describe graph.
    fn empty_describe_graph() -> AilGraph {
        let mut graph = AilGraph::new();
        let mut root = Node::new(NodeId::new(), "root", Pattern::Describe);
        root.children = Some(vec![]);
        let id = graph.add_node(root).unwrap();
        graph.set_root(id).unwrap();
        graph
    }

    /// Add a structural (parent) Do node — sets `children = Some(vec![])`.
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

    /// Add a leaf Do node.
    ///
    /// `parent_id` MUST refer to a node built by `add_do_parent` (or another node
    /// whose `children` field is `Some(vec![...])`). Calling with a parent built
    /// by `add_do_child` will panic on `children.as_mut().unwrap()` because leaf
    /// nodes have `children = None`.
    fn add_do_child(
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

    /// Build a parent-child Do pair and return `(nerve, verified, parent_id, child_id)`.
    fn parent_child_do(
        parent_params: Vec<Param>,
        parent_contracts: Vec<Contract>,
        child_params: Vec<Param>,
        child_contracts: Vec<Contract>,
    ) -> (CechNerve, VerifiedGraph, NodeId, NodeId) {
        let mut graph = empty_describe_graph();
        let root_id = graph.root_nodes().unwrap()[0];
        let parent_id = add_do_parent(
            &mut graph,
            root_id,
            "parent",
            "parent_fn",
            parent_params,
            parent_contracts,
        );
        let child_id = add_do_child(
            &mut graph,
            parent_id,
            "child",
            "child_fn",
            child_params,
            child_contracts,
        );
        let verified = make_verified(graph);
        let nerve = build_nerve(&verified);
        (nerve, verified, parent_id, child_id)
    }

    /// Construct a CechNerve manually (for tests that need specific sections/overlaps
    /// without going through build_nerve).
    fn manual_nerve(sections: Vec<SheafSection>, overlaps: Vec<SheafOverlap>) -> CechNerve {
        CechNerve { sections, overlaps }
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[test]
    fn consistent_overlap_returns_consistent() {
        // Parent: before amount >= 0; child: before amount >= 0 — jointly satisfiable.
        let (nerve, verified, _pid, _cid) = parent_child_do(
            vec![param("amount", "NonNegativeInteger")],
            vec![before("amount >= 0"), after("amount >= 0")],
            vec![param("amount", "NonNegativeInteger")],
            vec![before("amount >= 0"), after("amount >= 0")],
        );
        let results = detect_obstructions(&nerve, &verified);
        assert_eq!(results.len(), nerve.overlaps.len());
        for r in &results {
            assert_eq!(
                r.status,
                ObstructionStatus::Consistent,
                "compatible constraints must be Consistent; got: {:?}",
                r.status
            );
        }
    }

    #[test]
    fn contradictory_overlap_attributes_both_sides() {
        // Parent: before amount > 10; child: before amount < 5 — UNSAT together.
        let (nerve, verified, pid, cid) = parent_child_do(
            vec![param("amount", "NonNegativeInteger")],
            vec![before("amount > 10"), after("amount >= 0")],
            vec![param("amount", "NonNegativeInteger")],
            vec![before("amount < 5"), after("amount >= 0")],
        );
        let results = detect_obstructions(&nerve, &verified);
        // There must be exactly one overlap (parent-child).
        assert!(!results.is_empty());
        let r = results
            .iter()
            .find(|r| (r.node_a == pid && r.node_b == cid) || (r.node_a == cid && r.node_b == pid))
            .expect("result for parent-child overlap must exist");
        assert!(
            matches!(r.status, ObstructionStatus::Contradictory { .. }),
            "contradictory constraints must be detected; got: {:?}",
            r.status
        );
        if let ObstructionStatus::Contradictory {
            conflicting_a,
            conflicting_b,
        } = &r.status
        {
            // 17.2-A (HIGH): the parent contributes ONLY `amount > 10` and the child
            // contributes ONLY `amount < 5`. Neither alone is contradictory, so any
            // minimal UNSAT core MUST include at least one constraint from each side.
            // This is the end-to-end proof that per-side attribution works.
            assert!(
                !conflicting_a.is_empty(),
                "node_a must contribute to the UNSAT core (had `amount > 10`); got conflicting_a={conflicting_a:?}"
            );
            assert!(
                !conflicting_b.is_empty(),
                "node_b must contribute to the UNSAT core (had `amount < 5`); got conflicting_b={conflicting_b:?}"
            );
        } else {
            unreachable!("matches! check above already guarantees Contradictory");
        }
    }

    #[test]
    fn empty_combined_is_consistent() {
        // Two Do nodes with no contracts produce empty sections → no constraints to conflict.
        // Use manual_nerve to avoid build_nerve's skip-on-parse-fail behavior.
        let mut graph = empty_describe_graph();
        let root_id = graph.root_nodes().unwrap()[0];
        // Need real node ids that exist in the verified graph.
        let pid = add_do_parent(
            &mut graph,
            root_id,
            "parent",
            "parent_fn",
            vec![param("x", "NonNegativeInteger")],
            vec![before("x >= 0"), after("x >= 0")],
        );
        let cid = add_do_child(
            &mut graph,
            pid,
            "child",
            "child_fn",
            vec![param("x", "NonNegativeInteger")],
            vec![before("x >= 0"), after("x >= 0")],
        );
        let verified = make_verified(graph);
        // Construct nerve with empty constraints on both sections.
        let nerve = manual_nerve(
            vec![
                SheafSection {
                    node_id: pid,
                    constraints: vec![],
                    inherited: vec![],
                },
                SheafSection {
                    node_id: cid,
                    constraints: vec![],
                    inherited: vec![],
                },
            ],
            vec![SheafOverlap {
                node_a: pid,
                node_b: cid,
                combined: vec![],
            }],
        );
        let results = detect_obstructions(&nerve, &verified);
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].status,
            ObstructionStatus::Consistent,
            "empty constraints must be Consistent"
        );
    }

    #[test]
    fn duplicate_inherited_constraint_dedup_preserves_minimal_core() {
        // If both sides independently state `amount >= 0`, dedup within each side
        // means that constraint is asserted once per side. The combined set is SAT.
        let c = parse_constraint_expr("amount >= 0").unwrap();
        let mut graph = empty_describe_graph();
        let root_id = graph.root_nodes().unwrap()[0];
        let pid = add_do_parent(
            &mut graph,
            root_id,
            "parent",
            "parent_fn",
            vec![param("amount", "NonNegativeInteger")],
            vec![before("amount >= 0"), after("amount >= 0")],
        );
        let cid = add_do_child(
            &mut graph,
            pid,
            "child",
            "child_fn",
            vec![param("amount", "NonNegativeInteger")],
            vec![before("amount >= 0"), after("amount >= 0")],
        );
        let verified = make_verified(graph);
        // Duplicate the same constraint on side_a to verify dedup fires.
        let nerve = manual_nerve(
            vec![
                SheafSection {
                    node_id: pid,
                    constraints: vec![c.clone(), c.clone()],
                    inherited: vec![],
                },
                SheafSection {
                    node_id: cid,
                    constraints: vec![c.clone()],
                    inherited: vec![],
                },
            ],
            vec![SheafOverlap {
                node_a: pid,
                node_b: cid,
                combined: vec![],
            }],
        );
        let results = detect_obstructions(&nerve, &verified);
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].status,
            ObstructionStatus::Consistent,
            "dedup-reduced constraints must be Consistent"
        );
    }

    #[test]
    fn sort_conflict_int_vs_number_returns_unknown() {
        // Parent: amount: integer (Z3Sort::Int)
        // Child:  amount: number  (Z3Sort::Real)
        // The pre-check fires before Z3 and returns Unknown.
        let (nerve, verified, _pid, _cid) = parent_child_do(
            vec![param("amount", "integer")],
            vec![before("amount >= 0"), after("amount >= 0")],
            vec![param("amount", "number")],
            vec![before("amount >= 0"), after("amount >= 0")],
        );
        let results = detect_obstructions(&nerve, &verified);
        assert!(!results.is_empty());
        for r in &results {
            assert!(
                matches!(r.status, ObstructionStatus::Unknown { .. }),
                "sort-conflicting params must produce Unknown; got: {:?}",
                r.status
            );
        }
    }

    #[test]
    fn partial_encoding_unbound_var_returns_unknown() {
        // Constraint references `unknown_var` which is not registered in the
        // EncodeContext (no matching param). encode_constraint returns
        // EncodeError::UnboundVariable → encode_errors non-empty → Unknown.
        let bad_constraint =
            parse_constraint_expr("unknown_var > 0").expect("expression is syntactically valid");
        let mut graph = empty_describe_graph();
        let root_id = graph.root_nodes().unwrap()[0];
        // Two empty Do nodes so get_node() succeeds.
        let pid = add_do_parent(
            &mut graph,
            root_id,
            "parent",
            "parent_fn",
            vec![],
            vec![before("1 > 0"), after("1 > 0")],
        );
        let cid = add_do_child(
            &mut graph,
            pid,
            "child",
            "child_fn",
            vec![],
            vec![before("1 > 0"), after("1 > 0")],
        );
        let verified = make_verified(graph);
        // Construct nerve with the unbound constraint on side_a.
        let nerve = manual_nerve(
            vec![
                SheafSection {
                    node_id: pid,
                    constraints: vec![bad_constraint],
                    inherited: vec![],
                },
                SheafSection {
                    node_id: cid,
                    constraints: vec![],
                    inherited: vec![],
                },
            ],
            vec![SheafOverlap {
                node_a: pid,
                node_b: cid,
                combined: vec![],
            }],
        );
        let results = detect_obstructions(&nerve, &verified);
        assert_eq!(results.len(), 1);
        assert!(
            matches!(results[0].status, ObstructionStatus::Unknown { .. }),
            "unbound variable in encoding must produce Unknown; got: {:?}",
            results[0].status
        );
    }

    #[test]
    fn one_sided_contradiction_empty_conflicting_b() {
        // Only side_a has the conflicting constraints (amount > 10 AND amount < 5).
        // side_b has a compatible constraint. The UNSAT core should attribute
        // everything to side_a, leaving conflicting_b empty.
        let mut graph = empty_describe_graph();
        let root_id = graph.root_nodes().unwrap()[0];
        let pid = add_do_parent(
            &mut graph,
            root_id,
            "parent",
            "parent_fn",
            vec![param("amount", "NonNegativeInteger")],
            vec![before("amount >= 0"), after("amount >= 0")],
        );
        let cid = add_do_child(
            &mut graph,
            pid,
            "child",
            "child_fn",
            vec![param("amount", "NonNegativeInteger")],
            vec![before("amount >= 0"), after("amount >= 0")],
        );
        let verified = make_verified(graph);

        let c_gt10 = parse_constraint_expr("amount > 10").unwrap();
        let c_lt5 = parse_constraint_expr("amount < 5").unwrap();
        let c_compat = parse_constraint_expr("amount >= 0").unwrap();

        let nerve = manual_nerve(
            vec![
                SheafSection {
                    node_id: pid,
                    constraints: vec![c_gt10, c_lt5],
                    inherited: vec![],
                },
                SheafSection {
                    node_id: cid,
                    constraints: vec![c_compat],
                    inherited: vec![],
                },
            ],
            vec![SheafOverlap {
                node_a: pid,
                node_b: cid,
                combined: vec![],
            }],
        );
        let results = detect_obstructions(&nerve, &verified);
        assert_eq!(results.len(), 1);
        if let ObstructionStatus::Contradictory {
            conflicting_a,
            conflicting_b,
        } = &results[0].status
        {
            assert!(
                !conflicting_a.is_empty(),
                "conflicting_a must be non-empty for one-sided contradiction"
            );
            assert!(
                conflicting_b.is_empty(),
                "conflicting_b should be empty for purely one-sided contradiction; got: {conflicting_b:?}"
            );
        } else {
            panic!(
                "expected Contradictory for amount>10 AND amount<5; got: {:?}",
                results[0].status
            );
        }
    }

    #[test]
    fn same_constraint_on_both_sides_attributes_to_both() {
        // Both sides assert `amount > 10 AND amount < 5` → UNSAT.
        // The unsat-core should contain labels from both sides.
        let mut graph = empty_describe_graph();
        let root_id = graph.root_nodes().unwrap()[0];
        let pid = add_do_parent(
            &mut graph,
            root_id,
            "parent",
            "parent_fn",
            vec![param("amount", "NonNegativeInteger")],
            vec![before("amount >= 0"), after("amount >= 0")],
        );
        let cid = add_do_child(
            &mut graph,
            pid,
            "child",
            "child_fn",
            vec![param("amount", "NonNegativeInteger")],
            vec![before("amount >= 0"), after("amount >= 0")],
        );
        let verified = make_verified(graph);

        let c_gt10 = parse_constraint_expr("amount > 10").unwrap();
        let c_lt5 = parse_constraint_expr("amount < 5").unwrap();

        let nerve = manual_nerve(
            vec![
                SheafSection {
                    node_id: pid,
                    constraints: vec![c_gt10.clone(), c_lt5.clone()],
                    inherited: vec![],
                },
                SheafSection {
                    node_id: cid,
                    constraints: vec![c_gt10, c_lt5],
                    inherited: vec![],
                },
            ],
            vec![SheafOverlap {
                node_a: pid,
                node_b: cid,
                combined: vec![],
            }],
        );
        let results = detect_obstructions(&nerve, &verified);
        assert_eq!(results.len(), 1);
        // The constraints on both sides are contradictory — must be Contradictory.
        assert!(
            matches!(results[0].status, ObstructionStatus::Contradictory { .. }),
            "same contradictory constraints on both sides must be Contradictory; got: {:?}",
            results[0].status
        );
    }

    #[test]
    fn lib_re_export_compiles() {
        // Verify that the public re-export path compiles at the crate root.
        use crate::{
            detect_obstructions as _da, ObstructionResult as _OR, ObstructionStatus as _OS,
        };
        let _ = (
            _da,
            std::any::TypeId::of::<_OR>(),
            std::any::TypeId::of::<_OS>(),
        );
    }

    #[test]
    fn determinism_same_nerve_same_results() {
        let (nerve, verified, _pid, _cid) = parent_child_do(
            vec![param("amount", "NonNegativeInteger")],
            vec![before("amount > 10"), after("amount >= 0")],
            vec![param("amount", "NonNegativeInteger")],
            vec![before("amount < 5"), after("amount >= 0")],
        );
        let r1 = detect_obstructions(&nerve, &verified);
        let r2 = detect_obstructions(&nerve, &verified);
        assert_eq!(r1.len(), r2.len(), "run count must match");
        for (a, b) in r1.iter().zip(r2.iter()) {
            assert_eq!(a.status, b.status, "results must be deterministic");
        }
    }

    #[test]
    fn nerve_with_no_overlaps_returns_empty() {
        let mut graph = empty_describe_graph();
        let root_id = graph.root_nodes().unwrap()[0];
        // Single Do node → no overlaps.
        let _id = add_do_child(
            &mut graph,
            root_id,
            "lone op",
            "lone_op",
            vec![param("x", "NonNegativeInteger")],
            vec![before("x >= 0"), after("x >= 0")],
        );
        let verified = make_verified(graph);
        let nerve = build_nerve(&verified);
        assert!(nerve.overlaps.is_empty());
        let results = detect_obstructions(&nerve, &verified);
        assert!(results.is_empty(), "no overlaps → empty result");
    }

    #[test]
    fn multiple_overlaps_each_uses_fresh_solver() {
        // Mix Consistent and Contradictory overlaps in one nerve and assert that
        // each overlap reaches its own correct verdict. If a single solver were
        // reused across overlaps, contradictory assertions from one overlap would
        // poison the next; we'd see all-Contradictory or all-Unknown rather than
        // a mix.
        //
        // Construction (manual_nerve to control overlap shape exactly):
        //  - Section P (parent):   amount > 100  → satisfiable in isolation.
        //  - Section A (child A):  amount > 50   → satisfiable on its own.
        //  - Section B (child B):  amount < 5    → satisfiable on its own.
        //  - Overlap P–A:  {amount > 100, amount > 50}    → SAT (Consistent).
        //  - Overlap A–B:  {amount > 50,  amount < 5 }    → UNSAT (Contradictory).
        let mut graph = empty_describe_graph();
        let root_id = graph.root_nodes().unwrap()[0];
        let pid = add_do_parent(
            &mut graph,
            root_id,
            "parent",
            "parent_fn",
            vec![param("amount", "NonNegativeInteger")],
            vec![before("amount >= 0"), after("amount >= 0")],
        );
        let cid_a = add_do_child(
            &mut graph,
            pid,
            "child_a",
            "child_a_fn",
            vec![param("amount", "NonNegativeInteger")],
            vec![before("amount >= 0"), after("amount >= 0")],
        );
        let cid_b = add_do_child(
            &mut graph,
            pid,
            "child_b",
            "child_b_fn",
            vec![param("amount", "NonNegativeInteger")],
            vec![before("amount >= 0"), after("amount >= 0")],
        );
        let verified = make_verified(graph);

        let c_gt100 = parse_constraint_expr("amount > 100").unwrap();
        let c_gt50 = parse_constraint_expr("amount > 50").unwrap();
        let c_lt5 = parse_constraint_expr("amount < 5").unwrap();

        let nerve = manual_nerve(
            vec![
                SheafSection {
                    node_id: pid,
                    constraints: vec![c_gt100],
                    inherited: vec![],
                },
                SheafSection {
                    node_id: cid_a,
                    constraints: vec![c_gt50.clone()],
                    inherited: vec![],
                },
                SheafSection {
                    node_id: cid_b,
                    constraints: vec![c_lt5],
                    inherited: vec![],
                },
            ],
            vec![
                SheafOverlap {
                    node_a: pid,
                    node_b: cid_a,
                    combined: vec![],
                },
                SheafOverlap {
                    node_a: cid_a,
                    node_b: cid_b,
                    combined: vec![],
                },
            ],
        );
        let results = detect_obstructions(&nerve, &verified);
        assert_eq!(results.len(), 2, "one result per overlap");
        // Overlap 0 (P–A) must be Consistent.
        assert_eq!(
            results[0].status,
            ObstructionStatus::Consistent,
            "P-A overlap must be Consistent (amount > 100 AND amount > 50 is SAT); got: {:?}",
            results[0].status
        );
        // Overlap 1 (A–B) must be Contradictory — proves no solver-state bleed
        // from the prior Consistent check (a poisoned solver would still hold the
        // P–A assertions and might either short-circuit Sat or crosstalk).
        assert!(
            matches!(results[1].status, ObstructionStatus::Contradictory { .. }),
            "A-B overlap must be Contradictory (amount > 50 AND amount < 5 is UNSAT); got: {:?}",
            results[1].status
        );
    }

    #[test]
    fn z3_bool_const_to_string_matches_declared_name() {
        // Lock the z3-rs Ast::to_string() contract for named Bool constants —
        // attribute_core relies on Bool::new_const(ctx, name).to_string() == name
        // to round-trip labels through the unsat core. A future Z3 upgrade that
        // changes this format would silently break per-side attribution.
        let cfg = z3::Config::new();
        let ctx = z3::Context::new(&cfg);
        for name in ["lbl_a_0_0", "lbl_b_3_17", "lbl_a_42_99"] {
            let lbl = z3::ast::Bool::new_const(&ctx, name);
            assert_eq!(
                lbl.to_string(),
                name,
                "Bool::new_const(ctx, {name}).to_string() must equal the declared name"
            );
        }
    }

    // ── Phase 17.3: Serde tests (invariant 17.3-D) ───────────────────────────

    #[test]
    fn obstruction_status_consistent_serializes_with_kind_consistent() {
        // Golden literal — locks invariant 17.3-D: uniform tagged-object schema.
        let json = serde_json::to_string(&ObstructionStatus::Consistent).unwrap();
        assert_eq!(
            json, r#"{"kind":"consistent"}"#,
            "Consistent must serialize as {{\"kind\":\"consistent\"}}"
        );
    }

    #[test]
    fn obstruction_status_contradictory_serde_roundtrip() {
        use ail_types::parse_constraint_expr;

        let ca = vec![parse_constraint_expr("amount > 10").unwrap()];
        let cb = vec![parse_constraint_expr("amount < 5").unwrap()];
        let status = ObstructionStatus::Contradictory {
            conflicting_a: ca,
            conflicting_b: cb,
        };
        let json = serde_json::to_string(&status).unwrap();
        let parsed: ObstructionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(
            status, parsed,
            "Contradictory status must survive roundtrip"
        );
    }

    #[test]
    fn obstruction_status_unknown_with_reason_roundtrip() {
        let status = ObstructionStatus::Unknown {
            reason: "Z3 returned Unknown (timeout or resource limit)".to_string(),
        };
        let json = serde_json::to_string(&status).unwrap();
        let parsed: ObstructionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, parsed, "Unknown status must survive roundtrip");
        // Verify the reason string is preserved exactly.
        if let ObstructionStatus::Unknown { reason } = &parsed {
            assert!(reason.contains("timeout"), "reason must be preserved");
        } else {
            panic!("parsed status must be Unknown");
        }
    }

    #[test]
    fn obstruction_result_serde_roundtrip_full_object() {
        use ail_types::parse_constraint_expr;

        let node_a = NodeId::new();
        let node_b = NodeId::new();
        let ca = vec![parse_constraint_expr("amount > 10").unwrap()];
        let cb = vec![parse_constraint_expr("amount < 5").unwrap()];
        let result = ObstructionResult {
            overlap_index: 3,
            node_a,
            node_b,
            status: ObstructionStatus::Contradictory {
                conflicting_a: ca,
                conflicting_b: cb,
            },
        };
        let json = serde_json::to_string_pretty(&result).unwrap();
        let parsed: ObstructionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(
            result, parsed,
            "ObstructionResult must survive full roundtrip"
        );
        assert_eq!(parsed.overlap_index, 3);
        assert_eq!(parsed.node_a, node_a);
        assert_eq!(parsed.node_b, node_b);
    }
}
