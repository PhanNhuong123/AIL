//! Phase 16.4 — Reviewer (coverage scoring) DTOs.
//!
//! Mirror of `ide/src/lib/types.ts` `CoverageCompletePayload` /
//! `ReviewerCancelResult`. Wire format is camelCase (serde rename_all).
//!
//! Invariants:
//! - 16.4-L: `run_id` is a String on the wire (precision-safe).
//! - 16.4-N: `cancelled: false` is omitted from JSON.
//! - 16.4-R: `node_id` is PATH-LIKE (translated via IdMap::get_path()).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageCompletePayload {
    pub run_id: String,
    pub ok: bool,
    /// "Full" | "Partial" | "Weak" | "N/A" | "Unavailable".
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    /// PATH-LIKE node id. Empty for cancel emits. Invariant 16.4-R.
    pub node_id: String,
    /// Top-3 missing concept labels.
    pub missing_concepts: Vec<String>,
    pub empty_parent: bool,
    pub degenerate_basis_fallback: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub cancelled: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewerCancelResult {
    pub cancelled: bool,
    pub run_id: String,
}
