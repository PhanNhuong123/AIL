use std::collections::BTreeMap;

use crate::types::graph_json::IssueJson;
use crate::types::node_detail::NodeDetail;

/// Collect project-level issues from the serialized node detail map.
///
/// Walks the `BTreeMap` in key order (deterministic). For each entry where
/// `verification.ok == false`, emits one `IssueJson` with:
/// - `node_id`: the path key
/// - `message`: the counterexample's `violates` text, or `"verification failed"`
/// - `severity`: `"fail"`
/// - `source`: `"verify"`
///
/// Returns the issues in BTreeMap iteration order (alphabetical by node path).
/// Public for integration tests in `tests/issues.rs`.
pub fn collect_issues(detail: &BTreeMap<String, NodeDetail>) -> Vec<IssueJson> {
    let mut issues = Vec::new();

    for (path, node_detail) in detail {
        if !node_detail.verification.ok {
            let message = node_detail
                .verification
                .counterexample
                .as_ref()
                .map(|ce| ce.violates.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "verification failed".to_string());

            issues.push(IssueJson {
                node_id: path.clone(),
                message,
                stage: None,
                severity: Some("fail".to_string()),
                source: Some("verify".to_string()),
                outcome: None,
            });
        }
    }

    issues
}
