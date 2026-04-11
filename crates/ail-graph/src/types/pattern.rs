use serde::{Deserialize, Serialize};

/// The 17 structural and action patterns that form the closed AIL vocabulary.
/// Unknown patterns are a validation error.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pattern {
    // Type-defining patterns
    Define,
    Describe,
    Error,
    // Structural patterns
    Do,
    Promise,
    // Action patterns (appear as leaves inside Do bodies)
    Let,
    Check,
    ForEach,
    Match,
    Fetch,
    Save,
    Update,
    Remove,
    Return,
    Raise,
    Together,
    Retry,
}
