mod node_text;
mod tree_walk;

use ail_graph::AilGraph;

/// Render an `AilGraph` into deterministic `.ail` text.
///
/// The `depth` parameter controls child expansion:
/// - `0`: only top-level node signatures (no children or contracts)
/// - `N`: expand children up to `N` levels deep
/// - `usize::MAX`: expand everything
///
/// The output always ends with a trailing newline for non-empty graphs.
/// For gate types (`ValidGraph`, `TypedGraph`, `VerifiedGraph`), call `.graph()`
/// to obtain the `&AilGraph` reference.
pub fn render(graph: &AilGraph, depth: usize) -> String {
    let top_ids = tree_walk::collect_top_level_ids(graph);

    if top_ids.is_empty() {
        return String::new();
    }

    let mut out = String::new();

    for (i, &id) in top_ids.iter().enumerate() {
        if i > 0 {
            out.push('\n'); // blank line between top-level nodes
        }
        tree_walk::render_node(graph, id, 0, depth, &mut out);
    }

    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }

    out
}
