use serde::{Deserialize, Serialize};

/// A literal value that can appear in a constraint or value expression.
///
/// `Float` derives `PartialEq` but not `Eq` due to IEEE-754 NaN semantics.
/// Downstream code should treat NaN as an evaluator error, not a valid literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiteralValue {
    Integer(i64),
    Float(f64),
    Text(String),
    Bool(bool),
    /// Empty `option<T>` value — e.g. `user.email is not nothing`
    Nothing,
}
