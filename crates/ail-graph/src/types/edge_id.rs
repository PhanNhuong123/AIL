use petgraph::stable_graph::EdgeIndex;

/// Opaque handle to an edge in the `AilGraph`.
///
/// Returned by `AilGraph::add_edge` and accepted by `AilGraph::remove_edge`.
/// Edge identity is graph-session scoped — `EdgeId` is not serializable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeId(EdgeIndex);

impl EdgeId {
    pub(crate) fn new(index: EdgeIndex) -> Self {
        Self(index)
    }

    pub(crate) fn index(self) -> EdgeIndex {
        self.0
    }
}
