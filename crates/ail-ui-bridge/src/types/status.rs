use serde::{Deserialize, Serialize};

/// Verification status of a node, function, or module.
///
/// Variant order (`Ok < Warn < Fail`) is intentional: the derived `Ord`
/// implementation makes `.max()` return the worst-child status, which is
/// exactly what rollup needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Ok,
    Warn,
    Fail,
}
