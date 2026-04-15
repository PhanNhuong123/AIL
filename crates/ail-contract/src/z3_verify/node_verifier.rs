use ail_graph::{
    types::{ContractKind, Node},
    GraphBackend,
};
use ail_types::{parse_constraint_expr, ConstraintExpr};
use z3::{SatResult, Solver};

use crate::errors::VerifyError;
use crate::z3_encode::{
    encode_constraint, type_constraints::encode_type_constraint, EncodeContext,
};

use super::context_builder::{build_encode_context, collect_param_type_constraints};

/// Verify all contracts on a single `Do` node.
///
/// **Steps:**
/// 1. Build an [`EncodeContext`] from the node's params and return type.
/// 2. Create a solver with a 30-second per-node timeout.
/// 3. Assert type constraints (PositiveInteger, etc.) and check satisfiability.
/// 4. Assert any `child_posts` (from verified child nodes) as known facts.
///    Encoding errors for child posts are silently skipped — v0.1 limitation:
///    child param names may not match the parent scope.
/// 5. Assert each `Before` contract and check satisfiability.
/// 6. For each `After` / `Always` contract: assert `NOT(post)` and check UNSAT
///    (entailment proof). Extract a counterexample when SAT.
///
/// Returns the accumulated list of [`VerifyError`] values. An empty list means
/// all checks passed for this node.
pub(super) fn verify_do_node(
    node: &Node,
    graph: &dyn GraphBackend,
    child_posts: &[ConstraintExpr],
    z3_ctx: &z3::Context,
) -> Vec<VerifyError> {
    let mut errors: Vec<VerifyError> = Vec::new();
    let node_id = node.id;

    // ── Build encoding context ────────────────────────────────────────────────
    let enc = build_encode_context(node, graph, z3_ctx);

    // ── Create solver ─────────────────────────────────────────────────────────
    // Timeout is configured on the Z3 Context via Config in verify_contracts.
    // Per-node timeout enforcement is at the context level (30 s per call).
    let solver = Solver::new(z3_ctx);

    // ── Step 3: Assert type constraints ───────────────────────────────────────
    let type_constraints = collect_param_type_constraints(node, graph);
    for (var_name, builtin) in &type_constraints {
        let path = vec![var_name.clone()];
        let Some(dyn_var) = enc.get_var(&path) else {
            continue; // variable was not registered (Uninterpreted sort)
        };
        match encode_type_constraint(*builtin, dyn_var, z3_ctx) {
            Ok(assertions) => {
                for a in &assertions {
                    solver.assert(a);
                }
            }
            Err(_) => {
                // Text-based builtins return UnsupportedConstraint — skip silently.
            }
        }
    }

    match solver.check() {
        SatResult::Unsat => {
            return vec![VerifyError::UnsatTypeConstraints { node_id }];
        }
        SatResult::Unknown => {
            // Timeout on type-constraint check alone — record and continue.
            errors.push(VerifyError::SolverTimeout {
                node_id,
                contract_expr: "<type constraints>".to_string(),
            });
            return errors;
        }
        SatResult::Sat => {}
    }

    // ── Step 4: Assert child verified postconditions as known facts ───────────
    // v0.1 limitation: child param names must match the parent scope for encoding
    // to succeed. Mismatched names produce an UnboundVariable encode error that
    // is silently skipped. Full param-substitution is deferred to v0.2.
    for child_post in child_posts {
        if let Ok(encoded) = encode_constraint(child_post, &enc) {
            solver.assert(&encoded);
        }
        // UnboundVariable or UnsupportedConstraint → silently skipped.
    }

    // ── Step 5: Assert Before contracts ───────────────────────────────────────
    for contract in node
        .contracts
        .iter()
        .filter(|c| c.kind == ContractKind::Before)
    {
        let expr_text = contract.expression.as_ref();
        match parse_and_encode(expr_text, &enc, node_id) {
            Ok(encoded) => solver.assert(&encoded),
            Err(e) => errors.push(e),
        }
    }

    match solver.check() {
        SatResult::Unsat => {
            let counterexample = "<no model — preconditions are contradictory>".to_string();
            errors.push(VerifyError::ContradictoryPreconditions {
                node_id,
                counterexample,
            });
            return errors;
        }
        SatResult::Unknown => {
            errors.push(VerifyError::SolverTimeout {
                node_id,
                contract_expr: "<precondition satisfiability check>".to_string(),
            });
            return errors;
        }
        SatResult::Sat => {}
    }

    // ── Step 6: Verify After / Always contracts ───────────────────────────────
    let post_contracts = node
        .contracts
        .iter()
        .filter(|c| c.kind == ContractKind::After || c.kind == ContractKind::Always);

    for contract in post_contracts {
        let expr_text = contract.expression.as_ref();

        let encoded_post = match parse_and_encode(expr_text, &enc, node_id) {
            Ok(b) => b,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };

        // Entailment check: assert NOT(post) on top of the current solver state.
        // UNSAT means post is a logical consequence of the preconditions — proved.
        // SAT means post can fail — extract a counterexample.
        solver.push();
        solver.assert(&encoded_post.not());

        match solver.check() {
            SatResult::Unsat => {
                // Post is entailed — proved. Nothing to do.
            }
            SatResult::Sat => {
                let counterexample = solver
                    .get_model()
                    .map(|model| format_counterexample(&model, &enc))
                    .unwrap_or_else(|| "<no model>".to_string());

                errors.push(VerifyError::PostconditionNotEntailed {
                    node_id,
                    contract_expr: expr_text.to_string(),
                    counterexample,
                });
            }
            SatResult::Unknown => {
                errors.push(VerifyError::SolverTimeout {
                    node_id,
                    contract_expr: expr_text.to_string(),
                });
            }
        }

        solver.pop(1);
    }

    errors
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Parse an AIL constraint expression string and encode it into a Z3 `Bool`.
/// Produces a [`VerifyError::EncodingFailed`] on parse or encode failure.
fn parse_and_encode<'ctx>(
    expr_text: &str,
    enc: &EncodeContext<'ctx>,
    node_id: ail_graph::types::NodeId,
) -> Result<z3::ast::Bool<'ctx>, VerifyError> {
    let constraint = parse_constraint_expr(expr_text).map_err(|e| VerifyError::EncodingFailed {
        node_id,
        inner: crate::errors::EncodeError::UnboundVariable {
            name: format!("parse error: {e}"),
        },
    })?;

    encode_constraint(&constraint, enc)
        .map_err(|e| VerifyError::EncodingFailed { node_id, inner: e })
}

/// Format all registered variables in the Z3 model as a human-readable string.
///
/// Output: `"var1 = val1, var2 = val2, …"` sorted by name.
fn format_counterexample(model: &z3::Model<'_>, enc: &EncodeContext<'_>) -> String {
    let pairs = enc.format_model(model);
    if pairs.is_empty() {
        return "<empty model>".to_string();
    }
    pairs
        .into_iter()
        .map(|(name, val)| format!("{name} = {val}"))
        .collect::<Vec<_>>()
        .join(", ")
}
