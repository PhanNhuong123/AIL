use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::eval::Value;

/// A pre-defined semantic type that is always resolvable without a graph definition.
///
/// These types serve as the "always available" vocabulary for AIL programs — graph nodes
/// can declare fields or parameters of these types without a corresponding `define` node
/// in the graph. The type checker (task 2.4) resolves references to these names before
/// checking user-defined graph types.
///
/// Each variant carries its own constraint logic via [`BuiltinSemanticType::validate_value`]
/// and its constraint text representation via [`BuiltinSemanticType::constraint_exprs`]
/// (used by CIC Rule 4 — DIAGONAL to inject constraints into context packets).
///
/// # Deferred types (v0.2)
/// `Uuid` and `Timestamp` are not included in v0.1. Programs that need UUID-keyed IDs
/// (e.g. `UserID`) can define `UserID is text where length is 36` in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinSemanticType {
    /// An integer strictly greater than zero: `value > 0`.
    PositiveInteger,
    /// An integer greater than or equal to zero: `value >= 0`.
    NonNegativeInteger,
    /// A number (integer or float) strictly greater than zero: `value > 0`.
    ///
    /// # Decimal precision
    /// v0.1 only checks `> 0`. Decimal-place enforcement (e.g. monetary max 2 d.p.)
    /// is deferred to v0.2.
    PositiveAmount,
    /// A number (integer or float) in the closed interval `[0, 100]`.
    Percentage,
    /// A non-empty, non-whitespace-only text string: `len(trim(value)) > 0`.
    ///
    /// A string containing only whitespace (e.g. `"   "`) is considered empty for
    /// practical purposes. Use this type for names, labels, and descriptions.
    NonEmptyText,
    /// A text string that looks like an email address: `local@domain.tld`.
    ///
    /// # Validation level
    /// Uses a basic format check: `^[^@\s]+@[^@\s]+\.[^@\s]+$`. This is intentionally
    /// simple — not RFC 5322 compliant. Production systems should add domain-specific
    /// validation on top.
    EmailAddress,
    /// A text string that is a valid AIL identifier: `^[a-zA-Z_][a-zA-Z0-9_]*$`.
    Identifier,
}

impl BuiltinSemanticType {
    /// All built-in semantic type names in canonical order.
    pub const ALL: &'static [BuiltinSemanticType] = &[
        BuiltinSemanticType::PositiveInteger,
        BuiltinSemanticType::NonNegativeInteger,
        BuiltinSemanticType::PositiveAmount,
        BuiltinSemanticType::Percentage,
        BuiltinSemanticType::NonEmptyText,
        BuiltinSemanticType::EmailAddress,
        BuiltinSemanticType::Identifier,
    ];

    /// The canonical name of this type as it appears in AIL programs.
    pub fn name(self) -> &'static str {
        match self {
            BuiltinSemanticType::PositiveInteger => "PositiveInteger",
            BuiltinSemanticType::NonNegativeInteger => "NonNegativeInteger",
            BuiltinSemanticType::PositiveAmount => "PositiveAmount",
            BuiltinSemanticType::Percentage => "Percentage",
            BuiltinSemanticType::NonEmptyText => "NonEmptyText",
            BuiltinSemanticType::EmailAddress => "EmailAddress",
            BuiltinSemanticType::Identifier => "Identifier",
        }
    }

    /// Look up a built-in semantic type by its canonical name.
    ///
    /// Returns `None` if `name` does not match any known built-in.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "PositiveInteger" => Some(BuiltinSemanticType::PositiveInteger),
            "NonNegativeInteger" => Some(BuiltinSemanticType::NonNegativeInteger),
            "PositiveAmount" => Some(BuiltinSemanticType::PositiveAmount),
            "Percentage" => Some(BuiltinSemanticType::Percentage),
            "NonEmptyText" => Some(BuiltinSemanticType::NonEmptyText),
            "EmailAddress" => Some(BuiltinSemanticType::EmailAddress),
            "Identifier" => Some(BuiltinSemanticType::Identifier),
            _ => None,
        }
    }

    /// The AIL base type that this semantic type refines.
    ///
    /// Used by the type checker (task 2.4) to confirm that values being bound to a
    /// builtin-typed variable have the correct primitive kind before applying the
    /// semantic constraint.
    pub fn base_type(self) -> &'static str {
        match self {
            BuiltinSemanticType::PositiveInteger | BuiltinSemanticType::NonNegativeInteger => {
                "integer"
            }
            BuiltinSemanticType::PositiveAmount | BuiltinSemanticType::Percentage => "number",
            BuiltinSemanticType::NonEmptyText
            | BuiltinSemanticType::EmailAddress
            | BuiltinSemanticType::Identifier => "text",
        }
    }

    /// The constraint expressions for this type in AIL constraint syntax.
    ///
    /// CIC Rule 4 (DIAGONAL) injects these expressions into context packets so that
    /// any variable of this type automatically inherits its constraints. The evaluator
    /// uses [`validate_value`] for runtime checks; this method provides the textual
    /// form for static injection.
    ///
    /// # Note on `NonEmptyText`
    /// The returned expression `"length > 0"` is a simplified approximation. The runtime
    /// validator also trims whitespace (`!s.trim().is_empty()`), which cannot be expressed
    /// in v0.1 AIL constraint syntax. Phase 3 Z3 encoding uses `validate_value` directly.
    ///
    /// [`validate_value`]: BuiltinSemanticType::validate_value
    pub fn constraint_exprs(self) -> &'static [&'static str] {
        match self {
            BuiltinSemanticType::PositiveInteger => &["value > 0"],
            BuiltinSemanticType::NonNegativeInteger => &["value >= 0"],
            BuiltinSemanticType::PositiveAmount => &["value > 0"],
            BuiltinSemanticType::Percentage => &["value >= 0", "value <= 100"],
            BuiltinSemanticType::NonEmptyText => &["length > 0"],
            BuiltinSemanticType::EmailAddress => &[r#"value matches /^[^@\s]+@[^@\s]+\.[^@\s]+$/"#],
            BuiltinSemanticType::Identifier => &[r#"value matches /^[a-zA-Z_][a-zA-Z0-9_]*$/"#],
        }
    }

    /// Returns `true` if `value` satisfies the constraint for this semantic type.
    ///
    /// Only the expected `Value` variant(s) can satisfy a type — all others return `false`
    /// without an error, mirroring how the type checker distinguishes "wrong type" from
    /// "wrong value" (phase 2.4).
    pub fn validate_value(self, value: &Value) -> bool {
        match self {
            BuiltinSemanticType::PositiveInteger => match value {
                Value::Integer(n) => *n > 0,
                _ => false,
            },
            BuiltinSemanticType::NonNegativeInteger => match value {
                Value::Integer(n) => *n >= 0,
                _ => false,
            },
            BuiltinSemanticType::PositiveAmount => match value {
                Value::Integer(n) => *n > 0,
                Value::Float(f) => f.is_finite() && *f > 0.0,
                _ => false,
            },
            BuiltinSemanticType::Percentage => match value {
                Value::Integer(n) => (0..=100).contains(n),
                Value::Float(f) => f.is_finite() && *f >= 0.0 && *f <= 100.0,
                _ => false,
            },
            BuiltinSemanticType::NonEmptyText => match value {
                Value::Text(s) => !s.trim().is_empty(),
                _ => false,
            },
            BuiltinSemanticType::EmailAddress => match value {
                Value::Text(s) => email_regex().is_match(s),
                _ => false,
            },
            BuiltinSemanticType::Identifier => match value {
                Value::Text(s) => identifier_regex().is_match(s),
                _ => false,
            },
        }
    }
}

/// Compiled regex for `EmailAddress` validation.
/// Pattern: at least one non-whitespace/non-@ char, then `@`, then domain with a `.`.
fn email_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[^@\s]+@[^@\s]+\.[^@\s]+$").expect("email regex is valid"))
}

/// Compiled regex for `Identifier` validation.
/// Pattern: starts with letter or `_`, followed by letters, digits, or `_`.
fn identifier_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$").expect("identifier regex is valid"))
}
