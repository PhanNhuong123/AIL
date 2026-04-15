//! `ail.context` tool — CIC context packets formatted for AI use.
//!
//! Implements the five-step context algorithm from spec §8:
//! Search → Rank → Expand → Budget → Return.

use ail_graph::{AilGraph, Bm25Index, NodeId};

use crate::types::tool_io::{
    ContextInput, ContextNode, ContextOutput, ContextSummary, ScopeEntry,
};

/// Approximate words in a JSON-serialised [`ContextOutput`], used for budget
/// capping.  We use a rough 1-word = 6-byte heuristic on the serialised form.
fn approx_words(text: &str) -> usize {
    text.split_whitespace().count()
}

const DEFAULT_BUDGET_TOKENS: usize = 4096;
/// 20 BM25 candidates before splitting into primary / secondary.
const CANDIDATE_BUDGET: usize = 20;
/// 70 % of candidates become primary (full CIC packet).
const PRIMARY_FRACTION_NUMERATOR: usize = 7;
const PRIMARY_FRACTION_DENOMINATOR: usize = 10;

/// Compute a context response for the given task description.
///
/// Builds a BM25 index locally (does not use the server search cache; this
/// keeps the context algorithm independent of search state). The caller may
/// pass `budget_tokens` to cap the total output size.
pub(crate) fn run_context(graph: &AilGraph, input: &ContextInput) -> ContextOutput {
    let budget_tokens = input.budget_tokens.unwrap_or(DEFAULT_BUDGET_TOKENS);

    // ── 1. Search ────────────────────────────────────────────────────────────
    let index = Bm25Index::build_from_graph(graph);
    let candidates = index.search(&input.task, CANDIDATE_BUDGET, graph);

    // ── 2. Rank + split ──────────────────────────────────────────────────────
    let n_primary = (candidates.len() * PRIMARY_FRACTION_NUMERATOR)
        .div_ceil(PRIMARY_FRACTION_DENOMINATOR);
    let (primary_raw, secondary_raw) = candidates.split_at(n_primary.min(candidates.len()));

    // ── 3. Expand primary nodes to full CIC packets ──────────────────────────
    let mut primary: Vec<ContextNode> = primary_raw
        .iter()
        .filter_map(|r| expand_node(graph, r.node_id, &r.intent))
        .collect();

    // ── 4. Budget: truncate secondary if total exceeds cap ───────────────────
    let mut secondary: Vec<ContextSummary> = secondary_raw
        .iter()
        .map(|r| ContextSummary {
            node_id: r.node_id.to_string(),
            intent: r.intent.clone(),
        })
        .collect();

    // Rough cap: serialize what we have so far and trim secondary.
    let primary_json =
        serde_json::to_string(&primary).unwrap_or_default();
    let mut used = approx_words(&primary_json);
    secondary.retain(|s| {
        let w = approx_words(&s.intent) + 4; // node_id + fields overhead
        if used + w <= budget_tokens {
            used += w;
            true
        } else {
            false
        }
    });

    // ── 5. Collect contracts from primary packets ────────────────────────────
    let mut contracts: Vec<String> = Vec::new();
    for node in &primary {
        for c in &node.constraints {
            if !contracts.contains(c) {
                contracts.push(c.clone());
            }
        }
    }

    // Apply budget cap to primary if needed (trim from the end).
    while approx_words(&serde_json::to_string(&primary).unwrap_or_default()) > budget_tokens
        && !primary.is_empty()
    {
        primary.pop();
    }

    ContextOutput {
        primary,
        secondary,
        contracts,
    }
}

/// Compute the CIC packet for one node and format it as a [`ContextNode`].
/// Returns `None` if the graph cannot produce the packet.
fn expand_node(graph: &AilGraph, node_id: NodeId, intent: &str) -> Option<ContextNode> {
    let packet = graph.compute_context_packet(node_id).ok()?;

    let scope: Vec<ScopeEntry> = packet
        .scope
        .iter()
        .map(|v| ScopeEntry {
            name: v.name.clone(),
            constraint: v.type_ref.clone(),
        })
        .collect();

    // Flatten all constraint fields into a single list of expression strings.
    let mut constraints: Vec<String> = Vec::new();
    for c in packet
        .inherited_constraints
        .iter()
        .chain(packet.type_constraints.iter())
        .chain(packet.call_contracts.iter())
        .chain(packet.template_constraints.iter())
        .chain(packet.verified_facts.iter())
    {
        constraints.push(format!("{:?} {}", c.kind, c.expression));
    }

    Some(ContextNode {
        node_id: node_id.to_string(),
        intent: intent.to_owned(),
        intent_chain: packet.intent_chain,
        scope,
        constraints,
    })
}
