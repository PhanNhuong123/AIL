use serde::{Deserialize, Serialize};

/// Result of a full project verification pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyResultJson {
    pub ok: bool,
    pub failures: Vec<VerifyFailureJson>,
}

/// A single verification failure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyFailureJson {
    pub node_id: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
}
