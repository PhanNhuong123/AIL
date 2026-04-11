use std::collections::HashMap;

use crate::graph::AilGraph;
use crate::types::{Node, NodeId, Pattern};

use super::result::SearchResult;

// ─── BM25 tuning constants ─────────────────────────────────────────────────

/// Term-frequency saturation parameter.
const BM25_K1: f32 = 1.2;
/// Document-length normalisation parameter.
const BM25_B: f32 = 0.75;

/// How many times each field's tokens are repeated in the virtual document.
/// Higher weight = more influence on ranking.
const NAME_WEIGHT: usize = 3;
const INTENT_WEIGHT: usize = 2;
const PATTERN_WEIGHT: usize = 1;

// ─── Stopwords ─────────────────────────────────────────────────────────────

/// Returns `true` for common English function words that carry no signal.
/// AIL pattern names (fetch, save, check …) are intentionally excluded so
/// users can search by pattern.
fn is_stopword(term: &str) -> bool {
    matches!(
        term,
        "a" | "an"
            | "the"
            | "is"
            | "are"
            | "was"
            | "were"
            | "be"
            | "been"
            | "to"
            | "from"
            | "in"
            | "on"
            | "at"
            | "with"
            | "for"
            | "of"
            | "and"
            | "or"
            | "it"
            | "its"
            | "this"
            | "that"
    )
}

// ─── Tokeniser ─────────────────────────────────────────────────────────────

/// Tokenise `text` into lowercase terms.
///
/// Steps:
/// 1. Insert a space before each uppercase letter that follows a lowercase one
///    (CamelCase → separate words).
/// 2. Lowercase the result.
/// 3. Split on whitespace, `_`, and `-`.
/// 4. Drop empty tokens and stopwords.
fn tokenize(text: &str) -> Vec<String> {
    // Step 1: CamelCase split
    let mut spaced = String::with_capacity(text.len() + 8);
    let chars: Vec<char> = text.chars().collect();
    for (i, &ch) in chars.iter().enumerate() {
        if i > 0 && ch.is_uppercase() && chars[i - 1].is_lowercase() {
            spaced.push(' ');
        }
        spaced.push(ch);
    }

    // Steps 2–4
    spaced
        .to_lowercase()
        .split(|c: char| c.is_whitespace() || c == '_' || c == '-')
        .filter(|t| !t.is_empty() && !is_stopword(t))
        .map(|t| t.to_string())
        .collect()
}

// ─── Pattern → string ──────────────────────────────────────────────────────

fn pattern_label(pattern: &Pattern) -> &'static str {
    match pattern {
        Pattern::Define => "define",
        Pattern::Describe => "describe",
        Pattern::Error => "error",
        Pattern::Do => "do",
        Pattern::Promise => "promise",
        Pattern::Let => "let",
        Pattern::Check => "check",
        Pattern::ForEach => "foreach",
        Pattern::Match => "match",
        Pattern::Fetch => "fetch",
        Pattern::Save => "save",
        Pattern::Update => "update",
        Pattern::Remove => "remove",
        Pattern::Return => "return",
        Pattern::Raise => "raise",
        Pattern::Together => "together",
        Pattern::Retry => "retry",
    }
}

// ─── Document builder ──────────────────────────────────────────────────────

/// Build the weighted token bag for a single node.
///
/// Tokens from higher-weight fields are repeated, which shifts BM25 TF
/// without requiring per-field scoring.
fn build_document_tokens(node: &Node) -> Vec<String> {
    let mut tokens = Vec::new();

    // name ×3
    if let Some(name) = &node.metadata.name {
        for _ in 0..NAME_WEIGHT {
            tokens.extend(tokenize(name));
        }
    }

    // intent ×2
    for _ in 0..INTENT_WEIGHT {
        tokens.extend(tokenize(&node.intent));
    }

    // pattern ×1
    for _ in 0..PATTERN_WEIGHT {
        tokens.extend(tokenize(pattern_label(&node.pattern)));
    }

    tokens
}

// ─── Index ─────────────────────────────────────────────────────────────────

/// A BM25 full-text search index over [`AilGraph`] nodes.
///
/// Build with [`Bm25Index::build_from_graph`] and rebuild whenever the graph
/// is mutated — at v0.1 scale (< 1 000 nodes) a full rebuild is cheap.
pub struct Bm25Index {
    /// Inverted index: term → list of `(NodeId, raw_term_freq)`.
    inverted: HashMap<String, Vec<(NodeId, usize)>>,
    /// Per-node document length (total token count after weighting).
    doc_lengths: HashMap<NodeId, usize>,
    /// Total number of indexed nodes.
    doc_count: usize,
    /// Average document length across all nodes.
    avg_doc_length: f32,
}

impl Bm25Index {
    /// Index every node in `graph`.
    ///
    /// The index reflects the graph state at call time. Rebuild after any
    /// structural change (add/remove node or mutation of intent/name).
    pub fn build_from_graph(graph: &AilGraph) -> Self {
        // term → { NodeId → freq }
        let mut freq_map: HashMap<String, HashMap<NodeId, usize>> = HashMap::new();
        let mut doc_lengths: HashMap<NodeId, usize> = HashMap::new();

        for nx in graph.inner().node_indices() {
            let node = graph
                .inner()
                .node_weight(nx)
                // Safe: nx comes directly from node_indices()
                .expect("node weight must exist for valid index");

            let tokens = build_document_tokens(node);
            doc_lengths.insert(node.id, tokens.len());

            for token in tokens {
                *freq_map.entry(token).or_default().entry(node.id).or_insert(0) += 1;
            }
        }

        let doc_count = doc_lengths.len();
        let avg_doc_length = if doc_count == 0 {
            0.0
        } else {
            doc_lengths.values().sum::<usize>() as f32 / doc_count as f32
        };

        let inverted = freq_map
            .into_iter()
            .map(|(term, node_map)| (term, node_map.into_iter().collect()))
            .collect();

        Bm25Index {
            inverted,
            doc_lengths,
            doc_count,
            avg_doc_length,
        }
    }

    /// Return up to `budget` nodes ranked by BM25 relevance for `query`.
    ///
    /// `graph` is used only to build [`SearchResult`] fields (intent, name,
    /// path) — no re-indexing occurs.
    pub fn search(&self, query: &str, budget: usize, graph: &AilGraph) -> Vec<SearchResult> {
        if self.doc_count == 0 || budget == 0 {
            return Vec::new();
        }

        let query_terms = tokenize(query);
        if query_terms.is_empty() {
            return Vec::new();
        }

        let avg_dl = if self.avg_doc_length > 0.0 {
            self.avg_doc_length
        } else {
            1.0
        };
        let n = self.doc_count as f32;

        let mut scores: HashMap<NodeId, f32> = HashMap::new();

        for term in &query_terms {
            let Some(postings) = self.inverted.get(term) else {
                continue;
            };

            let df = postings.len() as f32;
            let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();

            for &(node_id, freq) in postings {
                let dl = *self.doc_lengths.get(&node_id).unwrap_or(&0) as f32;
                let tf = (freq as f32 * (BM25_K1 + 1.0))
                    / (freq as f32 + BM25_K1 * (1.0 - BM25_B + BM25_B * dl / avg_dl));
                *scores.entry(node_id).or_insert(0.0) += idf * tf;
            }
        }

        let mut ranked: Vec<(NodeId, f32)> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked.truncate(budget);

        ranked
            .into_iter()
            .filter_map(|(node_id, score)| build_search_result(graph, node_id, score))
            .collect()
    }
}

// ─── Result builder ────────────────────────────────────────────────────────

/// Walk up the Ev parent chain to compute `path` and `depth`, then assemble
/// the full [`SearchResult`].
fn build_search_result(graph: &AilGraph, node_id: NodeId, score: f32) -> Option<SearchResult> {
    let node = graph.get_node(node_id).ok()?;

    let mut path: Vec<String> = Vec::new();
    let mut current_id = node_id;

    loop {
        let current = graph.get_node(current_id).ok()?;
        let label = current
            .metadata
            .name
            .clone()
            .unwrap_or_else(|| first_word(&current.intent));
        path.push(label);

        match graph.parent_of(current_id) {
            Ok(Some(parent_id)) => current_id = parent_id,
            _ => break,
        }
    }
    path.reverse();
    let depth = path.len().saturating_sub(1);

    Some(SearchResult {
        node_id,
        score,
        intent: node.intent.clone(),
        name: node.metadata.name.clone(),
        pattern: node.pattern.clone(),
        depth,
        path,
    })
}

fn first_word(s: &str) -> String {
    s.split_whitespace()
        .next()
        .unwrap_or(s)
        .to_string()
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::AilGraph;
    use crate::types::{EdgeKind, NodeMetadata};

    fn make_node(intent: &str, pattern: Pattern, name: Option<&str>) -> Node {
        Node {
            id: NodeId::new(),
            intent: intent.to_string(),
            pattern,
            children: None,
            expression: None,
            contracts: vec![],
            metadata: NodeMetadata {
                name: name.map(|s| s.to_string()),
                ..Default::default()
            },
        }
    }

    // ── tokeniser ──────────────────────────────────────────────────────────

    #[test]
    fn bm25_tokenize_splits_camel_case() {
        let tokens = tokenize("WalletBalance");
        assert!(tokens.contains(&"wallet".to_string()));
        assert!(tokens.contains(&"balance".to_string()));
    }

    #[test]
    fn bm25_tokenize_splits_snake_case() {
        let tokens = tokenize("wallet_balance");
        assert_eq!(tokens, vec!["wallet", "balance"]);
    }

    #[test]
    fn bm25_tokenize_filters_stopwords() {
        let tokens = tokenize("save a wallet to the database");
        assert!(!tokens.contains(&"a".to_string()));
        assert!(!tokens.contains(&"to".to_string()));
        assert!(!tokens.contains(&"the".to_string()));
        assert!(tokens.contains(&"save".to_string()));
        assert!(tokens.contains(&"wallet".to_string()));
        assert!(tokens.contains(&"database".to_string()));
    }

    // ── single-node search ─────────────────────────────────────────────────

    #[test]
    fn bm25_search_finds_node_by_intent_term() {
        let mut graph = AilGraph::new();
        let node = make_node("deduct wallet balance", Pattern::Do, None);
        let nid = graph.add_node(node).unwrap();
        graph.set_root(nid).unwrap();

        let index = Bm25Index::build_from_graph(&graph);
        let results = index.search("deduct", 5, &graph);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].node_id, nid);
    }

    #[test]
    fn bm25_search_finds_node_by_name() {
        let mut graph = AilGraph::new();
        let node = make_node("Some unrelated intent text", Pattern::Do, Some("deduct_funds"));
        let nid = graph.add_node(node).unwrap();
        graph.set_root(nid).unwrap();

        let index = Bm25Index::build_from_graph(&graph);
        let results = index.search("deduct", 5, &graph);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].node_id, nid);
        assert_eq!(results[0].name, Some("deduct_funds".to_string()));
    }

    // ── ranking ────────────────────────────────────────────────────────────

    #[test]
    fn bm25_search_ranks_most_relevant_first() {
        let mut graph = AilGraph::new();
        // Node A: "payment" appears once
        let a = make_node("payment service", Pattern::Do, None);
        // Node B: "payment" appears twice (higher TF → higher score)
        let b = make_node("payment payment gateway", Pattern::Fetch, None);
        // Node C: unrelated
        let c = make_node("user authentication", Pattern::Check, None);

        let aid = graph.add_node(a).unwrap();
        let bid = graph.add_node(b).unwrap();
        let cid = graph.add_node(c).unwrap();
        graph.set_root(aid).unwrap();

        let index = Bm25Index::build_from_graph(&graph);
        let results = index.search("payment", 5, &graph);

        assert_eq!(results.len(), 2);
        // B has higher TF for "payment" → should be ranked first
        assert_eq!(results[0].node_id, bid);
        // C should not appear
        assert!(!results.iter().any(|r| r.node_id == cid));
    }

    #[test]
    fn bm25_search_multiword_query_ranks_higher_match_first() {
        let mut graph = AilGraph::new();
        // Node A matches all 3 query terms
        let a = make_node("wallet balance check operation", Pattern::Check, None);
        // Node C matches only 1 query term
        let c = make_node("save wallet record", Pattern::Save, None);

        let aid = graph.add_node(a).unwrap();
        let cid = graph.add_node(c).unwrap();
        graph.set_root(aid).unwrap();

        let index = Bm25Index::build_from_graph(&graph);
        let results = index.search("wallet balance check", 5, &graph);

        assert!(results.len() >= 2);
        assert_eq!(results[0].node_id, aid);
        assert_eq!(results[1].node_id, cid);
    }

    // ── budget + no-match ──────────────────────────────────────────────────

    #[test]
    fn bm25_search_respects_budget() {
        let mut graph = AilGraph::new();
        for i in 0..5 {
            let n = make_node(&format!("validate payment step {i}"), Pattern::Check, None);
            let nid = graph.add_node(n).unwrap();
            if i == 0 {
                graph.set_root(nid).unwrap();
            }
        }

        let index = Bm25Index::build_from_graph(&graph);
        let results = index.search("validate", 2, &graph);

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn bm25_search_returns_empty_for_no_match() {
        let mut graph = AilGraph::new();
        let n = make_node("transfer money between accounts", Pattern::Do, None);
        let nid = graph.add_node(n).unwrap();
        graph.set_root(nid).unwrap();

        let index = Bm25Index::build_from_graph(&graph);
        let results = index.search("xyzzy", 5, &graph);

        assert!(results.is_empty());
    }

    // ── depth + path ───────────────────────────────────────────────────────

    #[test]
    fn bm25_search_result_includes_depth_and_path() {
        let mut graph = AilGraph::new();
        let parent = make_node("Transfer money", Pattern::Do, Some("transfer_money"));
        let child = make_node("Check balance condition", Pattern::Check, Some("check_balance"));

        let pid = graph.add_node(parent).unwrap();
        let cid = graph.add_node(child).unwrap();
        graph.add_edge(pid, cid, EdgeKind::Ev).unwrap();
        graph.set_root(pid).unwrap();

        let index = Bm25Index::build_from_graph(&graph);
        let results = index.search("balance condition", 5, &graph);

        let hit = results.iter().find(|r| r.node_id == cid).expect("child should match");
        assert_eq!(hit.depth, 1);
        assert_eq!(
            hit.path,
            vec!["transfer_money".to_string(), "check_balance".to_string()]
        );
    }

    // ── sample wallet project ──────────────────────────────────────────────

    #[test]
    fn bm25_search_against_sample_wallet_project() {
        let mut graph = AilGraph::new();

        // Root function
        let root = make_node("Transfer money between wallets", Pattern::Do, Some("transfer_money"));
        let root_id = graph.add_node(root).unwrap();
        graph.set_root(root_id).unwrap();

        // Children
        let nodes: &[(&str, Pattern, &str)] = &[
            ("Validate sender has sufficient balance", Pattern::Check, "validate_sender"),
            ("Check wallet balance exceeds transfer amount", Pattern::Check, "check_balance"),
            ("Save transaction record to database", Pattern::Save, "save_transaction"),
            ("Update sender wallet balance after deduction", Pattern::Update, "update_sender"),
            ("Update receiver wallet balance after addition", Pattern::Update, "update_receiver"),
        ];

        for (intent, pattern, name) in nodes {
            let n = make_node(intent, pattern.clone(), Some(name));
            let nid = graph.add_node(n).unwrap();
            graph.add_edge(root_id, nid, EdgeKind::Ev).unwrap();
        }

        // Type and error nodes (not connected for simplicity)
        let type_node = make_node("Defines wallet balance value type", Pattern::Define, Some("WalletBalance"));
        graph.add_node(type_node).unwrap();

        let err_node = make_node("Error raised when wallet has insufficient funds", Pattern::Error, Some("InsufficientFundsError"));
        graph.add_node(err_node).unwrap();

        let index = Bm25Index::build_from_graph(&graph);

        // Query 1: "transfer money" → root should rank first
        let r1 = index.search("transfer money", 3, &graph);
        assert!(!r1.is_empty());
        assert_eq!(r1[0].node_id, root_id);

        // Query 2: "insufficient funds" → error node should appear
        let r2 = index.search("insufficient funds", 3, &graph);
        assert!(!r2.is_empty());
        assert_eq!(r2[0].name, Some("InsufficientFundsError".to_string()));

        // Query 3: "wallet balance" → check_balance or WalletBalance should top
        let r3 = index.search("wallet balance", 3, &graph);
        assert!(!r3.is_empty());
        let top_names: Vec<_> = r3.iter().filter_map(|r| r.name.as_deref()).collect();
        assert!(
            top_names.contains(&"WalletBalance")
                || top_names.contains(&"check_balance")
                || top_names.contains(&"validate_sender"),
            "expected wallet/balance related node, got: {top_names:?}"
        );
    }
}
