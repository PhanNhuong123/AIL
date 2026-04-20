//! Coverage types used by the CIC packet and the `ail-coverage` crate.
//!
//! These types are defined here in `ail-graph` so that `ContextPacket` can
//! carry a `CoverageInfo` without creating a circular dependency between
//! `ail-graph` and `ail-coverage`.

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// ─── CoverageConfig ───────────────────────────────────────────────────────────

/// Threshold + concept configuration for coverage scoring.
///
/// Passed from the caller through `ail-coverage`'s `compute_coverage` entry
/// point to `CoverageResult::into_info` so the resulting `CoverageInfo` is
/// self-describing: it embeds the config hash used to derive the score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageConfig {
    /// Whether coverage scoring is active for this run.
    pub enabled: bool,
    /// Score threshold at or above which status is [`CoverageStatus::Full`].
    pub threshold_full: f32,
    /// Score threshold at or above which status is [`CoverageStatus::Partial`].
    pub threshold_partial: f32,
    /// Caller-supplied extra concepts appended to the built-in 40-entry list.
    pub extra_concepts: Vec<String>,
}

impl Default for CoverageConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_full: 0.9,
            threshold_partial: 0.7,
            extra_concepts: Vec::new(),
        }
    }
}

impl CoverageConfig {
    /// Deterministic 16-char lowercase hex hash over a canonical string form.
    ///
    /// The hash is order-independent for `extra_concepts` (they are sorted
    /// before hashing) so that two configs differing only in concept list order
    /// produce the same hash.
    pub fn config_hash(&self) -> String {
        let mut extras = self.extra_concepts.clone();
        extras.sort();
        let canonical = format!(
            "{}|{:.6}|{:.6}|{}",
            self.enabled,
            self.threshold_full,
            self.threshold_partial,
            extras.join(","),
        );
        let mut hasher = DefaultHasher::new();
        canonical.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

// ─── CoverageStatus ───────────────────────────────────────────────────────────

/// Qualitative coverage status derived from a numeric score.
///
/// Serializes as a human-readable label string (e.g. `"Full"`, `"N/A"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageStatus {
    /// Score ≥ `threshold_full`.
    Full,
    /// Score ≥ `threshold_partial` but < `threshold_full`.
    Partial,
    /// Score < `threshold_partial`.
    Weak,
    /// No score (leaf node with no children; Guard D).
    Leaf,
    /// Coverage could not be computed (embedding unavailable etc.).
    Unavailable,
}

impl CoverageStatus {
    /// Map a raw `score` to a status using the supplied config thresholds.
    pub fn from_score(score: Option<f32>, cfg: &CoverageConfig) -> Self {
        match score {
            None => CoverageStatus::Leaf,
            Some(s) if s >= cfg.threshold_full => CoverageStatus::Full,
            Some(s) if s >= cfg.threshold_partial => CoverageStatus::Partial,
            Some(_) => CoverageStatus::Weak,
        }
    }

    /// Human-readable label. `Leaf` maps to `"N/A"` to avoid confusing
    /// "no children" with a coverage weakness.
    pub fn label(&self) -> &'static str {
        match self {
            CoverageStatus::Full => "Full",
            CoverageStatus::Partial => "Partial",
            CoverageStatus::Weak => "Weak",
            CoverageStatus::Leaf => "N/A",
            CoverageStatus::Unavailable => "Unavailable",
        }
    }
}

impl Serialize for CoverageStatus {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.label())
    }
}

impl<'de> Deserialize<'de> for CoverageStatus {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let label = String::deserialize(d)?;
        match label.as_str() {
            "Full" => Ok(CoverageStatus::Full),
            "Partial" => Ok(CoverageStatus::Partial),
            "Weak" => Ok(CoverageStatus::Weak),
            "N/A" => Ok(CoverageStatus::Leaf),
            "Unavailable" => Ok(CoverageStatus::Unavailable),
            other => Err(serde::de::Error::custom(format!(
                "unknown CoverageStatus: {}",
                other
            ))),
        }
    }
}

// ─── Child / missing-aspect info ─────────────────────────────────────────────

/// Serializable snapshot of a single child node's contribution to parent coverage.
///
/// Mirrors `ail_coverage::ChildContribution` but uses `String` for `node_id`
/// so that `ail-graph` does not take an `ail-coverage` dependency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChildContributionInfo {
    /// String representation of the child's [`crate::types::NodeId`].
    pub node_id: String,
    /// First ≤60 characters of the child's intent text.
    pub intent_preview: String,
    /// Absolute projection magnitude onto this child's basis vector.
    pub projection_magnitude: f32,
}

/// Serializable snapshot of a software concept not covered by the children.
///
/// Mirrors `ail_coverage::MissingAspect`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissingAspectInfo {
    /// Human-readable concept label (from the built-in list or `extra_concepts`).
    pub concept: String,
    /// Cosine similarity between the residual direction and this concept's embedding.
    pub similarity: f32,
}

// ─── CoverageInfo ─────────────────────────────────────────────────────────────

/// Computed and timestamped coverage data attached to a [`super::ContextPacket`].
///
/// Produced by `CoverageResult::into_info` in `ail-coverage` and persisted via
/// `ail-db` in later integration phases.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageInfo {
    /// Numeric score, or `None` for leaf nodes (Guard D).
    pub score: Option<f32>,
    /// Qualitative status derived from `score` and the thresholds in `config_hash`.
    pub status: CoverageStatus,
    /// Per-child projection magnitudes in original child order.
    pub child_contributions: Vec<ChildContributionInfo>,
    /// Aspects found in the parent residual but absent from children.
    pub missing_aspects: Vec<MissingAspectInfo>,
    /// `true` when the parent embedding was near-zero (Guard A).
    pub empty_parent: bool,
    /// `true` when the Gram-Schmidt basis degenerated and average-cosine
    /// fallback was used (Guard C).
    pub degenerate_basis_fallback: bool,
    /// Unix epoch seconds at which this result was computed.
    pub computed_at: i64,
    /// Hex hash of the `CoverageConfig` used to compute this result.
    pub config_hash: String,
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── CoverageConfig ────────────────────────────────────────────────────────

    #[test]
    fn coverage_config_default_values() {
        let cfg = CoverageConfig::default();
        assert!(cfg.enabled);
        assert!((cfg.threshold_full - 0.9).abs() < f32::EPSILON);
        assert!((cfg.threshold_partial - 0.7).abs() < f32::EPSILON);
        assert!(cfg.extra_concepts.is_empty());
    }

    #[test]
    fn coverage_config_hash_is_stable() {
        let cfg = CoverageConfig::default();
        let h1 = cfg.config_hash();
        let h2 = cfg.config_hash();
        assert_eq!(h1, h2, "same config must produce same hash");
        assert_eq!(h1.len(), 16, "hash must be 16 hex chars");
    }

    #[test]
    fn coverage_config_hash_changes_on_threshold_change() {
        let cfg1 = CoverageConfig::default();
        let cfg2 = CoverageConfig {
            threshold_full: 0.85,
            ..CoverageConfig::default()
        };
        assert_ne!(
            cfg1.config_hash(),
            cfg2.config_hash(),
            "changed threshold_full must change the hash"
        );
    }

    #[test]
    fn coverage_config_hash_order_independent_for_extra_concepts() {
        let cfg1 = CoverageConfig {
            extra_concepts: vec!["alpha".to_owned(), "beta".to_owned()],
            ..CoverageConfig::default()
        };
        let cfg2 = CoverageConfig {
            extra_concepts: vec!["beta".to_owned(), "alpha".to_owned()],
            ..CoverageConfig::default()
        };
        assert_eq!(
            cfg1.config_hash(),
            cfg2.config_hash(),
            "extra_concepts order must not affect hash"
        );
    }

    // ── CoverageStatus ────────────────────────────────────────────────────────

    #[test]
    fn coverage_status_from_score_none_is_leaf() {
        let cfg = CoverageConfig::default();
        assert_eq!(CoverageStatus::from_score(None, &cfg), CoverageStatus::Leaf);
    }

    #[test]
    fn coverage_status_from_score_full_boundary() {
        let cfg = CoverageConfig::default(); // threshold_full = 0.9
        assert_eq!(
            CoverageStatus::from_score(Some(0.9), &cfg),
            CoverageStatus::Full
        );
        assert_eq!(
            CoverageStatus::from_score(Some(1.0), &cfg),
            CoverageStatus::Full
        );
    }

    #[test]
    fn coverage_status_from_score_partial_boundary() {
        let cfg = CoverageConfig::default(); // threshold_partial = 0.7
        assert_eq!(
            CoverageStatus::from_score(Some(0.7), &cfg),
            CoverageStatus::Partial
        );
        assert_eq!(
            CoverageStatus::from_score(Some(0.89), &cfg),
            CoverageStatus::Partial
        );
    }

    #[test]
    fn coverage_status_from_score_weak_below_partial() {
        let cfg = CoverageConfig::default(); // threshold_partial = 0.7
        assert_eq!(
            CoverageStatus::from_score(Some(0.0), &cfg),
            CoverageStatus::Weak
        );
        assert_eq!(
            CoverageStatus::from_score(Some(0.69), &cfg),
            CoverageStatus::Weak
        );
    }

    // ── CoverageStatus serde ──────────────────────────────────────────────────

    #[test]
    fn coverage_status_serde_roundtrip_all_variants() {
        let variants = [
            (CoverageStatus::Full, "\"Full\""),
            (CoverageStatus::Partial, "\"Partial\""),
            (CoverageStatus::Weak, "\"Weak\""),
            (CoverageStatus::Leaf, "\"N/A\""),
            (CoverageStatus::Unavailable, "\"Unavailable\""),
        ];
        for (status, expected_json) in &variants {
            let json = serde_json::to_string(status).expect("serialize");
            assert_eq!(&json, expected_json, "wrong JSON for {:?}", status);
            let back: CoverageStatus = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(&back, status, "roundtrip failed for {:?}", status);
        }
    }

    #[test]
    fn coverage_status_leaf_label_is_na() {
        assert_eq!(CoverageStatus::Leaf.label(), "N/A");
    }
}
