use crate::errors::GraphError;
use crate::types::{Contract, EdgeKind, Node, NodeId, Pattern};

/// Abstract graph storage interface shared by the in-memory [`crate::graph::AilGraph`]
/// and future backends (e.g. `SqliteGraph` in `ail-db`).
///
/// Downstream crates accept `&dyn GraphBackend` or `&mut dyn GraphBackend`
/// so that pipeline stages are backend-agnostic.
///
/// ## Design notes
///
/// - **Owned reads** (`Option<Node>`): `get_node` returns a cloned `Node`
///   rather than `&Node`. A SQLite backend cannot return a borrow into data
///   it deserialised on the fly (review issue 7.1-A). Clone cost is negligible
///   at our scale.
///
/// - **Object-safe transactions**: the transaction API uses three discrete
///   methods (`begin_transaction`, `commit_transaction`, `rollback_transaction`)
///   instead of a generic closure form. A generic `transaction<F>` would make
///   the trait non-object-safe (review issue 7.1-B).
///
/// - **`all_node_ids` not `all_nodes`**: named to avoid shadowing
///   `AilGraph::all_nodes() -> impl Iterator<Item = &Node>`, which is used
///   throughout the v1.0 codebase.
///
/// - **`all_descendants`**: required by CIC cache invalidation; returns all
///   nodes in the subtree rooted at `id` in BFS order (review issue 7.1-D).
///
/// - **`diagonal_refs` pair**: returns `Vec<(NodeId, EdgeKind)>` so that
///   future labelled `Ed` variants can be distinguished without a breaking
///   change (review issue 7.1-E). All edges in v2.0 carry `EdgeKind::Ed`.
pub trait GraphBackend: Send + Sync {
    // ─── Node operations ─────────────────────────────────────────────────────

    /// Insert `node` into the graph. Returns the node's [`NodeId`].
    ///
    /// Errors with [`GraphError::DuplicateNodeId`] if a node with the same id
    /// already exists.
    fn add_node(&mut self, node: Node) -> Result<NodeId, GraphError>;

    /// Return a clone of the node identified by `id`, or `None` when the node
    /// does not exist. Never errors for a missing id — use `None` to signal
    /// absence.
    fn get_node(&self, id: NodeId) -> Result<Option<Node>, GraphError>;

    /// Replace the stored node at `id` with `node`.
    ///
    /// Errors with [`GraphError::NodeNotFound`] if `id` is unknown.
    fn update_node(&mut self, id: NodeId, node: Node) -> Result<(), GraphError>;

    /// Remove the node and all its incident edges.
    ///
    /// Errors with [`GraphError::NodeNotFound`] if `id` is unknown.
    fn remove_node(&mut self, id: NodeId) -> Result<(), GraphError>;

    /// Return all node ids currently in the graph.
    fn all_node_ids(&self) -> Result<Vec<NodeId>, GraphError>;

    /// Return the total number of nodes in the graph.
    fn node_count(&self) -> usize;

    // ─── Edge operations ──────────────────────────────────────────────────────

    /// Add a directed edge `from → to` labelled with `kind`.
    ///
    /// Errors if either endpoint does not exist.
    fn add_edge(&mut self, from: NodeId, to: NodeId, kind: EdgeKind) -> Result<(), GraphError>;

    /// Remove the directed edge `from → to` that carries `kind`.
    ///
    /// Errors with [`GraphError::EdgeKindNotFound`] if no such edge exists.
    fn remove_edge_by_kind(
        &mut self,
        from: NodeId,
        to: NodeId,
        kind: EdgeKind,
    ) -> Result<(), GraphError>;

    // ─── Navigation ───────────────────────────────────────────────────────────

    /// Return the ids of all direct structural children (outgoing `Ev` edges).
    fn children(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError>;

    /// Return all siblings that precede `id` in execution order (traversing
    /// incoming `Eh` edges), ordered earliest-first.
    fn siblings_before(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError>;

    /// Return all siblings that follow `id` in execution order (traversing
    /// outgoing `Eh` edges), ordered earliest-first.
    fn siblings_after(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError>;

    /// Return the structural parent (`Ev` incoming), or `None` for a root node.
    fn parent(&self, id: NodeId) -> Result<Option<NodeId>, GraphError>;

    /// Return all cross-references involving `id` via `Ed` edges (both
    /// outgoing and incoming), each paired with its [`EdgeKind`].
    ///
    /// In v2.0 all Ed edges carry `EdgeKind::Ed`. The pair is reserved for
    /// future labelled-edge variants.
    fn diagonal_refs(&self, id: NodeId) -> Result<Vec<(NodeId, EdgeKind)>, GraphError>;

    /// Return the ids of nodes reached by **outgoing** `Ed` edges from `id`.
    ///
    /// These are the nodes that `id` cross-references (calls, uses types of,
    /// follows templates of). Used by CIC computation to collect call contracts
    /// that flow into `id`'s context packet.
    fn outgoing_diagonal_refs(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError>;

    /// Return all ancestors of `id`, ordered from direct parent to root.
    fn ancestors(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError>;

    /// Return all descendants of `id` in BFS order (children first, then their
    /// children, etc.). Required by CIC cache invalidation (issue 7.1-D).
    fn all_descendants(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError>;

    // ─── Query operations ─────────────────────────────────────────────────────

    /// Return all node ids whose `pattern` field equals `pattern`.
    fn find_by_pattern(&self, pattern: Pattern) -> Result<Vec<NodeId>, GraphError>;

    /// Return all node ids whose `metadata.name` equals `name`.
    fn find_by_name(&self, name: &str) -> Result<Vec<NodeId>, GraphError>;

    /// Return all root nodes — nodes that have no structural parent.
    fn root_nodes(&self) -> Result<Vec<NodeId>, GraphError>;

    /// Return the depth of `id`: 0 for root nodes, 1 for their children, etc.
    fn depth(&self, id: NodeId) -> Result<usize, GraphError>;

    // ─── Contract operations ──────────────────────────────────────────────────

    /// Return all contracts attached to `id`.
    fn contracts(&self, id: NodeId) -> Result<Vec<Contract>, GraphError>;

    /// Append `contract` to the node identified by `id`.
    ///
    /// Errors with [`GraphError::NodeNotFound`] if `id` is unknown.
    fn add_contract(&mut self, id: NodeId, contract: Contract) -> Result<(), GraphError>;

    // ─── Transaction operations ───────────────────────────────────────────────

    /// Begin a transaction. No-op for in-memory backends.
    fn begin_transaction(&mut self) -> Result<(), GraphError>;

    /// Commit the current transaction. No-op for in-memory backends.
    fn commit_transaction(&mut self) -> Result<(), GraphError>;

    /// Roll back the current transaction. No-op for in-memory backends —
    /// state is NOT restored; the operation succeeds but has no effect.
    fn rollback_transaction(&mut self) -> Result<(), GraphError>;
}
