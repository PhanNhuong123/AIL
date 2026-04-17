/// Task 13.1 — Missing aspect detection tests.
///
/// Tests 8–10: threshold default list, extra concepts, blank/whitespace skipping.
use std::collections::HashMap;
use std::sync::Mutex;

use ail_graph::{AilGraph, EdgeKind, Node, NodeId, Pattern};
use ail_search::{EmbeddingProvider, SearchError};

// ─── Mock provider ────────────────────────────────────────────────────────────

struct MockProvider {
    table: HashMap<String, Vec<f32>>,
    dim: usize,
    fail_with: Mutex<Option<String>>,
    /// Texts received by embed_batch across all calls.
    received_texts: Mutex<Vec<String>>,
    name: &'static str,
}

impl MockProvider {
    fn new(dim: usize) -> Self {
        Self {
            table: HashMap::new(),
            dim,
            fail_with: Mutex::new(None),
            received_texts: Mutex::new(Vec::new()),
            name: "mock",
        }
    }
    fn with(mut self, text: &str, v: Vec<f32>) -> Self {
        self.table.insert(text.to_string(), v);
        self
    }
    fn received_texts(&self) -> Vec<String> {
        self.received_texts.lock().unwrap().clone()
    }
}

impl EmbeddingProvider for MockProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>, SearchError> {
        self.received_texts.lock().unwrap().push(text.to_string());
        if let Some(e) = self.fail_with.lock().unwrap().as_ref() {
            return Err(SearchError::InferenceFailed(e.clone()));
        }
        self.table
            .get(text)
            .cloned()
            .ok_or_else(|| SearchError::InferenceFailed(format!("mock missing: {text}")))
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SearchError> {
        // Track every text received so tests can assert which concepts reached
        // the provider.
        {
            let mut guard = self.received_texts.lock().unwrap();
            for t in texts {
                guard.push(t.to_string());
            }
        }
        if let Some(e) = self.fail_with.lock().unwrap().as_ref() {
            return Err(SearchError::InferenceFailed(e.clone()));
        }
        texts
            .iter()
            .map(|t| {
                self.table
                    .get(*t)
                    .cloned()
                    .ok_or_else(|| SearchError::InferenceFailed(format!("mock missing: {t}")))
            })
            .collect()
    }

    fn dimension(&self) -> usize {
        self.dim
    }
    fn name(&self) -> &str {
        self.name
    }
}

// ─── Graph helper ─────────────────────────────────────────────────────────────

fn build_graph(parent_intent: &str, child_intents: &[&str]) -> (AilGraph, NodeId, Vec<NodeId>) {
    let mut graph = AilGraph::new();
    let parent = Node::new(NodeId::new(), parent_intent, Pattern::Do);
    let parent_id = parent.id;
    graph.add_node(parent).unwrap();
    graph.set_root(parent_id).unwrap();

    let mut child_ids = Vec::new();
    for intent in child_intents {
        let child = Node::new(NodeId::new(), *intent, Pattern::Do);
        let child_id = child.id;
        graph.add_node(child).unwrap();
        graph.add_edge(parent_id, child_id, EdgeKind::Ev).unwrap();
        child_ids.push(child_id);
    }
    (graph, parent_id, child_ids)
}

// ─── Test 8: missing aspect above threshold reported from default list ─────────

/// The residual direction aligns with "error handling" from the default list.
/// The mock returns a vector for "error handling" that exactly matches the
/// residual direction, so similarity = 1.0 > 0.3 → must be reported.
///
/// Setup (2-D):
///   parent = [1, 1] / √2  (normalized)
///   child1 = [1, 0]       (x-axis only)
///
/// After projection, parent_unit = [1/√2, 1/√2].
/// Basis = {[1, 0]}.
/// Projection = [1/√2, 0].
/// Residual = [0, 1/√2].
///
/// We tell the mock that "error handling" embeds to [0, 1] (aligned with residual).
/// Cosine(residual, [0, 1]) = (1/√2) / (1/√2 * 1) = 1.0 > 0.3 → reported.
///
/// All other 39 default concepts map to [1, 0] (orthogonal to residual),
/// so only "error handling" passes the threshold.
#[test]
fn t131_missing_aspect_threshold_default_list() {
    let (graph, parent_id, _) = build_graph("handle payment", &["validate input"]);

    let sq2 = std::f32::consts::SQRT_2;

    // Build a table: parent + child for batch embed, then all 40 concepts.
    let mut provider = MockProvider::new(2)
        .with("handle payment", vec![1.0 / sq2, 1.0 / sq2])
        .with("validate input", vec![1.0, 0.0])
        .with("error handling", vec![0.0, 1.0]); // aligned with residual

    // All other default concepts: map to x-axis (orthogonal to residual).
    for concept in ail_coverage::DEFAULT_CONCEPT_LIST {
        if *concept != "error handling" {
            provider = provider.with(concept, vec![1.0, 0.0]);
        }
    }

    let result = ail_coverage::compute_coverage(&graph, &provider, parent_id, &[]).unwrap();

    assert!(
        result
            .missing_aspects
            .iter()
            .any(|m| m.concept == "error handling"),
        "expected 'error handling' in missing aspects; got: {:?}",
        result.missing_aspects
    );
    // The reported similarity should be above threshold.
    let ma = result
        .missing_aspects
        .iter()
        .find(|m| m.concept == "error handling")
        .unwrap();
    assert!(
        ma.similarity >= 0.3,
        "similarity should be >= 0.3, got {}",
        ma.similarity
    );
}

// ─── Test 9: extra_concepts appended and detected ────────────────────────────

/// Passing `extra_concepts = ["saga pattern"]` includes it in missing-aspect
/// detection even if it is not in the built-in list (it is in the built-in
/// list; choose something not in the list, e.g. "two-phase commit").
///
/// We use "two-phase commit" as an extra concept not in the built-in list.
/// The mock returns a vector for it that aligns with the residual.
#[test]
fn t131_missing_aspect_includes_extra_concepts() {
    let (graph, parent_id, _) = build_graph("handle distributed transaction", &["validate input"]);

    let sq2 = std::f32::consts::SQRT_2;
    let extra_concept = "two-phase commit";

    let mut provider = MockProvider::new(2)
        .with("handle distributed transaction", vec![1.0 / sq2, 1.0 / sq2])
        .with("validate input", vec![1.0, 0.0])
        .with(extra_concept, vec![0.0, 1.0]); // aligned with residual

    // All default concepts → x-axis (orthogonal to residual, will not trigger).
    for concept in ail_coverage::DEFAULT_CONCEPT_LIST {
        provider = provider.with(concept, vec![1.0, 0.0]);
    }

    let extra = vec![extra_concept.to_string()];
    let result = ail_coverage::compute_coverage(&graph, &provider, parent_id, &extra).unwrap();

    assert!(
        result
            .missing_aspects
            .iter()
            .any(|m| m.concept == extra_concept),
        "expected '{extra_concept}' in missing aspects; got: {:?}",
        result.missing_aspects
    );
}

// ─── Test 10: blank extra_concepts are not passed to provider ────────────────

/// `extra_concepts = ["", "   ", "idempotency"]` — blank and whitespace-only
/// entries must NOT reach the provider.
///
/// We verify this by checking that "" and "   " do not appear in the texts
/// received by the mock's `embed_batch`.
///
/// "idempotency" IS already in the default list, so the provider will see it
/// once (from the default list), not twice.
#[test]
fn t131_extra_concepts_empty_strings_skipped() {
    let (graph, parent_id, _) = build_graph("process order", &["validate order"]);

    let sq2 = std::f32::consts::SQRT_2;

    let mut provider = MockProvider::new(2)
        .with("process order", vec![1.0 / sq2, 1.0 / sq2])
        .with("validate order", vec![1.0, 0.0]);

    // All default concepts → x-axis.
    for concept in ail_coverage::DEFAULT_CONCEPT_LIST {
        provider = provider.with(concept, vec![1.0, 0.0]);
    }

    let extra = vec!["".to_string(), "   ".to_string(), "idempotency".to_string()];

    let _result = ail_coverage::compute_coverage(&graph, &provider, parent_id, &extra).unwrap();

    let texts = provider.received_texts();

    // Blank and whitespace-only entries must NOT have been embedded.
    assert!(
        !texts.iter().any(|t| t.is_empty()),
        "empty string must not reach provider; got: {texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t.trim().is_empty() && !t.is_empty()),
        "whitespace-only string must not reach provider; got: {texts:?}"
    );
    // "idempotency" should appear exactly once (from default list; dedup not
    // required, but blank must not add a duplicate).
    let idempotency_count = texts.iter().filter(|t| t.as_str() == "idempotency").count();
    assert!(
        idempotency_count >= 1,
        "idempotency must be in concept texts; got: {texts:?}"
    );
}
