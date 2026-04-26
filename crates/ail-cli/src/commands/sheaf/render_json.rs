//! JSON rendering for `ail sheaf` output.

use ail_contract::CechNerve;
use serde::Serialize;
use serde_json::Value;

/// Private envelope for JSON output.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct SheafCliOutput<'a> {
    z3_available: bool,
    scope: SheafScope<'a>,
    nerve: &'a CechNerve,
    /// `null` on default-features build; array of `ObstructionResult` on z3-verify build.
    obstructions: Value,
}

/// Scope descriptor for JSON output.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct SheafScope<'a> {
    /// "full" or "subtree"
    kind: &'a str,
    /// Present only when `kind == "subtree"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    root_node_id: Option<&'a str>,
    /// Present only when `kind == "subtree"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    root_query: Option<&'a str>,
}

pub(super) fn render_json(
    nerve: &CechNerve,
    scope_kind: &str,
    scope_root_id: Option<&str>,
    scope_root_query: Option<&str>,
    #[cfg(feature = "z3-verify")] obstructions: &[ail_contract::ObstructionResult],
) -> String {
    #[cfg(feature = "z3-verify")]
    let obstructions_value = serde_json::to_value(obstructions).unwrap_or(Value::Null);
    #[cfg(not(feature = "z3-verify"))]
    let obstructions_value = Value::Null;

    let envelope = SheafCliOutput {
        z3_available: cfg!(feature = "z3-verify"),
        scope: SheafScope {
            kind: scope_kind,
            root_node_id: scope_root_id,
            root_query: scope_root_query,
        },
        nerve,
        obstructions: obstructions_value,
    };

    serde_json::to_string_pretty(&envelope)
        .unwrap_or_else(|e| format!("{{\"error\": \"JSON serialization failed: {e}\"}}"))
}
