//! Directory-level `.ail` parser.
//!
//! [`parse_directory`] walks a directory tree, parses each `.ail` file with
//! [`super::parse`], and assembles the results into a single [`AilGraph`]
//! with directory-derived Ev/Eh edges.
//!
//! # Container nodes
//!
//! Directories become structural container nodes with `Pattern::Describe` and
//! `metadata.name = None`. This is the established convention — see
//! `set_container()` in the wallet-service fixture. `Pattern::Do` cannot be
//! used because validation rule v005 requires top-level Do nodes to have
//! Before + After contracts.
//!
//! # Edge types
//!
//! Only Ev (parent→child) and Eh (sibling→sibling) edges are created.
//! Ed (diagonal cross-reference) edges are **not** produced by the parser —
//! they are wired during later pipeline stages (index resolution, type
//! checking, hydration).
//!
//! # File ordering
//!
//! Entries within a directory are sorted alphabetically by filename,
//! directories before files. This produces deterministic Eh edge ordering
//! but does not reflect logical dependency order. Topological sort based on
//! Ed edges is deferred to a future version.

use std::fs;
use std::path::Path;

use ail_graph::types::{EdgeKind, Node, NodeId, Pattern};
use ail_graph::AilGraph;

use crate::errors::ParseError;

/// Parse all `.ail` files under `root` into a single [`AilGraph`].
///
/// The returned graph has a structural Describe root derived from the
/// directory name, with Ev edges to children and Eh edges between siblings.
/// It is **not** validated — callers should pass it through
/// `ail_graph::validate_graph()` for structural checks.
pub fn parse_directory(root: &Path) -> Result<AilGraph, ParseError> {
    if !root.is_dir() {
        return Err(ParseError::IoError {
            message: format!("not a directory: {}", root.display()),
            path: Some(root.display().to_string()),
        });
    }

    let mut graph = AilGraph::new();
    let root_id = parse_dir_subtree(&mut graph, root)?;
    graph.set_root(root_id).map_err(ParseError::GraphError)?;
    Ok(graph)
}

/// Recursively process a directory, adding nodes and edges to `graph`.
/// Returns the [`NodeId`] of the structural container node for this directory.
fn parse_dir_subtree(graph: &mut AilGraph, dir: &Path) -> Result<NodeId, ParseError> {
    let dir_name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Create structural container (Describe with name = None).
    let mut container = Node::new(NodeId::new(), &dir_name, Pattern::Describe);
    container.metadata.name = None;
    container.children = Some(Vec::new());
    let container_id = graph.add_node(container).map_err(ParseError::GraphError)?;

    // Collect and partition entries: directories first, then .ail files.
    let (dirs, files) = collect_sorted_entries(dir)?;

    let mut child_ids: Vec<NodeId> = Vec::new();

    // Process subdirectories first.
    for sub_dir in &dirs {
        let sub_id = parse_dir_subtree(graph, sub_dir)?;
        graph
            .add_edge(container_id, sub_id, EdgeKind::Ev)
            .map_err(ParseError::GraphError)?;
        child_ids.push(sub_id);
    }

    // Process .ail files.
    for file_path in &files {
        let source = fs::read_to_string(file_path).map_err(|e| ParseError::IoError {
            message: e.to_string(),
            path: Some(file_path.display().to_string()),
        })?;

        let mini = super::parse(&source)?;
        let root_ids = transfer_nodes(graph, mini)?;

        for &id in &root_ids {
            graph
                .add_edge(container_id, id, EdgeKind::Ev)
                .map_err(ParseError::GraphError)?;
        }
        child_ids.extend(root_ids);
    }

    // Wire Eh edges between consecutive children.
    for pair in child_ids.windows(2) {
        graph
            .add_edge(pair[0], pair[1], EdgeKind::Eh)
            .map_err(ParseError::GraphError)?;
    }

    // Update the container node's children field.
    graph
        .get_node_mut(container_id)
        .map_err(ParseError::GraphError)?
        .children = Some(child_ids);

    Ok(container_id)
}

/// Collect directory entries, partitioned into (directories, .ail files),
/// each sorted alphabetically by filename.
fn collect_sorted_entries(
    dir: &Path,
) -> Result<(Vec<std::path::PathBuf>, Vec<std::path::PathBuf>), ParseError> {
    let entries = fs::read_dir(dir).map_err(|e| ParseError::IoError {
        message: e.to_string(),
        path: Some(dir.display().to_string()),
    })?;

    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| ParseError::IoError {
            message: e.to_string(),
            path: Some(dir.display().to_string()),
        })?;

        let path = entry.path();
        let file_type = entry.file_type().map_err(|e| ParseError::IoError {
            message: e.to_string(),
            path: Some(path.display().to_string()),
        })?;

        if file_type.is_dir() {
            dirs.push(path);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("ail") {
            files.push(path);
        }
    }

    dirs.sort();
    files.sort();
    Ok((dirs, files))
}

/// Transfer all nodes and internal edges from `source` into `target`.
///
/// Returns the root-level [`NodeId`]s — nodes that have no incoming Ev edge
/// in the source graph. These become direct children of the calling
/// directory's container node.
///
/// Only Ev and Eh edges are transferred. Ed (diagonal) edges are not created
/// by the parser and therefore not present in the source graph.
fn transfer_nodes(target: &mut AilGraph, source: AilGraph) -> Result<Vec<NodeId>, ParseError> {
    // Collect all nodes and their relationships before transferring.
    let nodes: Vec<Node> = source.all_nodes().cloned().collect();

    // Identify root nodes (no Ev parent in source).
    let root_ids: Vec<NodeId> = nodes
        .iter()
        .filter(|n| source.parent_of(n.id).ok().flatten().is_none())
        .map(|n| n.id)
        .collect();

    // Collect edge relationships before consuming source.
    let mut ev_edges: Vec<(NodeId, NodeId)> = Vec::new();
    let mut eh_edges: Vec<(NodeId, NodeId)> = Vec::new();

    for node in &nodes {
        // Ev edges: parent → children
        if let Some(children) = &node.children {
            for &child_id in children {
                ev_edges.push((node.id, child_id));
            }
        }

        // Eh edges: node → next sibling
        if let Ok(Some(next_id)) = source.next_sibling_of(node.id) {
            eh_edges.push((node.id, next_id));
        }
    }

    // Add nodes to target.
    for node in nodes {
        target.add_node(node).map_err(ParseError::GraphError)?;
    }

    // Re-create Ev edges.
    for (from, to) in ev_edges {
        target
            .add_edge(from, to, EdgeKind::Ev)
            .map_err(ParseError::GraphError)?;
    }

    // Re-create Eh edges.
    for (from, to) in eh_edges {
        target
            .add_edge(from, to, EdgeKind::Eh)
            .map_err(ParseError::GraphError)?;
    }

    Ok(root_ids)
}
