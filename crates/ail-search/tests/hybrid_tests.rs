/// Task 10.2 — Hybrid Search (BM25 + Semantic) tests.
///
/// All tests are always-on: they use a `ConstantProvider` mock and
/// `EmbeddingIndex::from_vectors` so the ONNX model is never required.
///
/// BM25 results are produced by `Bm25Index::build_from_graph` on in-memory
/// `AilGraph` instances, matching the real BM25 path.
use std::collections::HashMap;

use ail_graph::{AilGraph, Bm25Index, Node, NodeId, Pattern};
use ail_search::{cosine_similarity, hybrid_search, EmbeddingIndex, RankingSource, SearchError};

// ─── Mock provider ────────────────────────────────────────────────────────────

use ail_search::EmbeddingProvider;

/// Provider that maps text → vector via a lookup table; panics on unknown text.
struct LookupProvider {
    dim: usize,
    table: HashMap<String, Vec<f32>>,
}

impl LookupProvider {
    fn new(dim: usize, table: HashMap<String, Vec<f32>>) -> Self {
        Self { dim, table }
    }
}

impl EmbeddingProvider for LookupProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>, SearchError> {
        self.table.get(text).cloned().ok_or_else(|| {
            SearchError::InferenceFailed(format!("LookupProvider: unknown text '{text}'"))
        })
    }
    fn dimension(&self) -> usize {
        self.dim
    }
    fn name(&self) -> &str {
        "test/lookup"
    }
}

// ─── Graph helpers ────────────────────────────────────────────────────────────

fn make_node(intent: &str, pattern: Pattern) -> Node {
    Node::new(NodeId::new(), intent, pattern)
}

fn make_node_named(intent: &str, pattern: Pattern, name: &str) -> Node {
    let mut n = Node::new(NodeId::new(), intent, pattern);
    n.metadata.name = Some(name.to_string());
    n
}

// ─── Cosine similarity tests ──────────────────────────────────────────────────

#[test]
fn t102_cosine_similarity_identical_vectors() {
    let v = vec![0.6_f32, 0.8];
    let sim = cosine_similarity(&v, &v);
    assert!(
        (sim - 1.0).abs() < 1e-6,
        "identical vectors must have cosine 1.0, got {sim}"
    );
}

#[test]
fn t102_cosine_similarity_orthogonal_vectors() {
    let a = vec![1.0_f32, 0.0];
    let b = vec![0.0_f32, 1.0];
    let sim = cosine_similarity(&a, &b);
    assert!(
        sim.abs() < 1e-6,
        "orthogonal vectors must have cosine 0.0, got {sim}"
    );
}

#[test]
fn t102_cosine_similarity_opposite_vectors() {
    let a = vec![1.0_f32, 0.0];
    let b = vec![-1.0_f32, 0.0];
    let sim = cosine_similarity(&a, &b);
    assert!(
        (sim - (-1.0)).abs() < 1e-6,
        "opposite vectors must have cosine -1.0, got {sim}"
    );
}

// ─── RRF scoring test ─────────────────────────────────────────────────────────

/// Verify that the output `rrf_score` matches the hand-computed value.
#[test]
fn t102_hybrid_search_rrf_scoring_correct() {
    // Single node that appears at BM25 rank 0 and semantic rank 0.
    // Expected: 1/(60+0+1) + 1/(60+0+1) = 2/61
    let mut graph = AilGraph::new();
    let node = make_node("transfer money safely", Pattern::Do);
    let nid = node.id;
    graph.add_node(node).unwrap();
    graph.set_root(nid).unwrap();

    let bm25 = Bm25Index::build_from_graph(&graph);
    let bm25_results = bm25.search("transfer", 20, &graph);

    // Build embedding index with a vector pointing "in the same direction" as
    // the query vector so it ranks first semantically.
    let query_vec = vec![1.0_f32, 0.0];
    let mut vectors = HashMap::new();
    vectors.insert(nid, vec![1.0_f32, 0.0]); // cosine 1.0 with query

    // LookupProvider returns query_vec for the query "transfer money safely" embed call.
    let mut table = HashMap::new();
    table.insert("transfer money safely".to_string(), query_vec.clone());
    let provider = LookupProvider::new(2, table);
    let index = EmbeddingIndex::from_vectors(Box::new(provider), vectors);

    let results = hybrid_search(
        "transfer money safely",
        &bm25_results,
        Some(&index),
        &graph,
        10,
    )
    .expect("hybrid search should succeed");

    assert_eq!(results.len(), 1);
    let expected_score = 2.0 / 61.0_f64; // rank 0 in both lists
    assert!(
        (results[0].rrf_score - expected_score).abs() < 1e-10,
        "expected rrf_score {expected_score}, got {}",
        results[0].rrf_score
    );
}

// ─── Combined results test ────────────────────────────────────────────────────

/// Hybrid search returns results from BOTH BM25 and semantic sources.
#[test]
fn t102_hybrid_search_returns_combined_results() {
    let mut graph = AilGraph::new();

    // Node A: strong BM25 match for "transfer"
    let a = make_node("transfer money between wallets", Pattern::Do);
    let aid = a.id;
    graph.add_node(a).unwrap();
    graph.set_root(aid).unwrap();

    // Node B: no BM25 match but will be a strong semantic match
    let b = make_node("move funds between accounts", Pattern::Do);
    let bid = b.id;
    graph.add_node(b).unwrap();

    let bm25 = Bm25Index::build_from_graph(&graph);
    let bm25_results = bm25.search("transfer", 20, &graph);

    // Semantic: B is similar to query, A is not
    let mut vectors = HashMap::new();
    vectors.insert(aid, vec![0.0_f32, 1.0]); // orthogonal to query
    vectors.insert(bid, vec![1.0_f32, 0.0]); // identical to query

    let mut table = HashMap::new();
    table.insert("transfer".to_string(), vec![1.0_f32, 0.0]);
    let provider = LookupProvider::new(2, table);
    let index = EmbeddingIndex::from_vectors(Box::new(provider), vectors);

    let results = hybrid_search("transfer", &bm25_results, Some(&index), &graph, 10)
        .expect("hybrid search should succeed");

    // Both nodes should appear
    let ids: Vec<NodeId> = results.iter().map(|r| r.node_id).collect();
    assert!(ids.contains(&aid), "BM25 match (A) should be in results");
    assert!(
        ids.contains(&bid),
        "semantic match (B) should be in results"
    );
}

// ─── BM25-only match ──────────────────────────────────────────────────────────

/// A node matched by BM25 but not semantically similar still appears in output.
#[test]
fn t102_hybrid_bm25_only_match_still_returned() {
    let mut graph = AilGraph::new();
    let n = make_node("validate sender balance", Pattern::Check);
    let nid = n.id;
    graph.add_node(n).unwrap();
    graph.set_root(nid).unwrap();

    let bm25 = Bm25Index::build_from_graph(&graph);
    let bm25_results = bm25.search("validate", 20, &graph);
    assert!(!bm25_results.is_empty(), "BM25 must find the node");

    // Semantic: node vector is orthogonal to query → cosine 0; still returned via BM25
    let mut vectors = HashMap::new();
    vectors.insert(nid, vec![0.0_f32, 1.0]);
    let mut table = HashMap::new();
    table.insert("validate".to_string(), vec![1.0_f32, 0.0]);
    let provider = LookupProvider::new(2, table);
    let index = EmbeddingIndex::from_vectors(Box::new(provider), vectors);

    let results = hybrid_search("validate", &bm25_results, Some(&index), &graph, 10)
        .expect("hybrid search should succeed");

    let ids: Vec<NodeId> = results.iter().map(|r| r.node_id).collect();
    assert!(
        ids.contains(&nid),
        "BM25-matched node must appear in hybrid results"
    );
}

// ─── Semantic-only match ──────────────────────────────────────────────────────

/// A node found only by semantic search (no keyword overlap) still appears.
/// This covers the "query intent that keyword search alone misses" requirement.
#[test]
fn t102_hybrid_semantic_only_match_still_returned() {
    let mut graph = AilGraph::new();

    // Node: "InsufficientBalanceError" — no overlap with query "not enough money"
    let n = make_node_named(
        "error raised when balance is insufficient",
        Pattern::Error,
        "InsufficientBalanceError",
    );
    let nid = n.id;
    graph.add_node(n).unwrap();
    graph.set_root(nid).unwrap();

    let bm25 = Bm25Index::build_from_graph(&graph);
    // BM25 will return 0 results for "not enough money" (no term overlap)
    let bm25_results = bm25.search("not enough money", 20, &graph);

    // Semantic: node vector is identical to query → strong match
    let mut vectors = HashMap::new();
    vectors.insert(nid, vec![1.0_f32, 0.0]);
    let mut table = HashMap::new();
    table.insert("not enough money".to_string(), vec![1.0_f32, 0.0]);
    let provider = LookupProvider::new(2, table);
    let index = EmbeddingIndex::from_vectors(Box::new(provider), vectors);

    let results = hybrid_search("not enough money", &bm25_results, Some(&index), &graph, 10)
        .expect("hybrid search should succeed");

    assert!(
        !results.is_empty(),
        "semantic-only match must appear in results"
    );
    assert_eq!(results[0].node_id, nid);
    assert_eq!(results[0].source, RankingSource::SemanticOnly);
    assert_eq!(results[0].bm25_rank, None);
    assert_eq!(results[0].semantic_rank, Some(0));
}

// ─── Both sources rank higher ─────────────────────────────────────────────────

/// A node present in both BM25 and semantic results outranks a node in only one.
#[test]
fn t102_hybrid_both_match_ranks_higher() {
    let mut graph = AilGraph::new();

    // Node A: appears in both BM25 and semantic (should rank highest)
    let a = make_node("transfer wallet balance", Pattern::Do);
    let aid = a.id;
    graph.add_node(a).unwrap();
    graph.set_root(aid).unwrap();

    // Node B: appears only in BM25 (keyword "transfer" present)
    let b = make_node("transfer record to archive", Pattern::Save);
    graph.add_node(b).unwrap();

    let bm25 = Bm25Index::build_from_graph(&graph);
    let bm25_results = bm25.search("transfer", 20, &graph);

    // Only A is in the semantic index; B is absent.
    // This makes A's RRF score = BM25_contribution + semantic_contribution,
    // while B's RRF score = BM25_contribution only — a large, unambiguous gap.
    let mut vectors = HashMap::new();
    vectors.insert(aid, vec![1.0_f32, 0.0]);
    let mut table = HashMap::new();
    table.insert("transfer".to_string(), vec![1.0_f32, 0.0]);
    let provider = LookupProvider::new(2, table);
    let index = EmbeddingIndex::from_vectors(Box::new(provider), vectors);

    let results = hybrid_search("transfer", &bm25_results, Some(&index), &graph, 10)
        .expect("hybrid search should succeed");

    assert!(results.len() >= 2, "both nodes must be in results");
    assert_eq!(
        results[0].node_id, aid,
        "node in both lists must rank first; got node_id {:?}",
        results[0].node_id
    );
    assert_eq!(results[0].source, RankingSource::Both);
}

// ─── Limit test ───────────────────────────────────────────────────────────────

#[test]
fn t102_hybrid_respects_limit() {
    let mut graph = AilGraph::new();
    let limit = 3;

    let mut root_id = NodeId::new();
    for i in 0..6 {
        let n = make_node(&format!("validate payment step {i}"), Pattern::Check);
        let nid = n.id;
        if i == 0 {
            root_id = nid;
        }
        graph.add_node(n).unwrap();
    }
    graph.set_root(root_id).unwrap();

    let bm25 = Bm25Index::build_from_graph(&graph);
    let bm25_results = bm25.search("validate", 20, &graph);

    let results = hybrid_search("validate", &bm25_results, None, &graph, limit)
        .expect("fallback should succeed");

    assert!(
        results.len() <= limit,
        "result count {} must not exceed limit {limit}",
        results.len()
    );
}

// ─── Fallback to BM25 ─────────────────────────────────────────────────────────

#[test]
fn t102_fallback_to_bm25_when_no_embeddings() {
    let mut graph = AilGraph::new();
    let n = make_node("transfer money between wallets", Pattern::Do);
    let nid = n.id;
    graph.add_node(n).unwrap();
    graph.set_root(nid).unwrap();

    let bm25 = Bm25Index::build_from_graph(&graph);
    let bm25_results = bm25.search("transfer", 20, &graph);
    assert!(!bm25_results.is_empty());

    // Pass `embeddings = None` to trigger fallback.
    let results = hybrid_search("transfer", &bm25_results, None, &graph, 10)
        .expect("BM25 fallback should always succeed");

    assert_eq!(results.len(), bm25_results.len());
    for r in &results {
        assert_eq!(
            r.source,
            RankingSource::Bm25Only,
            "all fallback results must have source=Bm25Only"
        );
        assert!(r.bm25_rank.is_some());
        assert_eq!(r.semantic_rank, None);
        assert!(r.rrf_score > 0.0);
    }
    // Order must match BM25 order
    assert_eq!(results[0].node_id, bm25_results[0].node_id);
}
