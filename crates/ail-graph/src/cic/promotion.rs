//! Check promotion — promoted facts derived from `check X otherwise raise E` nodes.
//!
//! When a `Check` node is encountered in the execution sequence, its condition
//! becomes a verified fact for all subsequent nodes in the same scope. This
//! module owns the fact representation and the helper that extracts a fact from
//! a node.
//!
//! ## Design note — `Expression` over `ConstraintExpr`
//! `PromotedFact.condition` stores [`crate::types::Expression`] (raw text), not
//! `ConstraintExpr` from `ail-types`. `ail-types` already depends on `ail-graph`;
//! the reverse dependency would be cyclic. AND-splitting and all semantic
//! interpretation of the condition string are delegated to `ail-contract` (task 8.3).
//! The struct definition in doc 22 (which shows `condition: ConstraintExpr`) is
//! superseded by this decision.

use serde::{Deserialize, Serialize};

use crate::types::{Expression, Node, NodeId, Pattern};

/// Why a [`PromotedFact`] was added to a context packet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactOrigin {
    /// The fact was derived from a `check X otherwise raise E` node.
    CheckPromotion,
    /// Reserved: from a `promise after: X` postcondition (future use).
    ContractPostcondition,
    /// Reserved: from a `define X where Y` type constraint (future use).
    TypeConstraint,
}

/// A condition that has been proved true by a preceding check node and is
/// therefore available as an assumption for all subsequent nodes.
///
/// Conditions are stored as raw [`Expression`] text. Compound expressions
/// (e.g. `"a >= 0 and b > 0"`) are stored as-is; callers that need individual
/// conjuncts must split the text themselves. `PromotedFact` is fully
/// serialisable so it can persist in the CIC SQLite cache (task 7.3 / issue 8.1-D).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotedFact {
    /// The `Check` node whose condition was promoted.
    pub source_node: NodeId,
    /// Raw expression text (e.g. `"sender.balance >= amount"`). Not parsed here;
    /// `ail-contract` handles semantic interpretation and AND-splitting.
    pub condition: Expression,
    /// Why this fact exists.
    pub origin: FactOrigin,
}

/// Try to extract a [`PromotedFact`] from `node`.
///
/// Returns `None` when:
/// - The node pattern is not `Check`.
/// - The node carries no expression (the condition is missing from the AST).
/// - The expression looks like an impure bare function call (see
///   [`is_impure_function_call`]). Issue 8.1-C.
pub(super) fn extract_promoted_fact(node_id: NodeId, node: &Node) -> Option<PromotedFact> {
    if !matches!(node.pattern, Pattern::Check) {
        return None;
    }
    let expr = node.expression.as_ref()?;
    if is_impure_function_call(expr.0.as_str()) {
        return None;
    }
    Some(PromotedFact {
        source_node: node_id,
        condition: expr.clone(),
        origin: FactOrigin::CheckPromotion,
    })
}

/// Return `true` when `expr` appears to be a bare function call with no
/// comparison operator — i.e., a call whose result is opaque and therefore
/// unsafe to promote as a verified fact.
///
/// **Heuristic**: the trimmed expression must start with an identifier (possibly
/// dotted) followed by `(`, **and** must not contain any comparison or logical
/// operator keyword that would make it a verifiable constraint expression.
/// Additionally, a call-shape expression that uses `or` (disjunction) is treated
/// as impure: the runtime may have satisfied the check through either branch, so
/// neither sub-expression is individually proved — promoting the whole would
/// deliver a false assumption to the downstream solver.
///
/// Representative cases:
/// | Expression                                    | Outcome  | Reason                                    |
/// |-----------------------------------------------|----------|-------------------------------------------|
/// | `"is_valid_transfer(sender, amount)"`         | impure   | bare call, no operator                    |
/// | `"is_valid(sender) or amount > 0"`            | impure   | call-shape with `or` — disjunction unsafe |
/// | `"sender.balance >= amount"`                  | safe     | has `>=`; no call shape                   |
/// | `"not is_empty(list)"`                        | safe     | has `not` prefix — verifiable             |
/// | `"sender.status is \"active\""`               | safe     | has `is` keyword                          |
/// | `"len(items) > 0"`                            | safe     | has `>` operator; no `or`                 |
///
/// The heuristic is intentionally conservative: when in doubt it allows
/// promotion. Only unambiguous bare calls without any operator are suppressed,
/// along with call-shape disjunctions.
pub(super) fn is_impure_function_call(expr: &str) -> bool {
    let trimmed = expr.trim();

    // Must start with an identifier (letters, digits, underscores, dots) then `(`.
    let call_prefix_end = trimmed.find('(');
    let has_call_shape = call_prefix_end.is_some_and(|idx| {
        let prefix = &trimmed[..idx];
        !prefix.is_empty()
            && prefix
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && prefix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    });

    if !has_call_shape {
        return false;
    }

    // Any comparison or logical keyword means it is a verifiable constraint.
    let has_operator = trimmed.contains(">=")
        || trimmed.contains("<=")
        || trimmed.contains("!=")
        || trimmed.contains("==")
        || trimmed.contains(" > ")
        || trimmed.contains(" < ")
        || trimmed.contains(" is ")
        || trimmed.contains(" in ")
        || trimmed.contains(" matches ")
        || trimmed.starts_with("not ")
        || trimmed.contains(" not ");

    // A disjunction (`or`) in a call-shape expression is not safely promotable:
    // the runtime may have satisfied the check via the other branch, leaving
    // this sub-expression unproved. Block promotion regardless of other operators.
    let has_disjunction = trimmed.contains(" or ");

    !has_operator || has_disjunction
}
