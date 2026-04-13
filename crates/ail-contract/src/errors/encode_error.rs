use thiserror::Error;

/// Z3 encoding errors produced when converting `ConstraintExpr` AST nodes into Z3
/// solver expressions.
///
/// Error codes follow the `AIL-C0xx` convention, continuing from the static-check
/// errors defined in [`ContractError`].
///
/// [`ContractError`]: super::ContractError
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EncodeError {
    /// AIL-C007: a `ConstraintExpr` or `ValueExpr` variant that Z3 cannot represent in v0.1.
    ///
    /// Unsupported in this release:
    /// - `Matches` — regex constraints require string theories not available in Z3 v0.12
    /// - `ForAll` / `Exists` — quantifier encoding is deferred to a future task
    /// - `In` with a non-literal collection
    /// - `Literal(Nothing)` — null/optional modelling is deferred
    /// - `Literal(Text(_))` — string constants are not Z3 Int/Real/Bool values
    /// - `Mod` applied to a Real-typed expression
    /// - `Call` and `Set` value expressions
    /// - Text-based builtin types (`NonEmptyText`, `EmailAddress`, `Identifier`)
    #[error("AIL-C007: Z3 encoding does not support '{variant}'")]
    UnsupportedConstraint { variant: &'static str },

    /// AIL-C008: a `Ref` path is not registered in the encode context.
    ///
    /// Task 3.3 (verifier) must pre-register every variable path that appears
    /// in a contract — including all nesting levels — before calling
    /// `encode_constraint`. This error signals a missing registration.
    #[error("AIL-C008: variable '{name}' is not registered in the Z3 encode context")]
    UnboundVariable { name: String },

    /// AIL-C009: a value expression's Z3 sort does not match the expected sort.
    ///
    /// Example: a Bool-typed variable used in an arithmetic (`Add`, `Sub`, …)
    /// expression, or a Float literal passed to `encode_value_int`.
    #[error("AIL-C009: Z3 sort mismatch — expected {expected}, got {found}")]
    SortMismatch {
        expected: &'static str,
        found: &'static str,
    },
}
