use serde::{Deserialize, Serialize};

/// Comparison operators used in constraint expressions.
///
/// Both document-style (`is`/`is not`) and code-style (`==`/`!=`) are kept as
/// distinct variants so that the Display roundtrip is lossless. The evaluator
/// treats `Is`/`Eq` as identical and `IsNot`/`Neq` as identical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareOp {
    /// `>=`
    Gte,
    /// `<=`
    Lte,
    /// `>`
    Gt,
    /// `<`
    Lt,
    /// `==` — code-style equality
    Eq,
    /// `!=` — code-style inequality
    Neq,
    /// `is` — document-style equality
    Is,
    /// `is not` — document-style inequality
    IsNot,
}

/// Arithmetic operators used in value expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}
