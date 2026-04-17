/// Task 13.1 — Coverage orchestrator tests.
///
/// Tests 4–7, 11, 12: Guards A/C/D, error propagation, node-not-found.
use std::collections::HashMap;
use std::sync::Mutex;

use ail_coverage::CoverageError;
use ail_graph::{AilGraph, EdgeKind, Node, NodeId, Pattern};
use ail_search::{EmbeddingProvider, SearchError};

// ─── Mock provider ────────────────────────────────────────────────────────────

struct MockProvider {
    table: HashMap<String, Vec<f32>>,
    dim: usize,
    fail_with: Mutex<Option<SearchError>>,
    call_count: Mutex<usize>,
    name: &'static str,
}

impl MockProvider {
    fn new(dim: usize) -> Self {
        Self {
            table: HashMap::new(),
            dim,
            fail_with: Mutex::new(None),
            call_count: Mutex::new(0),
            name: "mock",
        }
    }
    fn with(mut self, text: &str, v: Vec<f32>) -> Self {
        self.table.insert(text.to_string(), v);
        self
    }
    fn fail(self, e: SearchError) -> Self {
        *self.fail_with.lock().unwrap() = Some(e);
        self
    }
    #[allow(dead_code)]
    fn call_count(&self) -> usize {
        *self.call_count.lock().unwrap()
    }
}

impl EmbeddingProvider for MockProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>, SearchError> {
        *self.call_count.lock().unwrap() += 1;
        if let Some(ref e) = *self.fail_with.lock().unwrap() {
            return Err(SearchError::InferenceFailed(format!("{e}")));
        }
        self.table
            .get(text)
            .cloned()
            .ok_or_else(|| SearchError::InferenceFailed(format!("mock missing: {text}")))
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SearchError> {
        *self.call_count.lock().unwrap() += 1;
        if let Some(ref e) = *self.fail_with.lock().unwrap() {
            return Err(SearchError::InferenceFailed(format!("{e}")));
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

// ─── Test 4: Guard A — near-zero parent ──────────────────────────────────────

/// When the parent embedding is the zero vector, `score = Some(0.0)` and
/// `empty_parent = true`.
#[test]
fn t131_empty_parent_returns_zero_with_reason() {
    let (graph, parent_id, _) = build_graph("validate", &["check input"]);

    // Parent embeds to zero vector; child has a normal vector.
    let provider = MockProvider::new(2)
        .with("validate", vec![0.0, 0.0])
        .with("check input", vec![1.0, 0.0]);

    let result = ail_coverage::compute_coverage(&graph, &provider, parent_id, &[]).unwrap();

    assert_eq!(
        result.score,
        Some(0.0),
        "Guard A: score should be Some(0.0)"
    );
    assert!(result.empty_parent, "Guard A: empty_parent should be true");
    assert!(
        result.missing_aspects.is_empty(),
        "Guard A: missing_aspects should be empty"
    );
    assert!(!result.degenerate_basis_fallback);
}

// ─── Test 5: Guard C — all children have zero embedding → cosine fallback ───────

/// All children embed to the zero vector so the Gram-Schmidt basis degenerates
/// to empty (every child is rejected by the epsilon guard).  Must use
/// average-cosine fallback, report a bounded score, and log the degenerate-basis
/// warn.
///
/// Setup (2-D):
///   parent = [1, 0]       (non-zero)
///   child1 = [0, 0]       (zero vector → rejected during basis build)
///   child2 = [0, 0]       (zero vector → rejected)
///
/// After normalization all children remain zero.  Since all are below
/// BASIS_EPSILON the basis is empty → Guard C fires.
/// `average_cosine_fallback` also skips zero children → score = 0.0.
#[test]
fn t131_all_children_collinear_uses_cosine_fallback() {
    testing_logger::setup();

    let (graph, parent_id, _) = build_graph("process payment", &["debit account", "charge card"]);

    let provider = MockProvider::new(2)
        .with("process payment", vec![1.0, 0.0])
        .with("debit account", vec![0.0, 0.0]) // zero → rejected
        .with("charge card", vec![0.0, 0.0]); // zero → rejected

    let result = ail_coverage::compute_coverage(&graph, &provider, parent_id, &[]).unwrap();

    assert!(
        result.degenerate_basis_fallback,
        "Guard C: degenerate_basis_fallback should be true"
    );
    let score = result.score.expect("Guard C should produce a Some score");
    assert!(
        (0.0..=1.0).contains(&score),
        "Guard C: score should be in [0, 1], got {score}"
    );

    testing_logger::validate(|logs| {
        assert!(
            logs.iter()
                .any(|l| l.body.contains("orthonormal basis degenerated")),
            "expected 'orthonormal basis degenerated' warn log"
        );
    });
}

// ─── Test 6: Guard D — leaf node returns None ────────────────────────────────

/// A node with no children must return `score = None`.
/// `score != Some(0.0)` distinguishes leaf from zero-coverage.
#[test]
fn t131_leaf_node_returns_none() {
    let mut graph = AilGraph::new();
    let leaf = Node::new(NodeId::new(), "validate sender balance", Pattern::Check);
    let leaf_id = leaf.id;
    graph.add_node(leaf).unwrap();
    graph.set_root(leaf_id).unwrap();

    // Provider is never called for a leaf node; use a zero-table provider.
    let provider = MockProvider::new(2);

    let result = ail_coverage::compute_coverage(&graph, &provider, leaf_id, &[]).unwrap();

    assert_eq!(
        result.score, None,
        "Guard D: leaf node should return score=None"
    );
    assert_ne!(
        result.score,
        Some(0.0),
        "Guard D: leaf must not be confused with zero-coverage"
    );
    assert!(result.child_contributions.is_empty());
}

// ─── Test 7 (inline unit in projection.rs): clamp logging ────────────────────
// This test is placed in projection.rs inline (#[cfg(test)]).

// ─── Test 11: provider error propagates as CoverageError::Embedding ───────────

/// When the provider fails, `compute_coverage` must return
/// `CoverageError::Embedding(_)`.
#[test]
fn t131_provider_error_propagates_as_coverage_error() {
    let (graph, parent_id, _) = build_graph("transfer", &["validate"]);

    let provider =
        MockProvider::new(2).fail(SearchError::InferenceFailed("deliberate failure".into()));

    let err = ail_coverage::compute_coverage(&graph, &provider, parent_id, &[])
        .expect_err("should propagate provider error");

    assert!(
        matches!(err, CoverageError::Embedding(_)),
        "expected CoverageError::Embedding, got: {err:?}"
    );
}

// ─── Test 12: missing node → CoverageError::NodeNotFound ─────────────────────

/// Passing a `node_id` that does not exist in the graph must return
/// `CoverageError::NodeNotFound`.
#[test]
fn t131_node_not_found_error() {
    let graph = AilGraph::new();
    let missing_id = NodeId::new();
    let provider = MockProvider::new(2);

    let err = ail_coverage::compute_coverage(&graph, &provider, missing_id, &[])
        .expect_err("should return NodeNotFound for missing node");

    assert!(
        matches!(err, CoverageError::NodeNotFound(_)),
        "expected CoverageError::NodeNotFound, got: {err:?}"
    );
}
