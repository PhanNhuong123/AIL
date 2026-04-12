/// AIL base type names (Rules v1.0 §P6) that are always resolvable without a graph
/// definition. Phase 2 (`ail-types`) builds semantic types on top of these.
pub const BUILTIN_TYPE_NAMES: &[&str] = &[
    // Base primitive types
    "number",
    "integer",
    "text",
    "boolean",
    "list",
    "record",
    "option",
    "bytes",
    "timestamp",
    "void", // for Do nodes that produce no value
    // Semantic types (ail-types crate, task 2.3)
    "PositiveInteger",
    "NonNegativeInteger",
    "PositiveAmount",
    "Percentage",
    "NonEmptyText",
    "EmailAddress",
    "Identifier",
];
