use ail_search::EmbeddingProvider;

use crate::concepts::MISSING_ASPECT_THRESHOLD;
use crate::errors::CoverageError;
use crate::projection::{l2_norm, PARENT_NORM_EPSILON};
use crate::types::MissingAspect;

/// Compare the parent residual direction against `concept_texts` and return
/// all concepts whose cosine similarity with the residual exceeds
/// [`MISSING_ASPECT_THRESHOLD`].
///
/// Returns an empty `Vec` when:
/// - The residual is near-zero (fully covered — nothing missing).
/// - `concept_texts` is empty.
///
/// Results are sorted descending by similarity; ties broken alphabetically by
/// concept name.
pub(crate) fn detect_missing_aspects(
    residual: &[f32],
    concept_texts: &[&str],
    provider: &dyn EmbeddingProvider,
) -> Result<Vec<MissingAspect>, CoverageError> {
    if l2_norm(residual) < PARENT_NORM_EPSILON {
        return Ok(Vec::new());
    }
    if concept_texts.is_empty() {
        return Ok(Vec::new());
    }

    let vectors = provider.embed_batch(concept_texts)?;

    let residual_norm = l2_norm(residual);
    let mut out: Vec<MissingAspect> = Vec::new();

    for (i, v) in vectors.iter().enumerate() {
        let vn = l2_norm(v);
        if vn < PARENT_NORM_EPSILON {
            continue;
        }
        let sim = residual.iter().zip(v).map(|(a, b)| a * b).sum::<f32>() / (residual_norm * vn);
        if sim >= MISSING_ASPECT_THRESHOLD {
            out.push(MissingAspect {
                concept: concept_texts[i].to_string(),
                similarity: sim,
            });
        }
    }

    out.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.concept.cmp(&b.concept))
    });

    Ok(out)
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ail_search::SearchError;
    use std::collections::HashMap;

    // Minimal mock provider for unit tests in this module.
    struct MockProvider {
        table: HashMap<String, Vec<f32>>,
        dim: usize,
    }

    impl MockProvider {
        fn new(dim: usize) -> Self {
            Self {
                table: HashMap::new(),
                dim,
            }
        }
        fn with(mut self, text: &str, v: Vec<f32>) -> Self {
            self.table.insert(text.to_string(), v);
            self
        }
    }

    impl EmbeddingProvider for MockProvider {
        fn embed(&self, text: &str) -> Result<Vec<f32>, SearchError> {
            self.table
                .get(text)
                .cloned()
                .ok_or_else(|| SearchError::InferenceFailed(format!("mock missing: {text}")))
        }
        fn dimension(&self) -> usize {
            self.dim
        }
        fn name(&self) -> &str {
            "mock/unit"
        }
    }

    #[test]
    fn empty_residual_returns_empty_vec() {
        let residual = vec![0.0f32, 0.0, 0.0];
        let provider = MockProvider::new(3);
        let result = detect_missing_aspects(&residual, &["error handling"], &provider).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn zero_norm_concept_vector_skipped() {
        // If the provider returns a zero vector for a concept, it must not
        // produce a NaN similarity and must be skipped.
        let residual = vec![1.0f32, 0.0];
        let provider = MockProvider::new(2).with("error handling", vec![0.0, 0.0]); // zero vector
        let result = detect_missing_aspects(&residual, &["error handling"], &provider).unwrap();
        // Zero-norm concept is skipped; result must be empty.
        assert!(result.is_empty());
    }

    #[test]
    fn results_sorted_descending_by_similarity() {
        // Two concepts: b has higher similarity than a.
        let residual = vec![1.0f32, 0.0];
        let provider = MockProvider::new(2)
            .with("concept_a", vec![0.5f32, 0.5]) // sim ≈ 0.707
            .with("concept_b", vec![1.0f32, 0.0]); // sim = 1.0
        let result =
            detect_missing_aspects(&residual, &["concept_a", "concept_b"], &provider).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].concept, "concept_b");
        assert!(result[0].similarity > result[1].similarity);
    }
}
