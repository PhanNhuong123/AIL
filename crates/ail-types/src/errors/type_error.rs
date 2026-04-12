use ail_graph::NodeId;
use thiserror::Error;

/// A type error detected during the Phase 2 type-checking pass.
///
/// `type_check` accumulates all errors in a single pass and returns them as a
/// `Vec<TypeError>` rather than stopping at the first failure. This gives
/// developers a complete picture of what needs fixing.
///
/// Error codes follow the AIL error catalog (AIL-T0xx = type errors).
#[derive(Debug, Error)]
pub enum TypeError {
    /// AIL-T001: a `type_ref` string names no known base type, builtin semantic
    /// type, or user-defined type node (`Define` / `Describe` / `Error`).
    #[error("AIL-T001: undefined type '{name}' in node {node_id}")]
    UndefinedType { node_id: NodeId, name: String },

    /// AIL-T002: a node's declared output type differs from what its context
    /// requires (`must_produce` mismatch).
    ///
    /// **Phase 2 limitation**: uses string equality, not structural subtyping.
    /// A `PositiveInteger` will NOT satisfy `NonNegativeInteger` here even
    /// though it is logically a subtype.
    ///
    /// TODO(phase-3): replace string equality with Z3-backed subtype check.
    /// See AIL-Rules §5: "Same base + source constraints imply target
    /// constraints → subtype."
    #[error("AIL-T002: type mismatch in node {node_id}: expected '{expected}', found '{actual}'")]
    TypeMismatch {
        node_id: NodeId,
        expected: String,
        actual: String,
    },

    /// AIL-T003: a field-access chain in a contract expression (e.g.
    /// `sender.nonexistent`) references a field that does not exist on the
    /// resolved `Describe` type.
    #[error("AIL-T003: field '{field}' not found on type '{type_name}' in node {node_id}")]
    UndefinedField {
        node_id: NodeId,
        type_name: String,
        field: String,
    },

    /// AIL-T004: a variable passed to a `Do` function via an outgoing Ed edge
    /// has a `type_ref` that does not match the function's declared parameter
    /// type.
    #[error(
        "AIL-T004: parameter type mismatch for '{param}' in node {node_id}: \
         expected '{expected}', found '{actual}'"
    )]
    ParamTypeMismatch {
        node_id: NodeId,
        param: String,
        expected: String,
        actual: String,
    },
}
