use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::types::LiteralValue;

/// A runtime value produced by evaluating a `ValueExpr`.
///
/// `Record` enables field-path resolution (e.g. `sender.balance`).
/// `List` is used for collection membership (`in`) and quantifier iteration.
/// `Bytes` and `Timestamp` are supported for completeness (spec base types);
/// v0.1 supports only `Is`/`IsNot` comparisons on them — arithmetic is TODO.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Value {
    Integer(i64),
    Float(f64),
    Text(String),
    Bool(bool),
    Nothing,
    Record(HashMap<String, Value>),
    List(Vec<Value>),
    /// Raw bytes — no arithmetic in v0.1; use `is`/`is not` only.
    Bytes(Vec<u8>),
    /// Nanoseconds since Unix epoch — TODO: timestamp arithmetic in later phase.
    Timestamp(i64),
}

impl Value {
    /// Short string describing the variant kind, used in error messages.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Value::Integer(_) => "integer",
            Value::Float(_) => "float",
            Value::Text(_) => "text",
            Value::Bool(_) => "bool",
            Value::Nothing => "nothing",
            Value::Record(_) => "record",
            Value::List(_) => "list",
            Value::Bytes(_) => "bytes",
            Value::Timestamp(_) => "timestamp",
        }
    }
}

/// Manual `PartialEq` so that `Float(NaN) != Float(NaN)` (IEEE-754 semantics).
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b, // NaN != NaN
            (Value::Text(a), Value::Text(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Nothing, Value::Nothing) => true,
            (Value::Record(a), Value::Record(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Bytes(a), Value::Bytes(b)) => a == b,
            (Value::Timestamp(a), Value::Timestamp(b)) => a == b,
            _ => false,
        }
    }
}

impl From<&LiteralValue> for Value {
    fn from(lit: &LiteralValue) -> Self {
        match lit {
            LiteralValue::Integer(n) => Value::Integer(*n),
            LiteralValue::Float(f) => Value::Float(*f),
            LiteralValue::Text(s) => Value::Text(s.clone()),
            LiteralValue::Bool(b) => Value::Bool(*b),
            LiteralValue::Nothing => Value::Nothing,
        }
    }
}

/// Display for error messages — Records and Lists are truncated to ≤50 chars.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Integer(n) => write!(f, "{n}"),
            Value::Float(n) => {
                if n.fract() == 0.0 {
                    write!(f, "{n:.1}")
                } else {
                    write!(f, "{n}")
                }
            }
            Value::Text(s) => write!(f, "\"{s}\""),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Nothing => write!(f, "nothing"),
            Value::Record(map) => {
                let inner = map
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let repr = format!("{{{inner}}}");
                if repr.len() > 50 {
                    write!(f, "{{...}}")
                } else {
                    write!(f, "{repr}")
                }
            }
            Value::List(items) => {
                let inner = items
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                let repr = format!("[{inner}]");
                if repr.len() > 50 {
                    write!(f, "[...]")
                } else {
                    write!(f, "{repr}")
                }
            }
            Value::Bytes(b) => write!(f, "<{} bytes>", b.len()),
            Value::Timestamp(n) => write!(f, "<timestamp {n}>"),
        }
    }
}
