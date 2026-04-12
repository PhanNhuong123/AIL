use serde::{Deserialize, Serialize};

use crate::types::{CompareOp, ValueExpr};

/// A boolean constraint expression used in node contracts (`promise before/after/always`).
///
/// `And` and `Or` are n-ary: the parser flattens consecutive same-operator chains
/// so that `a and b and c` produces `And([a, b, c])`, not `And(And(a, b), c)`.
/// This makes Display clean (no unnecessary parens) and lets CIC merge constraints
/// by appending to an existing `Vec` without tree surgery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintExpr {
    /// `left op right` — e.g. `sender.balance >= 0`
    Compare {
        op: CompareOp,
        left: Box<ValueExpr>,
        right: Box<ValueExpr>,
    },

    /// `value in collection` — e.g. `status in {"active", "pending"}`
    In {
        value: Box<ValueExpr>,
        collection: Box<ValueExpr>,
    },

    /// `value matches /pattern/` — regex membership test
    Matches {
        value: Box<ValueExpr>,
        pattern: String,
    },

    /// N-ary logical conjunction — `a and b and c`
    And(Vec<ConstraintExpr>),

    /// N-ary logical disjunction — `a or b or c`
    Or(Vec<ConstraintExpr>),

    /// Logical negation — `not expr`
    Not(Box<ConstraintExpr>),

    /// Universal quantifier — `for all item in order.items, item.price > 0`
    ForAll {
        variable: String,
        collection: Box<ValueExpr>,
        condition: Box<ConstraintExpr>,
    },

    /// Existential quantifier — `exists result in results where result.status is "ok"`
    Exists {
        variable: String,
        collection: Box<ValueExpr>,
        condition: Box<ConstraintExpr>,
    },
}
