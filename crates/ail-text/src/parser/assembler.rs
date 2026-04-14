use std::collections::HashMap;

use ail_graph::{AilGraph, EdgeKind, Node, NodeId, Pattern};

use crate::errors::ParseError;
use crate::types::ParsedStatement;

/// Build an `AilGraph` from a flat list of `ParsedStatement`s.
///
/// Reconstructs parent-child relationships using indentation levels.
/// Promise statements are merged as contracts on their parent Do node.
pub(crate) fn assemble_graph(statements: Vec<ParsedStatement>) -> Result<AilGraph, ParseError> {
    let mut graph = AilGraph::new();

    // Stack tracks (indent_level, node_id) for finding the current parent.
    let mut indent_stack: Vec<(usize, NodeId)> = Vec::new();
    // Top-level node ids (indent 0, or unparented).
    let mut top_level_ids: Vec<NodeId> = Vec::new();
    // Tracks the last child added to each parent for Eh edges.
    let mut last_child: HashMap<NodeId, NodeId> = HashMap::new();

    for stmt in statements {
        // ── Validate indentation ─────────────────────────────────────────
        if stmt.indent % 2 != 0 {
            return Err(ParseError::InvalidIndentation {
                found: stmt.indent,
                span: stmt.span,
            });
        }

        // Check for indent jump (increase > 2 from current top of stack)
        if let Some(&(top_indent, _)) = indent_stack.last() {
            if stmt.indent > top_indent + 2 {
                return Err(ParseError::IndentJump {
                    parent_indent: top_indent,
                    child_indent: stmt.indent,
                    span: stmt.span,
                });
            }
        }

        // ── Find parent: pop stack until top.indent < stmt.indent ────────
        while let Some(&(top_indent, _)) = indent_stack.last() {
            if top_indent < stmt.indent {
                break;
            }
            indent_stack.pop();
        }

        let parent = indent_stack.last().map(|&(_, id)| id);

        // ── Promise attachment (not a graph node) ────────────────────────
        if stmt.pattern == Pattern::Promise {
            match parent {
                Some(parent_id) => {
                    let parent_node = graph.get_node(parent_id)?;
                    if parent_node.pattern != Pattern::Do {
                        return Err(ParseError::PromiseWithoutDo { span: stmt.span });
                    }
                    let parent_mut = graph.get_node_mut(parent_id)?;
                    parent_mut.contracts.extend(stmt.contracts);
                }
                None => {
                    return Err(ParseError::PromiseWithoutDo { span: stmt.span });
                }
            }
            continue;
        }

        // ── Create node ──────────────────────────────────────────────────
        let node_id = create_and_add_node(&mut graph, &stmt)?;

        // ── Handle inline children (together/retry) ──────────────────────
        if !stmt.inline_children.is_empty() {
            let child_ids = add_inline_children(&mut graph, node_id, &stmt.inline_children)?;
            let node = graph.get_node_mut(node_id)?;
            node.children = Some(child_ids);
        }

        // ── Wire edges ───────────────────────────────────────────────────
        match parent {
            Some(parent_id) => {
                // Ev edge: parent → child
                graph.add_edge(parent_id, node_id, EdgeKind::Ev)?;

                // Update parent's children list
                let parent_node = graph.get_node_mut(parent_id)?;
                match &mut parent_node.children {
                    Some(children) => children.push(node_id),
                    None => parent_node.children = Some(vec![node_id]),
                }

                // Eh edge: last sibling → this node
                if let Some(&prev_sibling) = last_child.get(&parent_id) {
                    graph.add_edge(prev_sibling, node_id, EdgeKind::Eh)?;
                }
                last_child.insert(parent_id, node_id);
            }
            None => {
                top_level_ids.push(node_id);
            }
        }

        // ── Push onto stack ──────────────────────────────────────────────
        indent_stack.push((stmt.indent, node_id));
    }

    // ── Wire top-level Eh edges ──────────────────────────────────────────
    for window in top_level_ids.windows(2) {
        graph.add_edge(window[0], window[1], EdgeKind::Eh)?;
    }

    // ── Set root ─────────────────────────────────────────────────────────
    if top_level_ids.len() == 1 {
        graph.set_root(top_level_ids[0])?;
    }

    Ok(graph)
}

/// Create a `Node` from a `ParsedStatement` and add it to the graph.
fn create_and_add_node(graph: &mut AilGraph, stmt: &ParsedStatement) -> Result<NodeId, ParseError> {
    let id = NodeId::new();
    let mut node = Node::new(id, &stmt.intent, stmt.pattern.clone());
    node.metadata = stmt.metadata.clone();
    node.expression = stmt.expression.clone();
    node.contracts = stmt.contracts.clone();
    Ok(graph.add_node(node)?)
}

/// Add inline children (for together/retry) and wire Ev + Eh edges.
fn add_inline_children(
    graph: &mut AilGraph,
    parent_id: NodeId,
    children: &[ParsedStatement],
) -> Result<Vec<NodeId>, ParseError> {
    let mut child_ids = Vec::new();

    for child_stmt in children {
        let child_id = create_and_add_node(graph, child_stmt)?;
        graph.add_edge(parent_id, child_id, EdgeKind::Ev)?;

        // Eh edge between consecutive inline children
        if let Some(&prev) = child_ids.last() {
            graph.add_edge(prev, child_id, EdgeKind::Eh)?;
        }

        child_ids.push(child_id);
    }

    Ok(child_ids)
}
