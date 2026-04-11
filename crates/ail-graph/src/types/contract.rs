use serde::{Deserialize, Serialize};

use crate::types::Expression;

/// When a contract obligation applies.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractKind {
    /// Must hold before the node executes.
    Before,
    /// Must hold after the node executes (may reference `old()` values).
    After,
    /// Must hold at all times during execution.
    Always,
}

/// A single `promise` attached to a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contract {
    pub kind: ContractKind,
    pub expression: Expression,
}
