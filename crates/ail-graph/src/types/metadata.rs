use serde::{Deserialize, Serialize};

/// A named, typed parameter (used by `Do` nodes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    /// Type reference as raw text. `ail-types` resolves this to a concrete type.
    pub type_ref: String,
}

/// A named, typed field (used by `Describe` and `Error` nodes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    /// Type reference as raw text. `ail-types` resolves this to a concrete type.
    pub type_ref: String,
}

/// Pattern-specific symbolic metadata attached to a node.
///
/// Only the fields relevant to the node's `Pattern` are populated;
/// the rest default to `None` / empty.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NodeMetadata {
    /// Symbol name — present on `Do`, `Define`, `Describe`, and `Error` nodes.
    pub name: Option<String>,
    /// Typed parameter list — populated for `Do` nodes.
    pub params: Vec<Param>,
    /// Return type as raw text — populated for `Do` nodes.
    pub return_type: Option<String>,
    /// Base type as raw text — populated for `Define` nodes.
    pub base_type: Option<String>,
    /// Record field list — populated for `Describe` nodes.
    pub fields: Vec<Field>,
    /// Error payload field list — populated for `Error` nodes.
    pub carries: Vec<Field>,
}
