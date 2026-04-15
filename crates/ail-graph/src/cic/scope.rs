use serde::{Deserialize, Serialize};

use crate::types::NodeId;

/// How a [`ScopeVariable`] entered the current scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeVariableKind {
    /// Declared as a `Do` node parameter.
    Parameter,
    /// Bound by a `let` leaf.
    LetBinding,
    /// Bound by the result of a `fetch` leaf.
    FetchResult,
    /// Bound as the loop variable of a `for each` node.
    LoopVariable,
}

/// A single named variable available at the current node's scope.
///
/// Scope is assembled by the CIC engine from ancestor parameters and from
/// across-chain sibling outputs (see the CIC compute module).
/// The variable's type is stored as raw text; semantic resolution happens in
/// the Phase 2 `ail-types` crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeVariable {
    /// Identifier the variable is bound to (e.g. `sender`, `new_balance`).
    pub name: String,
    /// Raw type-ref text (e.g. `User`, `WalletBalance`) — resolved later.
    pub type_ref: String,
    /// Node that introduced the binding.
    pub origin_node: NodeId,
    /// How the binding was introduced.
    pub kind: ScopeVariableKind,
}
