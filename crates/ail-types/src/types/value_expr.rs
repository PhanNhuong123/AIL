use serde::{Deserialize, Serialize};

use crate::types::{ArithOp, LiteralValue};

/// An expression that produces a value (not a boolean).
///
/// Used as operands in `ConstraintExpr::Compare`, arithmetic sub-expressions,
/// collection references in quantifiers, etc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueExpr {
    /// A compile-time constant: `42`, `3.14`, `"active"`, `true`, `nothing`.
    Literal(LiteralValue),

    /// A dotted-path field reference: `sender.balance` → `["sender", "balance"]`.
    Ref(Vec<String>),

    /// Pre-state snapshot: `old(sender.balance)` — valid only inside `after` contracts.
    Old(Box<ValueExpr>),

    /// A built-in function call: `len(items)`.
    ///
    /// v0.1 supports only `len`. The evaluator rejects unknown function names.
    Call { name: String, args: Vec<ValueExpr> },

    /// Binary arithmetic: `old(sender.balance) - amount`.
    Arithmetic {
        op: ArithOp,
        left: Box<ValueExpr>,
        right: Box<ValueExpr>,
    },

    /// An inline literal set used with `in`: `{0, 1, 2}`.
    Set(Vec<ValueExpr>),
}
