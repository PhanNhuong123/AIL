/// Task 13.1 — Projection tests.
///
/// Tests 1–3: orthogonal full coverage, partial coverage, and near-collinear
/// Guard B (skip + warn log).
use std::collections::HashMap;
use std::sync::Mutex;

use ail_graph::{AilGraph, EdgeKind, Node, NodeId, Pattern};
use ail_search::{EmbeddingProvider, SearchError};

// ─── Mock provider ────────────────────────────────────────────────────────────

struct MockProvider {
    table: HashMap<String, Vec<f32>>,
    dim: usize,
    fail_with: Mutex<Option<String>>,
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
    #[allow(dead_code)]
    fn call_count(&self) -> usize {
        *self.call_count.lock().unwrap()
    }
}

impl EmbeddingProvider for MockProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>, SearchError> {
        *self.call_count.lock().unwrap() += 1;
        if let Some(e) = self.fail_with.lock().unwrap().as_ref() {
            return Err(SearchError::InferenceFailed(e.clone()));
        }
        self.table
            .get(text)
            .cloned()
            .ok_or_else(|| SearchError::InferenceFailed(format!("mock missing: {text}")))
    }
    fn dimension(&self) -> usize {
        self.dim
    }
    fn name(&self) -> &str {
        self.name
    }
}

// ─── Graph helper ─────────────────────────────────────────────────────────────

/// Build an `AilGraph` with one parent node and N child nodes, all connected
/// by Ev (structural parent→child) edges.
///
/// Returns `(graph, parent_id, child_ids)`.
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

// ─── Test 1: orthogonal children → full coverage ─────────────────────────────

/// Two orthogonal child unit vectors that together span the parent → score ≈ 1.0.
///
/// Setup (3-D):
///   parent = [1, 1, 0] / √2  (45° in xy-plane)
///   child1 = [1, 0, 0]       (x-axis)
///   child2 = [0, 1, 0]       (y-axis)
///
/// The orthonormal basis {e_x, e_y} fully spans the parent, so the residual
/// should be near-zero and score ≈ 1.0.
#[test]
fn t131_orthogonal_children_full_coverage() {
    let (graph, parent_id, _) =
        build_graph("transfer funds", &["validate input", "authenticate user"]);

    let parent_text = "transfer funds";
    let child1_text = "validate input";
    let child2_text = "authenticate user";
    let sq2 = std::f32::consts::SQRT_2;

    let mut provider = MockProvider::new(3)
        .with(parent_text, vec![1.0 / sq2, 1.0 / sq2, 0.0])
        .with(child1_text, vec![1.0, 0.0, 0.0])
        .with(child2_text, vec![0.0, 1.0, 0.0]);

    // Default concepts map to x-axis (orthogonal to residual) to avoid
    // InferenceFailed errors if residual is non-trivial due to float imprecision.
    for concept in ail_coverage::DEFAULT_CONCEPT_LIST {
        provider = provider.with(concept, vec![1.0, 0.0, 0.0]);
    }

    let result = ail_coverage::compute_coverage(&graph, &provider, parent_id, &[]).unwrap();

    let score = result.score.expect("should have a score");
    assert!(
        score > 0.99,
        "orthogonal children spanning parent should yield score ≈ 1.0, got {score}"
    );
    assert!(!result.empty_parent);
    assert!(!result.degenerate_basis_fallback);
    assert_eq!(result.child_contributions.len(), 2);
}

// ─── Test 2: partial coverage ─────────────────────────────────────────────────

/// Parent has a component outside the child span → score < 1.0 and bounded.
///
/// Setup (2-D):
///   parent = [1, 1] / √2
///   child1 = [1, 0]          (x-axis only)
///
/// The child spans only the x-axis.  The parent residual has magnitude 1/√2,
/// so unclamped score = 1 - 1/√2 ≈ 0.293, which is < 1.
///
/// All default concepts map to [1, 0] (orthogonal to residual [0, 1/√2]),
/// so no missing aspects are reported — the test only validates the score.
#[test]
fn t131_parallel_children_partial_coverage() {
    let (graph, parent_id, _) = build_graph("handle transaction", &["validate sender"]);

    let parent_text = "handle transaction";
    let child_text = "validate sender";
    let sq2 = std::f32::consts::SQRT_2;

    let mut provider = MockProvider::new(2)
        .with(parent_text, vec![1.0 / sq2, 1.0 / sq2])
        .with(child_text, vec![1.0, 0.0]);

    // Provide orthogonal-to-residual vectors for all default concepts so the
    // mock does not fail when detect_missing_aspects batch-embeds them.
    for concept in ail_coverage::DEFAULT_CONCEPT_LIST {
        provider = provider.with(concept, vec![1.0, 0.0]);
    }

    let result = ail_coverage::compute_coverage(&graph, &provider, parent_id, &[]).unwrap();

    let score = result.score.expect("should have a score");
    assert!(
        score > 0.0 && score < 1.0,
        "partial coverage should be in (0, 1), got {score}"
    );
    assert!(!result.empty_parent);
    assert!(!result.degenerate_basis_fallback);
}

// ─── Test 3: near-collinear → Guard B skip + warn log ────────────────────────

/// Two nearly collinear children: the second reduces to near-zero after
/// orthogonalization and must be skipped.  A warn log must be emitted.
///
/// Setup (2-D):
///   parent = [1, 0]
///   child1 = [1, 0]     (x-axis — same as parent)
///   child2 = [1, 1e-10] (nearly identical to child1)
///
/// After Gram-Schmidt, child2's residual has norm ≈ 1e-10 < 1e-9 → skipped.
#[test]
fn t131_near_collinear_children_triggers_skip() {
    testing_logger::setup();

    let (graph, parent_id, _) = build_graph("process payment", &["debit account", "debit card"]);

    let parent_text = "process payment";
    let child1_text = "debit account";
    let child2_text = "debit card";

    let mut provider = MockProvider::new(2)
        .with(parent_text, vec![1.0f32, 0.0])
        .with(child1_text, vec![1.0f32, 0.0])
        .with(child2_text, vec![1.0f32, 1e-11]); // nearly collinear with child1

    // Default concepts: all map to x-axis so detect_missing_aspects (if called)
    // won't fail with InferenceFailed.
    for concept in ail_coverage::DEFAULT_CONCEPT_LIST {
        provider = provider.with(concept, vec![1.0, 0.0]);
    }

    let result = ail_coverage::compute_coverage(&graph, &provider, parent_id, &[]).unwrap();

    // Score must be Some and bounded.
    let score = result.score.expect("should have a score");
    assert!((0.0..=1.0).contains(&score), "score out of range: {score}");

    // Warn log must have been emitted.
    testing_logger::validate(|logs| {
        assert!(
            logs.iter().any(|l| l.body.contains("nearly collinear")),
            "expected 'nearly collinear' warn log"
        );
    });
}
