use std::collections::HashSet;

use ail_graph::cic::ScopeVariableKind;
use ail_graph::graph::AilGraph;
use ail_graph::types::{ContractKind, Node};
use ail_types::parse_constraint_expr;

use crate::errors::ContractError;

use super::scope::{check_has_old, collect_top_level_refs};

/// Check all `promise before` contracts on `node`.
///
/// Rules enforced:
/// - No `old()` usage: before-contracts describe pre-execution state, and
///   `old()` captures a *previous* pre-state that does not exist yet.
/// - All direct variable references must be declared input parameters of the
///   nearest enclosing `Do` node (resolved via the CIC context packet).
pub(crate) fn check_before_contracts(
    graph: &AilGraph,
    node: &Node,
    errors: &mut Vec<ContractError>,
) {
    let before_contracts: Vec<_> = node
        .contracts
        .iter()
        .filter(|c| c.kind == ContractKind::Before)
        .collect();

    if before_contracts.is_empty() {
        return;
    }

    // Resolve parameter-only scope once for all before-contracts on this node.
    let allowed_params = build_parameter_scope(graph, node.id);

    for contract in before_contracts {
        let expr_text = &contract.expression.0;

        let expr = match parse_constraint_expr(expr_text) {
            Ok(e) => e,
            Err(e) => {
                errors.push(ContractError::ContractParseError {
                    node_id: node.id,
                    contract_expr: expr_text.clone(),
                    message: e.to_string(),
                });
                continue;
            }
        };

        // old() is never valid in a before-contract. Report C002 and skip
        // further checks on this expression — a contract that snapshots
        // non-existent pre-state is fundamentally malformed; flagging
        // additional illegal refs on top would be noise.
        if check_has_old(&expr) {
            errors.push(ContractError::BeforeContractUsesOld {
                node_id: node.id,
                contract_expr: expr_text.clone(),
            });
            continue;
        }

        // All top-level references must be declared input parameters.
        for ref_name in collect_top_level_refs(&expr) {
            if !allowed_params.contains(&ref_name) {
                errors.push(ContractError::BeforeContractIllegalRef {
                    node_id: node.id,
                    contract_expr: expr_text.clone(),
                    illegal_ref: ref_name,
                });
            }
        }
    }
}

/// Build the set of parameter names visible to `node_id` via the CIC context.
///
/// Only `ScopeVariableKind::Parameter` variables are included: before-contracts
/// run before the function body, so no internal bindings (Let/Fetch/Loop)
/// exist yet.
fn build_parameter_scope(graph: &AilGraph, node_id: ail_graph::types::NodeId) -> HashSet<String> {
    graph
        .compute_context_packet(node_id)
        .map(|packet| {
            packet
                .scope
                .into_iter()
                .filter(|v| v.kind == ScopeVariableKind::Parameter)
                .map(|v| v.name)
                .collect()
        })
        .unwrap_or_default()
}
