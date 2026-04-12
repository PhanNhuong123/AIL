use ail_graph::types::NodeId;
use thiserror::Error;

/// Contract-level semantic errors produced by `ail-contract` static checks.
///
/// Error codes follow the `AIL-C0xx` convention. These are distinct from
/// `ValidationError` (Phase 1 structural) and `TypeError` (Phase 2 type system).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ContractError {
    /// AIL-C001: `promise before` references a variable not in the parameter input scope.
    ///
    /// Before-contracts may only reference declared parameters — the function
    /// has not started executing, so no internal bindings exist yet.
    #[error(
        "AIL-C001: before contract on {node_id} references '{illegal_ref}', not an input param"
    )]
    BeforeContractIllegalRef {
        node_id: NodeId,
        contract_expr: String,
        illegal_ref: String,
    },

    /// AIL-C002: `promise before` uses `old()` — invalid before execution.
    ///
    /// `old()` captures pre-state values, but a before-contract *is* the
    /// pre-state. There is no prior state to snapshot.
    #[error(
        "AIL-C002: before contract on {node_id} uses old(), not allowed pre-execution"
    )]
    BeforeContractUsesOld {
        node_id: NodeId,
        contract_expr: String,
    },

    /// AIL-C003: `promise after` references a variable outside `{inputs, result}`.
    ///
    /// After-contracts must describe the stable observable interface: input
    /// params, the return value (`result`), and pre-state snapshots via
    /// `old(param)`. Internal bindings (Let, Fetch, ForEach) are
    /// implementation details and must not leak into contracts.
    #[error(
        "AIL-C003: after contract on {node_id} references '{illegal_ref}', \
         not a param, result, or old(param)"
    )]
    AfterContractIllegalRef {
        node_id: NodeId,
        contract_expr: String,
        illegal_ref: String,
    },

    /// AIL-C004: `raise` references an error not declared by the enclosing function.
    ///
    /// Every raised error must correspond to an `Error` node connected to the
    /// enclosing `Do` node via an outgoing `Ed` edge.
    #[error(
        "AIL-C004: raise on {node_id} references unknown error '{error_name}'"
    )]
    RaiseUnknownError {
        node_id: NodeId,
        error_name: String,
        known_errors: Vec<String>,
    },

    /// AIL-C005: a `Do` node following a template is missing a required phase.
    ///
    /// When a `Do` node declares a template reference via an outgoing `Ed` edge,
    /// every named child of that template must appear as a named child of the
    /// implementing node.
    #[error(
        "AIL-C005: {node_id} missing template phase '{missing_phase}' \
         (required by '{template_name}')"
    )]
    FollowingMissingPhase {
        node_id: NodeId,
        template_name: String,
        missing_phase: String,
    },

    /// AIL-C006: a contract expression string failed to parse.
    ///
    /// Reported when `parse_constraint_expr` returns an error. Remaining
    /// checks on the same node continue after recording this error.
    #[error("AIL-C006: contract parse error on {node_id}: {message}")]
    ContractParseError {
        node_id: NodeId,
        contract_expr: String,
        message: String,
    },
}
