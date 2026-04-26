use serde::{Deserialize, Serialize};

/// Result of a full project verification pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyResultJson {
    pub ok: bool,
    pub failures: Vec<VerifyFailureJson>,
}

/// A single verification failure.
///
/// This type also doubles as `IssueJson` (see `graph_json::IssueJson`). The
/// optional `severity` and `source` fields carry issue-level metadata when used
/// in the `GraphJson.issues` list (e.g. for the TitleBar "⚠ N issues" pill and
/// the verify-lens banner). They are absent in the `VerifyResultJson.failures`
/// list where only `node_id`, `message`, and `stage` are set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyFailureJson {
    pub node_id: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    /// Issue severity: `"fail"` or `"warn"`. Present when used as `IssueJson`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    /// Issue source: `"verify"`, `"rule"`, `"type"`, or `"parse"`. Present when
    /// used as `IssueJson`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Verification outcome subtype: `"fail"` (hard counterexample, default
    /// when None), `"timeout"` (Z3 solver timeout, AIL-C013), or `"unknown"`
    /// (encoding failed, AIL-C014). Phase 16.3 schema lock: backend MVP always
    /// emits None; classification logic deferred to a future task that wires
    /// the `z3-verify` feature flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
}

/// Payload emitted on the `verify-complete` Tauri event after a `run_verifier`
/// run terminates (done, error, or cancelled). Strict superset of
/// `VerifyResultJson`: any consumer reading only `ok`/`failures` continues to
/// work without changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyCompletePayload {
    pub ok: bool,
    pub failures: Vec<VerifyFailureJson>,
    pub run_id: String,
    /// Scope of the verification: `'project'` | `'module'` | `'function'` | `'step'`
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_id: Option<String>,
    pub node_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub cancelled: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Result returned by `cancel_verifier_run`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyCancelResult {
    pub cancelled: bool,
}
