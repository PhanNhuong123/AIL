use std::collections::HashSet;

use ail_graph::cic::ScopeVariableKind;
use ail_graph::compute_context_packet_for_backend;
use ail_graph::graph::GraphBackend;
use ail_graph::types::{ContractKind, Node, NodeId};
use ail_types::parse_constraint_expr;

use crate::errors::ContractError;

use super::scope::collect_top_level_refs;

/// Check all `promise after` contracts on `node`.
///
/// Rules enforced:
/// - Top-level variable references (outside `old()`) must be either a declared
///   input parameter or the special name `"result"`.
/// - `old(...)` references are allowed unconditionally — they snapshot the
///   pre-execution state of inputs, which is always valid post-execution.
/// - Internal bindings (Let, Fetch, ForEach loop variables) are
///   implementation details and must not appear in after-contracts.
pub(crate) fn check_after_contracts(
    graph: &dyn GraphBackend,
    node: &Node,
    errors: &mut Vec<ContractError>,
) {
    let after_contracts: Vec<_> = node
        .contracts
        .iter()
        .filter(|c| c.kind == ContractKind::After)
        .collect();

    if after_contracts.is_empty() {
        return;
    }

    let allowed = build_after_allowed_set(graph, node.id);

    for contract in after_contracts {
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

        // collect_top_level_refs excludes refs inside old() automatically.
        for ref_name in collect_top_level_refs(&expr) {
            if !allowed.contains(&ref_name) {
                errors.push(ContractError::AfterContractIllegalRef {
                    node_id: node.id,
                    contract_expr: expr_text.clone(),
                    illegal_ref: ref_name,
                });
            }
        }
    }
}

/// Build the allowed reference set for after-contracts on `node_id`.
///
/// Allowed: declared input parameters + the special name `"result"`.
/// Excluded: internal bindings (Let/Fetch/ForEach) which are implementation
/// details not part of the stable observable interface.
fn build_after_allowed_set(graph: &dyn GraphBackend, node_id: NodeId) -> HashSet<String> {
    let mut allowed: HashSet<String> = compute_context_packet_for_backend(graph, node_id)
        .map(|packet| {
            packet
                .scope
                .into_iter()
                .filter(|v| v.kind == ScopeVariableKind::Parameter)
                .map(|v| v.name)
                .collect()
        })
        .unwrap_or_default();

    allowed.insert("result".to_string());
    allowed
}
