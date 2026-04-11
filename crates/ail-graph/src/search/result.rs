use crate::types::{NodeId, Pattern};

/// A ranked search result returned by [`super::Bm25Index::search`].
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    /// The matched node.
    pub node_id: NodeId,
    /// BM25 relevance score (higher = more relevant).
    pub score: f32,
    /// The node's intent string.
    pub intent: String,
    /// The node's symbol name, if present (`metadata.name`).
    pub name: Option<String>,
    /// The node's pattern kind.
    pub pattern: Pattern,
    /// Depth of this node in the Ev tree (root = 0).
    pub depth: usize,
    /// Path from the root down to this node. Each element is the node's
    /// `metadata.name` if set, otherwise the first word of its intent.
    pub path: Vec<String>,
}
