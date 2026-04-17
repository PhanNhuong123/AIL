/// Minimum norm for the parent vector before we declare it near-zero.
pub(crate) const PARENT_NORM_EPSILON: f32 = 1e-9;

/// Minimum norm for a post-orthogonalization child vector before we skip it.
pub(crate) const BASIS_EPSILON: f32 = 1e-9;

// ─── Basic math ───────────────────────────────────────────────────────────────

/// Compute the L2 norm (Euclidean length) of `v`.
pub(crate) fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Normalize `v` in place to unit length.
///
/// No-op if `l2_norm(v) < PARENT_NORM_EPSILON` (avoids division by near-zero).
pub(crate) fn normalize_in_place(v: &mut [f32]) {
    let n = l2_norm(v);
    if n < PARENT_NORM_EPSILON {
        return;
    }
    for x in v.iter_mut() {
        *x /= n;
    }
}

/// Dot product of two equal-length slices.
pub(crate) fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

// ─── Gram-Schmidt ─────────────────────────────────────────────────────────────

/// Result of [`build_orthonormal_basis`].
pub(crate) struct BasisBuildOutcome {
    /// Orthonormal basis vectors derived from the accepted children.
    pub basis: Vec<Vec<f32>>,
    /// Indices into the original `child_vecs` slice that were accepted into the
    /// basis (i.e., not skipped due to near-collinearity).
    pub accepted_child_indices: Vec<usize>,
    /// Number of child vectors skipped because their post-orthogonalization norm
    /// was below [`BASIS_EPSILON`].
    #[allow(dead_code)]
    pub skipped_collinear_count: usize,
}

/// Build an orthonormal basis from `child_vecs` using Modified Gram-Schmidt
/// with a single reorthogonalization pass (two passes total) for numerical
/// stability.
///
/// Vectors whose post-orthogonalization norm falls below [`BASIS_EPSILON`] are
/// skipped and a `log::warn!` is emitted (issue 13.1-A).
pub(crate) fn build_orthonormal_basis(child_vecs: &[Vec<f32>]) -> BasisBuildOutcome {
    let mut basis: Vec<Vec<f32>> = Vec::new();
    let mut accepted = Vec::new();
    let mut skipped = 0usize;

    for (idx, child) in child_vecs.iter().enumerate() {
        let mut u = child.clone();

        // Two MGS passes for numerical stability (single reorthogonalization).
        for _ in 0..2 {
            for b in &basis {
                let coeff = dot(&u, b);
                for (ui, bi) in u.iter_mut().zip(b.iter()) {
                    *ui -= coeff * bi;
                }
            }
        }

        let n = l2_norm(&u);
        if n < BASIS_EPSILON {
            skipped += 1;
            log::warn!("child vectors nearly collinear — coverage may be underestimated");
            continue;
        }

        for x in u.iter_mut() {
            *x /= n;
        }
        basis.push(u);
        accepted.push(idx);
    }

    BasisBuildOutcome {
        basis,
        accepted_child_indices: accepted,
        skipped_collinear_count: skipped,
    }
}

// ─── Projection ───────────────────────────────────────────────────────────────

/// Result of [`project_onto_basis`].
pub(crate) struct ProjectionOutcome {
    /// The component of `parent` lying in the span of the basis.
    #[allow(dead_code)]
    pub projection: Vec<f32>,
    /// The component of `parent` orthogonal to the basis span.
    pub residual: Vec<f32>,
    /// Absolute projection magnitude onto each basis vector, in basis order.
    pub per_basis_projection_magnitudes: Vec<f32>,
}

/// Project `parent` onto the orthonormal `basis` and return the projection,
/// residual, and per-basis magnitudes.
pub(crate) fn project_onto_basis(parent: &[f32], basis: &[Vec<f32>]) -> ProjectionOutcome {
    let dim = parent.len();
    let mut projection = vec![0.0f32; dim];
    let mut mags = Vec::with_capacity(basis.len());

    for b in basis {
        let c = dot(parent, b);
        mags.push(c.abs());
        for (pi, bi) in projection.iter_mut().zip(b.iter()) {
            *pi += c * bi;
        }
    }

    let residual: Vec<f32> = parent
        .iter()
        .zip(projection.iter())
        .map(|(p, q)| p - q)
        .collect();

    ProjectionOutcome {
        projection,
        residual,
        per_basis_projection_magnitudes: mags,
    }
}

// ─── Score helpers ────────────────────────────────────────────────────────────

/// Clamp an unclamped coverage score to `[0.0, 1.0]`, emitting `log::warn!`
/// when the value is NaN or outside the expected range (issue 13.1-B).
pub(crate) fn clamp_score_with_log(unclamped: f32) -> f32 {
    if unclamped.is_nan() {
        log::warn!("ail-coverage: coverage score was NaN; substituting 0.0");
        return 0.0;
    }
    if !(0.0..=1.0).contains(&unclamped) {
        log::warn!("ail-coverage: clamping out-of-range coverage score {unclamped} to [0.0, 1.0]");
        return unclamped.clamp(0.0, 1.0);
    }
    unclamped
}

/// Fallback score (Guard C) when the Gram-Schmidt basis degenerated to empty.
///
/// Returns the average cosine similarity between `parent` and each raw child
/// vector, clamped to `[0.0, 1.0]`.  Returns `0.0` for an empty child list or
/// a near-zero parent.
pub(crate) fn average_cosine_fallback(parent: &[f32], raw_children: &[Vec<f32>]) -> f32 {
    if raw_children.is_empty() {
        return 0.0;
    }
    let pn = l2_norm(parent);
    if pn < PARENT_NORM_EPSILON {
        return 0.0;
    }
    let mut acc = 0.0f32;
    let mut count = 0usize;
    for c in raw_children {
        let cn = l2_norm(c);
        if cn < PARENT_NORM_EPSILON {
            continue;
        }
        acc += dot(parent, c) / (pn * cn);
        count += 1;
    }
    if count == 0 {
        return 0.0;
    }
    (acc / count as f32).clamp(0.0, 1.0)
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── l2_norm ──────────────────────────────────────────────────────────────

    #[test]
    fn l2_norm_zero_vector() {
        assert!((l2_norm(&[0.0, 0.0, 0.0]) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn l2_norm_unit_vector() {
        assert!((l2_norm(&[1.0, 0.0]) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn l2_norm_known_value() {
        assert!((l2_norm(&[3.0, 4.0]) - 5.0).abs() < 1e-6);
    }

    // ── normalize_in_place ───────────────────────────────────────────────────

    #[test]
    fn normalize_in_place_produces_unit_vector() {
        let mut v = vec![3.0f32, 4.0];
        normalize_in_place(&mut v);
        assert!((l2_norm(&v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn normalize_in_place_near_zero_no_panic() {
        let mut v = vec![0.0f32, 0.0];
        normalize_in_place(&mut v); // must not divide by zero
        assert_eq!(v, vec![0.0, 0.0]);
    }

    // ── dot ──────────────────────────────────────────────────────────────────

    #[test]
    fn dot_orthogonal() {
        assert!((dot(&[1.0, 0.0], &[0.0, 1.0]) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn dot_parallel() {
        assert!((dot(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-9);
    }

    // ── build_orthonormal_basis ───────────────────────────────────────────────

    #[test]
    fn basis_from_two_orthogonal_unit_vectors() {
        let children = vec![vec![1.0f32, 0.0], vec![0.0f32, 1.0]];
        let outcome = build_orthonormal_basis(&children);
        assert_eq!(outcome.basis.len(), 2);
        assert_eq!(outcome.accepted_child_indices, vec![0, 1]);
        assert_eq!(outcome.skipped_collinear_count, 0);
    }

    #[test]
    fn basis_skips_zero_vector() {
        let children = vec![vec![1.0f32, 0.0], vec![0.0f32, 0.0]];
        let outcome = build_orthonormal_basis(&children);
        assert_eq!(outcome.basis.len(), 1);
        assert_eq!(outcome.skipped_collinear_count, 1);
    }

    #[test]
    fn basis_skips_collinear_vector() {
        // Two parallel vectors: second reduces to zero after orthogonalization.
        let children = vec![vec![1.0f32, 0.0], vec![2.0f32, 0.0]];
        let outcome = build_orthonormal_basis(&children);
        assert_eq!(outcome.basis.len(), 1);
        assert_eq!(outcome.skipped_collinear_count, 1);
    }

    // ── project_onto_basis ───────────────────────────────────────────────────

    #[test]
    fn project_identity_when_parent_in_span() {
        let parent = vec![1.0f32, 0.0];
        let basis = vec![vec![1.0f32, 0.0]];
        let out = project_onto_basis(&parent, &basis);
        // Residual should be near zero.
        assert!(l2_norm(&out.residual) < 1e-6);
        assert!((out.per_basis_projection_magnitudes[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn project_full_residual_when_orthogonal() {
        let parent = vec![0.0f32, 1.0];
        let basis = vec![vec![1.0f32, 0.0]];
        let out = project_onto_basis(&parent, &basis);
        // Parent is orthogonal to basis: projection ≈ 0, residual ≈ parent.
        assert!(l2_norm(&out.projection) < 1e-6);
        assert!((l2_norm(&out.residual) - 1.0).abs() < 1e-6);
    }

    // ── clamp_score_with_log ─────────────────────────────────────────────────

    #[test]
    fn clamp_happy_path() {
        // In-range values pass through unchanged.
        assert!((clamp_score_with_log(0.75) - 0.75).abs() < 1e-9);
        assert!((clamp_score_with_log(0.0) - 0.0).abs() < 1e-9);
        assert!((clamp_score_with_log(1.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn clamp_negative_value_logs_and_clamps() {
        testing_logger::setup();
        let result = clamp_score_with_log(-0.1);
        assert!((result - 0.0).abs() < 1e-9);
        testing_logger::validate(|logs| {
            assert!(
                logs.iter()
                    .any(|l| l.body.contains("clamping out-of-range")),
                "expected warn log for out-of-range score"
            );
        });
    }

    #[test]
    fn clamp_above_1_logs_and_clamps() {
        testing_logger::setup();
        let result = clamp_score_with_log(1.5);
        assert!((result - 1.0).abs() < 1e-9);
        testing_logger::validate(|logs| {
            assert!(
                logs.iter()
                    .any(|l| l.body.contains("clamping out-of-range")),
                "expected warn log for out-of-range score"
            );
        });
    }

    #[test]
    fn clamp_nan_logs_and_returns_zero() {
        testing_logger::setup();
        let result = clamp_score_with_log(f32::NAN);
        assert!((result - 0.0).abs() < 1e-9);
        testing_logger::validate(|logs| {
            assert!(
                logs.iter().any(|l| l.body.contains("NaN")),
                "expected warn log for NaN score"
            );
        });
    }

    // ── average_cosine_fallback ───────────────────────────────────────────────

    #[test]
    fn cosine_fallback_empty_children() {
        let parent = vec![1.0f32, 0.0];
        assert!((average_cosine_fallback(&parent, &[]) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn cosine_fallback_near_zero_parent() {
        let parent = vec![0.0f32, 0.0];
        let children = vec![vec![1.0f32, 0.0]];
        assert!((average_cosine_fallback(&parent, &children) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn cosine_fallback_identical_vectors() {
        let parent = vec![1.0f32, 0.0];
        let children = vec![vec![1.0f32, 0.0]];
        let score = average_cosine_fallback(&parent, &children);
        assert!((score - 1.0).abs() < 1e-6, "got {score}");
    }
}
