use serde::{Deserialize, Serialize};

use ail_graph::NodeId;

// ─── Preview constant ─────────────────────────────────────────────────────────

pub(crate) const INTENT_PREVIEW_LEN: usize = 60;

/// Truncate `intent` to at most [`INTENT_PREVIEW_LEN`] characters, appending
/// `"…"` if the text was cut.  Truncation respects Unicode character boundaries.
pub(crate) fn truncate_preview(intent: &str) -> String {
    let mut chars = intent.chars();
    let mut result = String::with_capacity(INTENT_PREVIEW_LEN + 4);
    let mut count = 0usize;
    while count < INTENT_PREVIEW_LEN {
        match chars.next() {
            Some(c) => {
                result.push(c);
                count += 1;
            }
            None => return result,
        }
    }
    // Check whether there is anything remaining.
    if chars.next().is_some() {
        result.push('…');
    }
    result
}

// ─── Public types ─────────────────────────────────────────────────────────────

/// The result of computing coverage for a single node.
///
/// - `score: None` — node has no children (Guard D); leaf nodes are not weak.
/// - `score: Some(0.0)` with `empty_parent: true` — parent intent is near-zero (Guard A).
/// - `score: Some(x)` — normal score in `[0.0, 1.0]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageResult {
    /// Coverage score, or `None` for leaf nodes.
    pub score: Option<f32>,
    /// Per-child projection magnitudes.
    pub child_contributions: Vec<ChildContribution>,
    /// Software concepts present in the parent's residual but absent from children.
    pub missing_aspects: Vec<MissingAspect>,
    /// `true` when the parent's embedding vector was near-zero (Guard A).
    pub empty_parent: bool,
    /// `true` when the Gram-Schmidt basis degenerated to empty and
    /// average-cosine fallback was used (Guard C).
    pub degenerate_basis_fallback: bool,
}

/// Contribution of a single child node to parent coverage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChildContribution {
    /// The child node's identifier.
    pub node_id: NodeId,
    /// First [`INTENT_PREVIEW_LEN`] characters of the child's intent.
    pub intent_preview: String,
    /// Absolute projection magnitude onto the orthonormal basis vector for this
    /// child.  Zero when the child was skipped (nearly collinear) or fallback
    /// was used.
    pub projection_magnitude: f32,
}

/// A software concept found in the parent's residual direction but not covered
/// by children.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissingAspect {
    /// Human-readable concept label (from the built-in list or `extra_concepts`).
    pub concept: String,
    /// Cosine similarity between the residual direction and this concept's embedding.
    pub similarity: f32,
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_preview_empty() {
        assert_eq!(truncate_preview(""), "");
    }

    #[test]
    fn truncate_preview_short() {
        let s = "hello world";
        assert_eq!(truncate_preview(s), "hello world");
    }

    #[test]
    fn truncate_preview_exactly_60_chars() {
        let s: String = "a".repeat(60);
        let result = truncate_preview(&s);
        assert_eq!(result, s);
        assert!(!result.contains('…'));
    }

    #[test]
    fn truncate_preview_long_ascii() {
        let s: String = "a".repeat(80);
        let result = truncate_preview(&s);
        // Should be 60 chars + ellipsis
        let char_count: usize = result.chars().count();
        assert_eq!(char_count, 61, "expected 60 chars + '…', got {char_count}");
        assert!(result.ends_with('…'));
    }

    #[test]
    fn truncate_preview_multibyte_no_panic() {
        // Vietnamese characters are multi-byte; ensure no panic on char boundaries.
        let s = "Xử lý đầu vào và xác thực dữ liệu trong hệ thống thanh toán";
        let result = truncate_preview(s);
        // Result must be valid UTF-8 (no panic) and at most 61 chars (60 + ellipsis).
        assert!(result.chars().count() <= 61);
        // Emoji test.
        let emoji_s: String = "🎉".repeat(80);
        let emoji_result = truncate_preview(&emoji_s);
        assert!(emoji_result.chars().count() <= 61);
    }
}
