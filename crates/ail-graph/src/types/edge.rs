use serde::{Deserialize, Serialize};

/// The three directed edge kinds in the PSSD graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Vertical: parent → child decomposition (folder depth / structural nesting).
    Ev,
    /// Horizontal: sibling → sibling sequence (file order within a folder / execution order).
    Eh,
    /// Diagonal: node ↔ node cross-reference (type, error, function, pattern, template).
    Ed,
}
