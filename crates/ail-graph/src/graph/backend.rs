use crate::errors::GraphError;
use crate::types::{Contract, EdgeKind, Node, NodeId, Pattern};

/// Abstract graph interface shared by the in-memory [`crate::graph::AilGraph`]
/// and the future `SqliteGraph` in `ail-db`.
///
/// Downstream crates accept `&dyn GraphBackend` (or `&mut dyn GraphBackend`)
/// so that pipeline stages work with either storage backend without code
/// duplication.
///
/// ## Design decisions
///
/// - **Owned nodes on read** (`Option<Node>`): `get_node` returns a cloned
///   `Node` rather than `&Node`. SQLite cannot return a borrow to data it
///   deserialised on the fly; cloning is negligible at our scale.
/// - **Object-safe transactions**: transactions are split into
///   `begin_transaction`, `commit_transaction`, and `rollback_transaction`
///   instead of a generic closure form, so the trait remains object-safe
///   (`dyn GraphBackend` is valid).
/// - **`all_node_ids` not `all_nodes`**: named to avoid shadowing AilGraph's
///   existing `all_nodes() -> impl Iterator<Item = &Node>` used throughout
///   the v1.0 codebase.
pub trait GraphBackend: Send + Sync {
    // === Node Operations ===

    /// Insert a node. Returns the node's [`NodeId`]. Errors on duplicate id.
    fn add_node(&mut self, node: Node) -> Result<NodeId, GraphError>;

    /// Return a clone of the node, or `None` if no node with that id exists.
    fn get_node(&self, id: NodeId) -> Result<Option<Node>, GraphError>;

    /// Replace the stored node at `id` with `node`. Errors if `id` not found.
    fn update_node(&mut self, id: NodeId, node: Node) -> Result<(), GraphError>;

    /// Remove a node and all its incident edges. Errors if `id` not found.
    fn remove_node(&mut self, id: NodeId) -> Result<(), GraphError>;

    /// Return all node ids currently in the graph.
    fn all_node_ids(&self) -> Result<Vec<NodeId>, GraphError>;

    /// Number of nodes in the graph.
    fn node_count(&self) -> usize;

    // === Edge Operations ===

    /// Add a directed edge `from → to` with the given [`EdgeKind`].
    fn add_edge(&mut self, from: NodeId, to: NodeId, kind: EdgeKind) -> Result<(), GraphError>;

    /// Remove the directed edge `from → to` that has the given [`EdgeKind`].
    /// Errors if no such edge exists.
    fn remove_edge_by_kind(
        &mut self,
        from: NodeId,
        to: NodeId,
        kind: EdgeKind,
    ) -> Result<(), GraphError>;

    /// Return the ids of the direct structural children (outgoing `Ev` edges).
    fn children(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError>;

    /// Return all siblings that precede `id` in sequence order (incoming `Eh`
    /// chain), ordered earliest-first.
    fn siblings_before(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError>;

    /// Return all siblings that follow `id` in sequence order (outgoing `Eh`
    /// chain), ordered earliest-first.
    fn siblings_after(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError>;

    /// Return the structural parent (`Ev` reverse), or `None` for a root node.
    fn parent(&self, id: NodeId) -> Result<Option<NodeId>, GraphError>;

    /// Return all `Ed` cross-references involving `id` (outgoing and incoming),
    /// paired with their [`EdgeKind`] (always `Ed` in this version; reserved
    /// for future labelled-edge support).
    fn diagonal_refs(&self, id: NodeId) -> Result<Vec<(NodeId, EdgeKind)>, GraphError>;

    /// Return all ancestors of `id`, ordered from direct parent to root.
    fn ancestors(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError>;

    /// Return all descendants of `id` (BFS order).
    ///
    /// Required by CIC cache invalidation (issue 7.1-D).
    fn all_descendants(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError>;

    // === Query Operations ===

    /// Return all node ids whose `pattern` matches `pattern`.
    fn find_by_pattern(&self, pattern: Pattern) -> Result<Vec<NodeId>, GraphError>;

    /// Return all node ids whose `metadata.name` equals `name`.
    fn find_by_name(&self, name: &str) -> Result<Vec<NodeId>, GraphError>;

    /// Return all root nodes (nodes with no parent, i.e. depth 0).
    fn root_nodes(&self) -> Result<Vec<NodeId>, GraphError>;

    /// Return the depth of `id` (number of ancestors). Root nodes have depth 0.
    fn depth(&self, id: NodeId) -> Result<usize, GraphError>;

    // === Contract Operations ===

    /// Return all contracts attached to node `id`.
    fn contracts(&self, id: NodeId) -> Result<Vec<Contract>, GraphError>;

    /// Append a contract to node `id`. Errors if `id` not found.
    fn add_contract(&mut self, id: NodeId, contract: Contract) -> Result<(), GraphError>;

    // === Transaction Operations ===

    /// Begin a transaction. No-op for in-memory backends.
    fn begin_transaction(&mut self) -> Result<(), GraphError>;

    /// Commit the current transaction. No-op for in-memory backends.
    fn commit_transaction(&mut self) -> Result<(), GraphError>;

    /// Roll back the current transaction. No-op for in-memory backends
    /// (state is NOT restored; the operation is accepted but has no effect).
    fn rollback_transaction(&mut self) -> Result<(), GraphError>;
}
