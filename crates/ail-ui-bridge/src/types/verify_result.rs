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
}
