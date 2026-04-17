/// Cosine similarity threshold above which a concept is reported as missing.
///
/// v3.0: hard-coded at 0.3. Tuning tracked as v4.0 work (issue 13.1-E).
pub const MISSING_ASPECT_THRESHOLD: f32 = 0.3;

/// Default software concept list used for missing-aspect detection.
///
/// Covers common software engineering concerns across six categories:
/// security/access, reliability/resilience, data/persistence,
/// observability, interaction/delivery, and operations/background work.
///
/// Callers may extend this list via `[coverage].extra_concepts` in
/// `ail.config.toml` (13.2 concern). The crate itself never reads config files.
pub const DEFAULT_CONCEPT_LIST: &[&str] = &[
    // Security / access
    "input validation",
    "authentication",
    "authorization",
    "rate limiting",
    "cross-site request forgery protection",
    "secrets management",
    // Reliability / resilience
    "error handling",
    "retry logic",
    "idempotency",
    "transaction",
    "concurrency control",
    "deadlock prevention",
    "saga pattern",
    "circuit breaker",
    "timeout handling",
    "graceful degradation",
    "rollback",
    // Data / persistence
    "caching",
    "persistence",
    "pagination",
    "sorting",
    "filtering",
    "data migration",
    "schema validation",
    "serialization",
    "deserialization",
    "data consistency",
    "event sourcing",
    // Observability
    "logging",
    "observability",
    "metrics collection",
    "tracing",
    "audit trail",
    // Configuration / feature management
    "feature flag",
    "configuration management",
    // Interaction / delivery / operations
    "notification",
    "scheduling",
    "background job",
    "webhook delivery",
    "resource cleanup",
];

/// Merge the built-in concept list with caller-supplied `extras`.
///
/// Blank and whitespace-only strings in `extras` are skipped so that
/// an `extra_concepts = ["", "  "]` config entry does not pollute the batch.
///
/// The returned `Vec<&str>` borrows from `DEFAULT_CONCEPT_LIST` for built-in
/// entries and from `extras` for caller-supplied entries.
pub(crate) fn combined_concepts<'a>(extras: &'a [String]) -> Vec<&'a str> {
    let mut out: Vec<&'a str> = DEFAULT_CONCEPT_LIST.to_vec();
    for e in extras {
        let t = e.trim();
        if !t.is_empty() {
            out.push(t);
        }
    }
    out
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_concept_list_has_40_entries() {
        assert_eq!(
            DEFAULT_CONCEPT_LIST.len(),
            40,
            "expected exactly 40 concepts, got {}",
            DEFAULT_CONCEPT_LIST.len()
        );
    }

    #[test]
    fn default_concept_list_all_lowercase() {
        for concept in DEFAULT_CONCEPT_LIST {
            assert_eq!(
                *concept,
                concept.to_lowercase(),
                "concept not lowercase: {concept}"
            );
        }
    }

    #[test]
    fn default_concept_list_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for concept in DEFAULT_CONCEPT_LIST {
            assert!(seen.insert(*concept), "duplicate concept found: {concept}");
        }
    }

    #[test]
    fn combined_concepts_skips_blank_and_whitespace() {
        let extras = vec!["".to_string(), "   ".to_string(), "idempotency".to_string()];
        let result = combined_concepts(&extras);
        // Must include all defaults plus "idempotency" (blank/whitespace skipped)
        assert_eq!(result.len(), DEFAULT_CONCEPT_LIST.len() + 1);
        assert!(result.contains(&"idempotency"));
        assert!(!result.contains(&""));
        assert!(!result.contains(&"   "));
    }

    #[test]
    fn missing_aspect_threshold_is_0_3() {
        assert!(
            (MISSING_ASPECT_THRESHOLD - 0.3_f32).abs() < 1e-7,
            "expected 0.3, got {MISSING_ASPECT_THRESHOLD}"
        );
    }
}
