use ail_graph::types::NodeId;
use thiserror::Error;

use crate::errors::EncodeError;

/// Z3 verification errors produced when checking contracts on `Do` nodes.
///
/// Error codes follow the `AIL-C0xx` convention, continuing from the encoding
/// errors defined in [`EncodeError`].
///
/// [`EncodeError`]: super::EncodeError
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VerifyError {
    /// AIL-C010: the type constraints implied by a node's parameter types are
    /// unsatisfiable — i.e. the declared parameter types are mutually contradictory
    /// and no valid input can ever be constructed.
    ///
    /// Example: a parameter declared as both `PositiveInteger` (> 0) and
    /// `NegativeInteger` (< 0) simultaneously.
    #[error(
        "AIL-C010: type constraints on {node_id} are unsatisfiable (contradictory param types)"
    )]
    UnsatTypeConstraints { node_id: NodeId },

    /// AIL-C011: the before-contracts on a node are mutually contradictory — no
    /// input satisfies all preconditions simultaneously.
    ///
    /// `counterexample` is a human-readable summary of the Z3 model witness
    /// demonstrating the contradiction (what variables the solver chose to satisfy
    /// the negation).
    #[error(
        "AIL-C011: preconditions on {node_id} are contradictory (UNSAT). \
         Counterexample: {counterexample}"
    )]
    ContradictoryPreconditions {
        node_id: NodeId,
        counterexample: String,
    },

    /// AIL-C012: a postcondition (After or Always contract) is not entailed by
    /// the preconditions — i.e. `pre ∧ ¬post` is satisfiable, so the post
    /// condition can fail at runtime.
    ///
    /// `contract_expr` is the raw expression text of the failing postcondition.
    /// `counterexample` is the Z3 model that witnesses the failure.
    #[error(
        "AIL-C012: postcondition on {node_id} is not entailed: `{contract_expr}`. \
         Counterexample: {counterexample}"
    )]
    PostconditionNotEntailed {
        node_id: NodeId,
        contract_expr: String,
        counterexample: String,
    },

    /// AIL-C013: the Z3 solver returned `Unknown` for a contract check, indicating
    /// the 30-second per-node timeout was reached before a definitive answer.
    ///
    /// `contract_expr` is the raw expression text of the timed-out check.
    #[error(
        "AIL-C013: Z3 solver timed out verifying postcondition on {node_id}: `{contract_expr}`"
    )]
    SolverTimeout {
        node_id: NodeId,
        contract_expr: String,
    },

    /// AIL-C014: the Z3 encoding of a contract expression failed.
    ///
    /// This occurs when a `ConstraintExpr` variant is not supported by the current
    /// Z3 encoder (e.g. `Matches`, `ForAll`, text literals). The inner `EncodeError`
    /// message explains the specific failure. Verification continues on other
    /// contracts after recording this error.
    #[error("AIL-C014: encoding failed for contract on {node_id}: {inner}")]
    EncodingFailed { node_id: NodeId, inner: EncodeError },

    /// AIL-C015: a promoted fact from a `check` node contradicts the established
    /// preconditions on a node. The `source_check_ids` list the originating
    /// `Check` nodes so the developer can trace back to the conflicting guard.
    ///
    /// This occurs when a preceding `check X otherwise raise E` node promotes
    /// `X` as a verified fact, but `X` is inconsistent with the node's type
    /// constraints, before-contracts, or child postconditions.
    #[error(
        "AIL-C015: promoted fact from check node(s) {source_check_ids:?} contradicts \
         preconditions on {node_id}. Counterexample: {counterexample}"
    )]
    PromotedFactContradiction {
        node_id: NodeId,
        source_check_ids: Vec<NodeId>,
        counterexample: String,
    },
}
