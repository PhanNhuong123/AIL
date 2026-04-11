use std::path::Path;

use crate::errors::GraphError;
use crate::types::{EdgeKind, Node, NodeId, Pattern};

use super::ail_graph::AilGraph;

/// Builds an [`AilGraph`] from a filesystem directory tree.
///
/// This is a structural scaffold builder — it wires Ev (parent → child) and
/// Eh (sibling → sibling) edges from directory nesting and file order. Ed
/// (diagonal cross-reference) edges are resolved later by `ail-text` when
/// the full parser exists.
///
/// Node intent is derived from the directory or file stem name.
/// All nodes are created with `Pattern::Do` as a placeholder; `ail-text`
/// will replace these with the real parsed pattern.
pub struct AilGraphBuilder;

impl AilGraphBuilder {
    /// Walk `root` and return an [`AilGraph`] with Ev/Eh edges wired.
    ///
    /// Only `.ail` files and directories are included; other files are skipped.
    pub fn build_from_directory(root: &Path) -> Result<AilGraph, GraphError> {
        let mut graph = AilGraph::new();
        let root_id = build_subtree(&mut graph, root)?;
        graph.set_root(root_id)?;
        Ok(graph)
    }
}

/// Recursively build the subtree rooted at `dir`, add nodes/edges into `graph`,
/// and return the [`NodeId`] of the subtree root.
fn build_subtree(graph: &mut AilGraph, dir: &Path) -> Result<NodeId, GraphError> {
    let dir_name = extract_name(dir, true);
    let root_node = Node::new(NodeId::new(), dir_name, Pattern::Do);
    let root_id = graph.add_node(root_node)?;

    // Collect entries as (path, is_dir) pairs to avoid repeated stat calls.
    let mut entries: Vec<(std::path::PathBuf, bool)> = std::fs::read_dir(dir)?
        .filter_map(|e| {
            let de = e.ok()?;
            let file_type = de.file_type().ok()?;
            let path = de.path();
            let is_dir = file_type.is_dir();
            if is_dir || path.extension().and_then(|ext| ext.to_str()) == Some("ail") {
                Some((path, is_dir))
            } else {
                None
            }
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    // First pass: create child nodes (recursive for sub-directories).
    let mut child_ids: Vec<NodeId> = Vec::with_capacity(entries.len());
    for (entry, is_dir) in &entries {
        let child_id = if *is_dir {
            build_subtree(graph, entry)?
        } else {
            // .ail file — stub leaf node
            let stem = extract_name(entry, false);
            let leaf = Node::new(NodeId::new(), stem, Pattern::Do);
            graph.add_node(leaf)?
        };
        // Ev edge: parent → child (structural decomposition)
        graph.add_edge(root_id, child_id, EdgeKind::Ev)?;
        child_ids.push(child_id);
    }

    // Second pass: wire consecutive siblings with Eh edges.
    for window in child_ids.windows(2) {
        graph.add_edge(window[0], window[1], EdgeKind::Eh)?;
    }

    Ok(root_id)
}

/// Extract a display name from a path: full name for directories, stem for files.
fn extract_name(path: &Path, is_dir: bool) -> String {
    if is_dir {
        path.file_name()
    } else {
        path.file_stem()
    }
    .and_then(|n| n.to_str())
    .unwrap_or("unknown")
    .to_string()
}
