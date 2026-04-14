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
    /// Error type raised when a `Check` condition fails (`otherwise raise <type>`).
    /// Populated only for `Check` nodes.
    pub otherwise_error: Option<String>,
    /// Field assignments in the `otherwise raise ... carries` clause.
    /// Each pair is (field_name, expression_value). Populated only for `Check` nodes.
    pub otherwise_assigns: Vec<(String, String)>,

    // ── ForEach structured fields ───────────────────────────────────────
    /// The collection expression iterated over. Populated only for `ForEach` nodes.
    /// Example: `"order.items"`.
    #[serde(default)]
    pub collection: Option<String>,
    /// The `do` intent clause following the collection. Populated only for `ForEach`
    /// nodes that include a `do <intent>` suffix.
    #[serde(default)]
    pub body_intent: Option<String>,

    // ── Match structured fields ─────────────────────────────────────────
    /// The discriminant expression being matched. Populated only for `Match` nodes.
    /// Example: `"user.status"`.
    #[serde(default)]
    pub discriminant: Option<String>,
    /// When-clause arms as `(value, then_expression)` pairs. Populated only for
    /// `Match` nodes.
    #[serde(default)]
    pub arms: Vec<(String, String)>,
    /// The otherwise clause expression. Populated only for `Match` nodes.
    #[serde(default)]
    pub otherwise_result: Option<String>,
}
