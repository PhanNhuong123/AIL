/// Errors that can occur during constraint expression evaluation.
#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    /// A variable name referenced in the expression was not found in bindings.
    #[error("undefined variable: {0}")]
    UndefinedVariable(String),

    /// A field access failed because the field does not exist on the value.
    #[error("undefined field '{field}' on {value_kind}")]
    UndefinedField { field: String, value_kind: String },

    /// An operation was applied to a value of the wrong type.
    ///
    /// `value_preview` shows a truncated string representation of the offending value
    /// to aid debugging (e.g. `type mismatch: expected number, got text (value: "hello")`).
    #[error("type mismatch: expected {expected}, got {actual} (value: {value_preview})")]
    TypeMismatch {
        expected: String,
        actual: String,
        value_preview: String,
    },

    /// A function name was not recognized. v0.1 supports only `len`.
    #[error("unknown built-in function: {0}")]
    UnknownFunction(String),

    /// A built-in function was called with the wrong number of arguments.
    #[error("wrong argument count for {name}: expected {expected}, got {actual}")]
    WrongArgCount {
        name: String,
        expected: usize,
        actual: usize,
    },

    /// Integer or float division by zero.
    #[error("division by zero")]
    DivisionByZero,

    /// The regex pattern in a `matches` expression is malformed.
    #[error("invalid regex pattern '{0}': {1}")]
    InvalidRegex(String, String),

    /// `old(expr)` was evaluated outside a post-condition context (no old bindings available).
    #[error("old() used outside post-condition context")]
    OldOutsidePostCondition,
}
