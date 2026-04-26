//! Subgraph scoping for Čech nerve filtering.
//!
//! `filter_to_subtree` restricts a [`CechNerve`] to the subtree rooted at a
//! given node, using BFS over `Ev` children from [`GraphBackend::children`].
//!
//! Invariant 17.3-C: overlaps are retained only when **both** `node_a` and
//! `node_b` are within the collected subtree. This preserves the guarantee that
//! every overlap endpoint resolves to a section in the filtered nerve, which
//! the Phase 17.4 IDE consumer relies on for node navigation.

use std::collections::{HashSet, VecDeque};

use ail_graph::{GraphBackend, NodeId};

use super::types::CechNerve;

/// Collect the set of nodes in the subtree rooted at `root` via BFS over
/// `graph.children()`. `root` is always included, even if it has no children.
///
/// Uses a visited set to defend against any graph cycles. Errors from
/// `graph.children()` are skipped (invariant 17.1-E skip-on-error pattern).
fn collect_subtree(graph: &dyn GraphBackend, root: NodeId) -> HashSet<NodeId> {
    let mut in_scope: HashSet<NodeId> = HashSet::new();
    let mut queue: VecDeque<NodeId> = VecDeque::from([root]);

    while let Some(id) = queue.pop_front() {
        if !in_scope.insert(id) {
            // Already visited — dedupe and defend against cycles.
            continue;
        }
        match graph.children(id) {
            Ok(kids) => {
                for kid in kids {
                    queue.push_back(kid);
                }
            }
            Err(_) => {
                // Skip-on-error per invariant 17.1-E.
                continue;
            }
        }
    }

    in_scope
}

/// Return a new [`CechNerve`] containing only the sections and overlaps that
/// fall within the subtree rooted at `root`.
///
/// - Sections are kept when `section.node_id` is in scope.
/// - Overlaps are kept when **both** `overlap.node_a` and `overlap.node_b` are
///   in scope (invariant 17.3-C).
///
/// If `root` is not present in the graph (i.e. `graph.children(root)` returns
/// an error and the BFS yields only `root` with no children — or root is
/// genuinely absent from sections), the returned nerve reflects reality: the
/// sections list will be empty if no section carries that `node_id`.
pub fn filter_to_subtree(nerve: &CechNerve, root: NodeId, graph: &dyn GraphBackend) -> CechNerve {
    let in_scope = collect_subtree(graph, root);

    let sections = nerve
        .sections
        .iter()
        .filter(|s| in_scope.contains(&s.node_id))
        .cloned()
        .collect();

    let overlaps = nerve
        .overlaps
        .iter()
        .filter(|o| in_scope.contains(&o.node_a) && in_scope.contains(&o.node_b))
        .cloned()
        .collect();

    CechNerve { sections, overlaps }
}
