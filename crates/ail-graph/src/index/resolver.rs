use std::collections::HashMap;

use crate::errors::GraphError;
use crate::graph::AilGraph;
use crate::types::NodeId;

use super::entry::IndexEntry;
use super::folder_index::FolderIndex;
use super::generator::generate_folder_index_for_node;

/// Resolves names by walking up the Ev-parent chain from a starting node.
///
/// The resolution order follows the AIL spec (§5.3):
/// 1. Search the containing folder's index.
/// 2. If not found, search the parent folder's index.
/// 3. Repeat until root.
/// 4. If not found at root → [`GraphError::NameNotFound`].
/// 5. If the same name appears more than once at the same scope level →
///    [`GraphError::AmbiguousName`].
///
/// Folder indices are cached on first generation and reused on subsequent lookups.
/// Build one `NameResolver` per query session, then call [`NameResolver::resolve_name`]
/// repeatedly for O(1) per-folder cost after the first lookup.
pub struct NameResolver<'g> {
    graph: &'g AilGraph,
    cache: HashMap<NodeId, FolderIndex>,
}

impl<'g> NameResolver<'g> {
    /// Create a new resolver backed by `graph`.
    pub fn new(graph: &'g AilGraph) -> Self {
        Self {
            graph,
            cache: HashMap::new(),
        }
    }

    /// Resolve `name` starting from `start_node_id`.
    ///
    /// The search begins in the folder that contains `start_node_id` (its Ev parent,
    /// or the root if it has no parent), then walks upward until found or exhausted.
    pub fn resolve_name(
        &mut self,
        name: &str,
        start_node_id: NodeId,
    ) -> Result<IndexEntry, GraphError> {
        let folder_id = self.find_containing_folder(start_node_id);
        self.resolve_from_folder(name, folder_id)
    }

    /// The "containing folder" of a node is its Ev parent.
    ///
    /// If the node has no parent (it is the root), it is its own folder.
    fn find_containing_folder(&self, node_id: NodeId) -> NodeId {
        self.graph
            .parent_of(node_id)
            .ok()
            .flatten()
            .unwrap_or(node_id)
    }

    /// Return a reference to the cached folder index, generating it if absent.
    fn folder_index_for(&mut self, folder_id: NodeId) -> &FolderIndex {
        self.cache
            .entry(folder_id)
            .or_insert_with(|| generate_folder_index_for_node(folder_id, self.graph))
    }

    fn resolve_from_folder(
        &mut self,
        name: &str,
        folder_id: NodeId,
    ) -> Result<IndexEntry, GraphError> {
        let matches: Vec<IndexEntry> = self
            .folder_index_for(folder_id)
            .entries
            .iter()
            .filter(|e| e.name == name)
            .cloned()
            .collect();

        match matches.len() {
            1 => Ok(matches
                .into_iter()
                .next()
                .expect("matches.len() == 1 guarantees one element")),
            0 => match self.graph.parent_of(folder_id) {
                Ok(Some(parent_id)) => self.resolve_from_folder(name, parent_id),
                Ok(None) | Err(_) => Err(GraphError::NameNotFound(name.to_string())),
            },
            _ => {
                let locations = matches.iter().map(|e| e.node_id.to_string()).collect();
                Err(GraphError::AmbiguousName {
                    name: name.to_string(),
                    locations,
                })
            }
        }
    }
}

// ─── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::GraphError;
    use crate::types::{EdgeKind, Node, NodeId, Pattern};
    use crate::AilGraph;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn make_named_node(id: NodeId, pattern: Pattern, name: &str) -> Node {
        let mut n = Node::new(id, "intent", pattern);
        n.metadata.name = Some(name.to_string());
        n
    }

    /// Build:
    /// ```
    /// root (Describe, unnamed)
    ///   ├── wallet (Describe, named)
    ///   │   ├── Amount (Define, named)
    ///   │   └── Invoice (Describe, named)
    ///   └── billing (Describe, named)
    ///       └── Amount (Define, named)   ← same name — ambiguous at root scope
    /// ```
    fn make_three_level_graph() -> (AilGraph, NodeId, NodeId, NodeId, NodeId, NodeId, NodeId) {
        let mut graph = AilGraph::new();
        let root_id = NodeId::new();
        let wallet_id = NodeId::new();
        let amount_w_id = NodeId::new();
        let invoice_id = NodeId::new();
        let billing_id = NodeId::new();
        let amount_b_id = NodeId::new();

        // root — unnamed structural node (no parent → is its own folder)
        let root = Node::new(root_id, "root", Pattern::Describe);
        graph.add_node(root).unwrap();
        graph.set_root(root_id).unwrap();

        // wallet module
        let wallet = make_named_node(wallet_id, Pattern::Describe, "wallet");
        graph.add_node(wallet).unwrap();
        graph.add_edge(root_id, wallet_id, EdgeKind::Ev).unwrap();

        // wallet.Amount
        let mut amount_w = make_named_node(amount_w_id, Pattern::Define, "Amount");
        amount_w.metadata.base_type = Some("number".to_string());
        graph.add_node(amount_w).unwrap();
        graph.add_edge(wallet_id, amount_w_id, EdgeKind::Ev).unwrap();

        // wallet.Invoice
        let invoice = make_named_node(invoice_id, Pattern::Describe, "Invoice");
        graph.add_node(invoice).unwrap();
        graph.add_edge(wallet_id, invoice_id, EdgeKind::Ev).unwrap();

        // billing module
        let billing = make_named_node(billing_id, Pattern::Describe, "billing");
        graph.add_node(billing).unwrap();
        graph.add_edge(root_id, billing_id, EdgeKind::Ev).unwrap();

        // billing.Amount — same name as wallet.Amount
        let mut amount_b = make_named_node(amount_b_id, Pattern::Define, "Amount");
        amount_b.metadata.base_type = Some("integer".to_string());
        graph.add_node(amount_b).unwrap();
        graph.add_edge(billing_id, amount_b_id, EdgeKind::Ev).unwrap();

        (
            graph, root_id, wallet_id, amount_w_id, invoice_id, billing_id, amount_b_id,
        )
    }

    // ── tests ─────────────────────────────────────────────────────────────────

    #[test]
    fn index014_resolve_name_found_in_local_folder() {
        // Amount is a direct sibling of Invoice inside wallet.
        // Resolving "Amount" starting from invoice_id → containing folder is wallet_id.
        let (graph, _root, _wallet, amount_w_id, invoice_id, _billing, _amount_b) =
            make_three_level_graph();

        let mut resolver = NameResolver::new(&graph);
        let entry = resolver.resolve_name("Amount", invoice_id).unwrap();
        assert_eq!(entry.name, "Amount");
        // Should resolve to wallet.Amount (same folder scope)
        assert_eq!(entry.node_id, amount_w_id);
    }

    #[test]
    fn index015_resolve_name_found_in_parent_folder() {
        // Invoice is defined in wallet. Starting from amount_b_id inside billing,
        // "Invoice" is NOT in billing → walk up to root → root subtree has wallet.Invoice.
        let (graph, _root, _wallet, _amount_w, invoice_id, _billing, amount_b_id) =
            make_three_level_graph();

        let mut resolver = NameResolver::new(&graph);
        let entry = resolver.resolve_name("Invoice", amount_b_id).unwrap();
        assert_eq!(entry.name, "Invoice");
        assert_eq!(entry.node_id, invoice_id);
    }

    #[test]
    fn index016_resolve_name_not_found_returns_error() {
        let (graph, _root, _wallet, _amount_w, invoice_id, _billing, _amount_b) =
            make_three_level_graph();

        let mut resolver = NameResolver::new(&graph);
        let result = resolver.resolve_name("NonExistent", invoice_id);
        assert!(
            matches!(result, Err(GraphError::NameNotFound(ref n)) if n == "NonExistent"),
            "expected NameNotFound, got {result:?}"
        );
    }

    #[test]
    fn index017_resolve_name_ambiguous_returns_error() {
        // "Amount" exists in both wallet and billing.
        // Starting from root_id (no parent) → root is its own folder.
        // root's subtree collects BOTH Amount declarations → AmbiguousName.
        let (graph, root_id, _wallet, _amount_w, _invoice, _billing, _amount_b) =
            make_three_level_graph();

        let mut resolver = NameResolver::new(&graph);
        let result = resolver.resolve_name("Amount", root_id);
        assert!(
            matches!(result, Err(GraphError::AmbiguousName { ref name, .. }) if name == "Amount"),
            "expected AmbiguousName, got {result:?}"
        );
    }

    #[test]
    fn index014b_resolver_caches_folder_index() {
        // Resolve the same name twice — second call must hit cache (same result, no panic).
        let (graph, _root, _wallet, amount_w_id, invoice_id, _billing, _amount_b) =
            make_three_level_graph();

        let mut resolver = NameResolver::new(&graph);
        let first = resolver.resolve_name("Amount", invoice_id).unwrap();
        let second = resolver.resolve_name("Amount", invoice_id).unwrap();
        assert_eq!(first.node_id, second.node_id);
        assert_eq!(first.node_id, amount_w_id);
    }
}
