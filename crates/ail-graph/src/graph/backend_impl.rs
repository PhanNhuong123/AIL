use petgraph::visit::EdgeRef;
use petgraph::Direction;

use crate::errors::GraphError;
use crate::types::{Contract, EdgeKind, Node, NodeId, Pattern};

use super::ail_graph::AilGraph;
use super::backend::GraphBackend;

impl GraphBackend for AilGraph {
    // ─── Node operations ─────────────────────────────────────────────────────

    fn add_node(&mut self, node: Node) -> Result<NodeId, GraphError> {
        AilGraph::add_node(self, node)
    }

    fn get_node(&self, id: NodeId) -> Result<Option<Node>, GraphError> {
        match AilGraph::get_node(self, id) {
            Ok(node) => Ok(Some(node.clone())),
            Err(GraphError::NodeNotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn update_node(&mut self, id: NodeId, node: Node) -> Result<(), GraphError> {
        let slot = AilGraph::get_node_mut(self, id)?;
        *slot = node;
        Ok(())
    }

    fn remove_node(&mut self, id: NodeId) -> Result<(), GraphError> {
        AilGraph::remove_node(self, id).map(|_| ())
    }

    fn all_node_ids(&self) -> Result<Vec<NodeId>, GraphError> {
        Ok(self.node_ids().collect())
    }

    fn node_count(&self) -> usize {
        AilGraph::node_count(self)
    }

    // ─── Edge operations ──────────────────────────────────────────────────────

    fn add_edge(&mut self, from: NodeId, to: NodeId, kind: EdgeKind) -> Result<(), GraphError> {
        AilGraph::add_edge(self, from, to, kind).map(|_| ())
    }

    fn remove_edge_by_kind(
        &mut self,
        from: NodeId,
        to: NodeId,
        kind: EdgeKind,
    ) -> Result<(), GraphError> {
        let from_nx = self.resolve_node_index(from)?;
        let to_nx = self.resolve_node_index(to)?;

        // Find the edge index before mutating the graph.
        let edge_idx = self
            .inner()
            .edges_directed(from_nx, Direction::Outgoing)
            .find(|e| e.target() == to_nx && *e.weight() == kind)
            .map(|e| e.id());

        match edge_idx {
            Some(eid) => {
                self.inner_mut().remove_edge(eid);
                Ok(())
            }
            None => Err(GraphError::EdgeKindNotFound { from, to, kind }),
        }
    }

    // ─── Navigation ───────────────────────────────────────────────────────────

    fn children(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError> {
        self.children_of(id)
    }

    fn siblings_before(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError> {
        // Walk the incoming Eh chain toward the first sibling, collecting as
        // we go, then reverse so the result is ordered earliest-first.
        let mut result = Vec::new();
        let mut current = id;
        while let Some(prev) = self.prev_sibling_of(current)? {
            result.push(prev);
            current = prev;
        }
        result.reverse();
        Ok(result)
    }

    fn siblings_after(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError> {
        let mut result = Vec::new();
        let mut current = id;
        while let Some(next) = self.next_sibling_of(current)? {
            result.push(next);
            current = next;
        }
        Ok(result)
    }

    fn parent(&self, id: NodeId) -> Result<Option<NodeId>, GraphError> {
        self.parent_of(id)
    }

    fn diagonal_refs(&self, id: NodeId) -> Result<Vec<(NodeId, EdgeKind)>, GraphError> {
        let refs = self.diagonal_refs_of(id)?;
        Ok(refs.into_iter().map(|nid| (nid, EdgeKind::Ed)).collect())
    }

    fn outgoing_diagonal_refs(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError> {
        self.outgoing_diagonal_refs_of(id)
    }

    fn ancestors(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError> {
        let mut result = Vec::new();
        let mut current = id;
        while let Some(parent_id) = self.parent_of(current)? {
            result.push(parent_id);
            current = parent_id;
        }
        Ok(result)
    }

    fn all_descendants(&self, id: NodeId) -> Result<Vec<NodeId>, GraphError> {
        // BFS over the Ev (structural child) edges.
        let mut result = Vec::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(id);
        while let Some(current) = queue.pop_front() {
            for child in self.children_of(current)? {
                result.push(child);
                queue.push_back(child);
            }
        }
        Ok(result)
    }

    // ─── Query operations ─────────────────────────────────────────────────────

    fn find_by_pattern(&self, pattern: Pattern) -> Result<Vec<NodeId>, GraphError> {
        let ids = self
            .all_nodes()
            .filter(|n| n.pattern == pattern)
            .map(|n| n.id)
            .collect();
        Ok(ids)
    }

    fn find_by_name(&self, name: &str) -> Result<Vec<NodeId>, GraphError> {
        let ids = self
            .all_nodes()
            .filter(|n| n.metadata.name.as_deref() == Some(name))
            .map(|n| n.id)
            .collect();
        Ok(ids)
    }

    fn root_nodes(&self) -> Result<Vec<NodeId>, GraphError> {
        let roots = self
            .node_ids()
            .filter(|&id| {
                self.parent_of(id)
                    .map(|p| p.is_none())
                    // If the node index was somehow removed, skip it.
                    .unwrap_or(false)
            })
            .collect();
        Ok(roots)
    }

    fn depth(&self, id: NodeId) -> Result<usize, GraphError> {
        let mut depth = 0usize;
        let mut current = id;
        while let Some(parent_id) = self.parent_of(current)? {
            depth += 1;
            current = parent_id;
        }
        Ok(depth)
    }

    // ─── Contract operations ──────────────────────────────────────────────────

    fn contracts(&self, id: NodeId) -> Result<Vec<Contract>, GraphError> {
        let node = AilGraph::get_node(self, id)?;
        Ok(node.contracts.clone())
    }

    fn add_contract(&mut self, id: NodeId, contract: Contract) -> Result<(), GraphError> {
        let node = AilGraph::get_node_mut(self, id)?;
        node.contracts.push(contract);
        Ok(())
    }

    // ─── Transaction operations (no-op for in-memory graph) ──────────────────

    fn begin_transaction(&mut self) -> Result<(), GraphError> {
        Ok(())
    }

    fn commit_transaction(&mut self) -> Result<(), GraphError> {
        Ok(())
    }

    fn rollback_transaction(&mut self) -> Result<(), GraphError> {
        Ok(())
    }
}
