use std::collections::BTreeMap;

use crate::types::graph_json::IssueJson;
use crate::types::node_detail::{NodeDetail, VerifyOutcome};

fn outcome_to_str(o: VerifyOutcome) -> &'static str {
    match o {
        VerifyOutcome::Sat => "sat",
        VerifyOutcome::Unsat => "unsat",
        VerifyOutcome::Unknown => "unknown",
        VerifyOutcome::Timeout => "timeout",
    }
}

/// Collect project-level issues from the serialized node detail map.
///
/// Walks the `BTreeMap` in key order (deterministic). For each entry where
/// `verification.ok == false`, emits one `IssueJson` with:
/// - `node_id`: the path key
/// - `message`: the counterexample's `violates` text, or `"verification failed"`
/// - `severity`: `"fail"`
/// - `source`: `"verify"`
/// - `outcome`: the per-node `VerifyOutcome` lowercased (`"unsat"`, `"unknown"`,
///   `"timeout"`), or `None` when verification produced no Z3 verdict for the
///   node (typed-only path).
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

            let outcome = node_detail
                .verification
                .outcome
                .map(|o| outcome_to_str(o).to_string());

            issues.push(IssueJson {
                node_id: path.clone(),
                message,
                stage: None,
                severity: Some("fail".to_string()),
                source: Some("verify".to_string()),
                outcome,
            });
        }
    }

    issues
}
