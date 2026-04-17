use std::collections::HashMap;

use petgraph::stable_graph::{NodeIndex, StableDiGraph};

use crate::errors::GraphError;
use crate::search::Bm25Index;
use crate::types::{EdgeId, EdgeKind, Node, NodeId};

/// The core PSSD graph.
///
/// Wraps a `petgraph::StableDiGraph` keyed by [`NodeId`] (UUID). Maintains an
/// internal index map so callers always use stable [`NodeId`]s rather than raw
/// petgraph indices.
///
/// `Clone` is derived so in-memory callers (e.g. the MCP `ail.batch` handler)
/// can snapshot the graph before a batch of mutations and restore on failure.
#[derive(Clone)]
pub struct AilGraph {
    inner: StableDiGraph<Node, EdgeKind>,
    node_index_map: HashMap<NodeId, NodeIndex>,
    root_id: Option<NodeId>,
}

impl AilGraph {
    /// Create an empty graph with no nodes, no edges, and no root.
    pub fn new() -> Self {
        Self {
            inner: StableDiGraph::new(),
            node_index_map: HashMap::new(),
            root_id: None,
        }
    }

    // ─── CRUD ──────────────────────────────────────────────────────────────

    /// Add a node to the graph. Returns the node's id.
    ///
    /// Errors if a node with the same `id` already exists.
    pub fn add_node(&mut self, node: Node) -> Result<NodeId, GraphError> {
        let id = node.id;
        if self.node_index_map.contains_key(&id) {
            return Err(GraphError::DuplicateNodeId(id));
        }
        let nx = self.inner.add_node(node);
        self.node_index_map.insert(id, nx);
        Ok(id)
    }

    /// Remove a node and all its incident edges from the graph.
    ///
    /// Errors if the node does not exist.
    pub fn remove_node(&mut self, node_id: NodeId) -> Result<Node, GraphError> {
        let nx = self.resolve_node_index(node_id)?;
        self.node_index_map.remove(&node_id);
        if self.root_id == Some(node_id) {
            self.root_id = None;
        }
        // petgraph removes incident edges automatically
        self.inner
            .remove_node(nx)
            .ok_or(GraphError::NodeNotFound(node_id))
    }

    /// Return a shared reference to a node.
    ///
    /// Errors if the node does not exist.
    pub fn get_node(&self, node_id: NodeId) -> Result<&Node, GraphError> {
        let nx = self.resolve_node_index(node_id)?;
        self.inner
            .node_weight(nx)
            .ok_or(GraphError::NodeNotFound(node_id))
    }

    /// Return a mutable reference to a node.
    ///
    /// Errors if the node does not exist.
    pub fn get_node_mut(&mut self, node_id: NodeId) -> Result<&mut Node, GraphError> {
        let nx = self.resolve_node_index(node_id)?;
        self.inner
            .node_weight_mut(nx)
            .ok_or(GraphError::NodeNotFound(node_id))
    }

    /// Add a directed edge `from → to` with the given [`EdgeKind`].
    ///
    /// Returns an [`EdgeId`] that can be passed to [`AilGraph::remove_edge`].
    /// Errors if either endpoint does not exist.
    pub fn add_edge(
        &mut self,
        from: NodeId,
        to: NodeId,
        kind: EdgeKind,
    ) -> Result<EdgeId, GraphError> {
        let from_nx = self.resolve_node_index(from)?;
        let to_nx = self.resolve_node_index(to)?;
        let ex = self.inner.add_edge(from_nx, to_nx, kind);
        Ok(EdgeId::new(ex))
    }

    /// Remove an edge by its [`EdgeId`].
    ///
    /// Errors if the edge no longer exists (already removed or id is stale).
    pub fn remove_edge(&mut self, edge_id: EdgeId) -> Result<(), GraphError> {
        self.inner
            .remove_edge(edge_id.index())
            .map(|_| ())
            .ok_or(GraphError::EdgeNotFound(edge_id))
    }

    // ─── Root ──────────────────────────────────────────────────────────────

    /// Designate `node_id` as the graph root.
    ///
    /// Errors if the node does not exist.
    pub fn set_root(&mut self, node_id: NodeId) -> Result<(), GraphError> {
        // verify the node exists before storing
        self.resolve_node_index(node_id)?;
        self.root_id = Some(node_id);
        Ok(())
    }

    /// Return the root node id, if one has been set.
    pub fn root_id(&self) -> Option<NodeId> {
        self.root_id
    }

    // ─── Counts ────────────────────────────────────────────────────────────

    pub fn node_count(&self) -> usize {
        self.inner.node_count()
    }

    pub fn edge_count(&self) -> usize {
        self.inner.edge_count()
    }

    // ─── Iteration ─────────────────────────────────────────────────────────

    /// Returns an iterator over all [`NodeId`]s currently in the graph.
    pub fn node_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.node_index_map.keys().copied()
    }

    /// Returns an iterator over shared references to all nodes currently in the graph.
    pub fn all_nodes(&self) -> impl Iterator<Item = &Node> + '_ {
        self.inner.node_weights()
    }

    // ─── Search ────────────────────────────────────────────────────────────

    /// Build a BM25 search index over all nodes in this graph.
    ///
    /// The index is a snapshot of the current graph state. Rebuild after any
    /// mutation (add/remove node, or changes to intent/name).
    pub fn build_search_index(&self) -> Bm25Index {
        Bm25Index::build_from_graph(self)
    }

    // ─── Internal helpers ──────────────────────────────────────────────────

    /// Resolve a [`NodeId`] to its petgraph [`NodeIndex`].
    pub(crate) fn resolve_node_index(&self, node_id: NodeId) -> Result<NodeIndex, GraphError> {
        self.node_index_map
            .get(&node_id)
            .copied()
            .ok_or(GraphError::NodeNotFound(node_id))
    }

    /// Return a reference to the underlying petgraph for navigation use.
    pub(crate) fn inner(&self) -> &StableDiGraph<Node, EdgeKind> {
        &self.inner
    }

    /// Return a mutable reference to the underlying petgraph.
    ///
    /// Used by [`super::backend_impl`] for edge removal by kind, which requires
    /// a petgraph edge index not exposed through the public [`EdgeId`] API.
    pub(crate) fn inner_mut(&mut self) -> &mut StableDiGraph<Node, EdgeKind> {
        &mut self.inner
    }
}

impl Default for AilGraph {
    fn default() -> Self {
        Self::new()
    }
}
