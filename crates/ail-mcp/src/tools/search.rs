//! `ail.search` tool — BM25 semantic search with a lazy cached index.

use std::cell::RefCell;

use ail_graph::{AilGraph, Bm25Index};

use crate::types::tool_io::{SearchInput, SearchItem, SearchOutput};

const DEFAULT_BUDGET: usize = 10;

/// Execute a BM25 search against the project graph.
///
/// The `Bm25Index` is built lazily on the first call and cached in `cache`.
/// Callers must clear the cache whenever the underlying graph changes.
pub(crate) fn run_search(
    graph: &AilGraph,
    cache: &RefCell<Option<Bm25Index>>,
    input: &SearchInput,
) -> SearchOutput {
    // Build or reuse the index.
    let mut borrow = cache.borrow_mut();
    if borrow.is_none() {
        *borrow = Some(Bm25Index::build_from_graph(graph));
    }
    let index = borrow.as_ref().unwrap(); // safe: just set above

    let budget = input.budget.unwrap_or(DEFAULT_BUDGET);
    let raw = index.search(&input.query, budget, graph);

    let results: Vec<SearchItem> = raw
        .into_iter()
        .map(|r| SearchItem {
            node_id: r.node_id.to_string(),
            score: r.score,
            intent: r.intent,
            pattern: format!("{:?}", r.pattern),
            path: r.path,
        })
        .collect();

    let total = results.len();
    SearchOutput { results, total }
}
