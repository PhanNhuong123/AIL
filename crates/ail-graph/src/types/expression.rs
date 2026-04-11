use std::fmt;

use serde::{Deserialize, Serialize};

/// Raw expression text attached to a leaf node.
///
/// Stored as unparsed source text at the graph layer.
/// `ail-types` is responsible for parsing and type-checking this string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Expression(pub String);

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Expression {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
