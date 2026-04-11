use petgraph::visit::EdgeRef;
use petgraph::Direction;

use crate::errors::GraphError;
use crate::types::{EdgeKind, NodeId};

use super::ail_graph::AilGraph;

impl AilGraph {
    /// Return the ids of all nodes reached by outgoing `Ev` edges from `node_id`.
    ///
    /// These are the direct structural children of the node.
    pub fn children_of(&self, node_id: NodeId) -> Result<Vec<NodeId>, GraphError> {
        let nx = self.resolve_node_index(node_id)?;
        let children = self
            .inner()
            .edges_directed(nx, Direction::Outgoing)
            .filter(|e| *e.weight() == EdgeKind::Ev)
            .map(|e| {
                self.inner()
                    .node_weight(e.target())
                    // target always exists because StableDiGraph keeps indices stable
                    .expect("target node must exist")
                    .id
            })
            .collect();
        Ok(children)
    }

    /// Return the id of the node that reaches `node_id` via an `Ev` edge,
    /// i.e. the structural parent. Returns `None` for the root.
    pub fn parent_of(&self, node_id: NodeId) -> Result<Option<NodeId>, GraphError> {
        let nx = self.resolve_node_index(node_id)?;
        let parent = self
            .inner()
            .edges_directed(nx, Direction::Incoming)
            .find(|e| *e.weight() == EdgeKind::Ev)
            .map(|e| {
                self.inner()
                    .node_weight(e.source())
                    .expect("source node must exist")
                    .id
            });
        Ok(parent)
    }

    /// Return the id of the node reached by the outgoing `Eh` edge from `node_id`,
    /// i.e. the next sibling in execution order. Returns `None` if there is none.
    pub fn next_sibling_of(&self, node_id: NodeId) -> Result<Option<NodeId>, GraphError> {
        let nx = self.resolve_node_index(node_id)?;
        let next = self
            .inner()
            .edges_directed(nx, Direction::Outgoing)
            .find(|e| *e.weight() == EdgeKind::Eh)
            .map(|e| {
                self.inner()
                    .node_weight(e.target())
                    .expect("target node must exist")
                    .id
            });
        Ok(next)
    }

    /// Return the id of the node that reaches `node_id` via an `Eh` edge,
    /// i.e. the previous sibling in execution order. Returns `None` if there is none.
    pub fn prev_sibling_of(&self, node_id: NodeId) -> Result<Option<NodeId>, GraphError> {
        let nx = self.resolve_node_index(node_id)?;
        let prev = self
            .inner()
            .edges_directed(nx, Direction::Incoming)
            .find(|e| *e.weight() == EdgeKind::Eh)
            .map(|e| {
                self.inner()
                    .node_weight(e.source())
                    .expect("source node must exist")
                    .id
            });
        Ok(prev)
    }

    /// Return the ids of all nodes connected to `node_id` by an `Ed` edge,
    /// in either direction (outgoing cross-references and incoming back-references).
    pub fn diagonal_refs_of(&self, node_id: NodeId) -> Result<Vec<NodeId>, GraphError> {
        let nx = self.resolve_node_index(node_id)?;

        let outgoing = self
            .inner()
            .edges_directed(nx, Direction::Outgoing)
            .filter(|e| *e.weight() == EdgeKind::Ed)
            .map(|e| {
                self.inner()
                    .node_weight(e.target())
                    .expect("target node must exist")
                    .id
            });

        let incoming = self
            .inner()
            .edges_directed(nx, Direction::Incoming)
            .filter(|e| *e.weight() == EdgeKind::Ed)
            .map(|e| {
                self.inner()
                    .node_weight(e.source())
                    .expect("source node must exist")
                    .id
            });

        let mut refs: Vec<NodeId> = outgoing.chain(incoming).collect();
        refs.dedup();
        Ok(refs)
    }
}
