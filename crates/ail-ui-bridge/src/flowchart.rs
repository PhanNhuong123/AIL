use ail_graph::{GraphBackend, NodeId, Pattern};

use crate::errors::BridgeError;
use crate::ids::IdMap;
use crate::types::flowchart::{FlowEdgeJson, FlowNodeJson, FlowNodeKind, FlowchartJson};
use crate::types::status::Status;

/// Build a `FlowchartJson` for the function identified by `fn_path_id`.
///
/// The function node is looked up via `id_map`. Its children (excluding
/// `Promise` nodes) become flowchart nodes. A `Start` node is prepended and
/// an `End` node appended. Sequential edges connect all nodes. Falls back to
/// `[Start, End]` when the function has no eligible children.
pub fn build_flowchart(
    backend: &dyn GraphBackend,
    id_map: &IdMap,
    fn_path_id: &str,
) -> Result<FlowchartJson, BridgeError> {
    let fn_node_id = id_map
        .get_id(fn_path_id)
        .ok_or_else(|| BridgeError::NodeNotFound {
            id: fn_path_id.to_string(),
        })?;

    let fn_node =
        backend
            .get_node(fn_node_id)
            .ok()
            .flatten()
            .ok_or_else(|| BridgeError::NodeNotFound {
                id: fn_path_id.to_string(),
            })?;

    if fn_node.pattern != Pattern::Do {
        return Err(BridgeError::InvalidInput {
            reason: format!("node '{fn_path_id}' is not a Do function"),
        });
    }

    let children = backend.children(fn_node_id).unwrap_or_default();
    let eligible: Vec<NodeId> = children
        .into_iter()
        .filter(|&id| {
            backend
                .get_node(id)
                .ok()
                .flatten()
                .map(|n| n.pattern != Pattern::Promise)
                .unwrap_or(false)
        })
        .collect();

    if eligible.is_empty() {
        return Ok(generate_fallback_flowchart(fn_path_id));
    }

    let mut nodes: Vec<FlowNodeJson> = Vec::new();
    let mut edges: Vec<FlowEdgeJson> = Vec::new();

    // Start node.
    let start_id = format!("{fn_path_id}.start");
    nodes.push(FlowNodeJson {
        id: start_id.clone(),
        kind: FlowNodeKind::Start,
        label: "Start".to_string(),
        x: 0.0,
        y: 0.0,
        status: None,
    });

    let mut prev_id = start_id;

    for (idx, child_id) in eligible.iter().enumerate() {
        let Some(child_node) = backend.get_node(*child_id).ok().flatten() else {
            // The eligible list was built by calling get_node just above in
            // the filter — a missing node here means the backend returned
            // inconsistent data, which is graph corruption.
            return Err(BridgeError::NodeNotFound {
                id: id_map.get_path(*child_id).to_string(),
            });
        };

        let step_path = id_map.get_path(*child_id).to_string();
        let node_id_str = if step_path.is_empty() {
            format!("{fn_path_id}.step_{idx}")
        } else {
            step_path
        };

        let label = child_node
            .metadata
            .name
            .as_deref()
            .unwrap_or(&child_node.intent)
            .to_string();

        let kind = kind_for_pattern(&child_node.pattern);

        nodes.push(FlowNodeJson {
            id: node_id_str.clone(),
            kind,
            label,
            x: 0.0,
            y: 80.0 * (idx as f64 + 1.0),
            status: Some(Status::Ok),
        });

        edges.push(FlowEdgeJson {
            from: prev_id.clone(),
            to: node_id_str.clone(),
            label: None,
            style: None,
        });

        prev_id = node_id_str;
    }

    // End node.
    let end_y = 80.0 * (eligible.len() as f64 + 1.0);
    let end_id = format!("{fn_path_id}.end");
    nodes.push(FlowNodeJson {
        id: end_id.clone(),
        kind: FlowNodeKind::End,
        label: "End".to_string(),
        x: 0.0,
        y: end_y,
        status: None,
    });
    edges.push(FlowEdgeJson {
        from: prev_id,
        to: end_id,
        label: None,
        style: None,
    });

    Ok(FlowchartJson { nodes, edges })
}

/// Generate a minimal `[Start, End]` flowchart for functions with no children.
pub fn generate_fallback_flowchart(fn_path_id: &str) -> FlowchartJson {
    let start_id = format!("{fn_path_id}.start");
    let end_id = format!("{fn_path_id}.end");

    FlowchartJson {
        nodes: vec![
            FlowNodeJson {
                id: start_id.clone(),
                kind: FlowNodeKind::Start,
                label: "Start".to_string(),
                x: 0.0,
                y: 0.0,
                status: None,
            },
            FlowNodeJson {
                id: end_id.clone(),
                kind: FlowNodeKind::End,
                label: "End".to_string(),
                x: 0.0,
                y: 80.0,
                status: None,
            },
        ],
        edges: vec![FlowEdgeJson {
            from: start_id,
            to: end_id,
            label: None,
            style: None,
        }],
    }
}

/// Map an AIL `Pattern` to a flowchart node kind.
pub fn kind_for_pattern(pattern: &Pattern) -> FlowNodeKind {
    match pattern {
        Pattern::Check => FlowNodeKind::Decision,
        Pattern::Fetch => FlowNodeKind::Io,
        Pattern::Save | Pattern::Update => FlowNodeKind::Storage,
        Pattern::Return => FlowNodeKind::End,
        Pattern::Do => FlowNodeKind::Sub,
        _ => FlowNodeKind::Process,
    }
}
