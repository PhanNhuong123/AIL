//! Integration tests for the `ail.review` MCP tool (task 13.2 Phase E).
//!
//! These tests exercise the tool dispatch, the tools/list schema, and
//! the leaf/Unavailable response paths that are always reachable regardless
//! of the `embeddings` feature flag.

use std::path::PathBuf;

use ail_graph::{AilGraph, EdgeKind, Node, NodeId, Pattern};
use ail_mcp::{JsonRpcId, JsonRpcRequest, McpServer, ProjectContext};
use serde_json::{json, Value};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn empty_server() -> McpServer {
    McpServer::new(PathBuf::from("."), ProjectContext::Raw(AilGraph::new()))
}

fn memory_server(graph: AilGraph) -> McpServer {
    McpServer::new(PathBuf::from("."), ProjectContext::Raw(graph))
}

fn tools_call(name: &str, args: Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(JsonRpcId::Number(1)),
        method: "tools/call".into(),
        params: Some(json!({"name": name, "arguments": args})),
    }
}

fn tools_list_request() -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(JsonRpcId::Number(2)),
        method: "tools/list".into(),
        params: None,
    }
}

// ── t132_mcp_review_tool_in_tools_list ───────────────────────────────────────

/// Verify `ail.review` appears in the `tools/list` response with the correct
/// `inputSchema.properties.node` field.
#[test]
fn t132_mcp_review_tool_in_tools_list() {
    let server = empty_server();
    let resp = server.handle(tools_list_request()).unwrap();
    let result = resp.result.unwrap();

    let tools = result["tools"].as_array().unwrap();
    let review_tool = tools
        .iter()
        .find(|t| t["name"].as_str() == Some("ail.review"));

    assert!(
        review_tool.is_some(),
        "ail.review must appear in tools/list; tools present: {:?}",
        tools
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect::<Vec<_>>()
    );

    let tool = review_tool.unwrap();

    // inputSchema must be present with the required `node` property.
    let schema = &tool["inputSchema"];
    assert_eq!(
        schema["type"].as_str(),
        Some("object"),
        "inputSchema.type must be 'object'"
    );

    let node_prop = &schema["properties"]["node"];
    assert_eq!(
        node_prop["type"].as_str(),
        Some("string"),
        "properties.node.type must be 'string'"
    );
    assert!(
        node_prop.get("description").is_some(),
        "properties.node must have a description"
    );

    let required = schema["required"].as_array().unwrap();
    assert!(
        required.iter().any(|r| r.as_str() == Some("node")),
        "inputSchema.required must contain 'node'"
    );
}

// ── t132_mcp_review_leaf_node_returns_na_status ───────────────────────────────

/// A node with no children must return `status = "N/A"`, `coverage = null`,
/// and a non-empty suggestion string.
///
/// Without the `embeddings` feature, the response is `"Unavailable"` instead.
/// Both paths are tested: neither panics and both return valid structured JSON.
#[test]
fn t132_mcp_review_leaf_node_returns_na_status() {
    // Build a graph with a single leaf node (no children, no Ev edges out).
    let mut graph = AilGraph::new();
    let leaf = graph
        .add_node(Node::new(NodeId::new(), "leaf node intent", Pattern::Do))
        .unwrap();
    graph.set_root(leaf).unwrap();

    let server = memory_server(graph);

    let req = tools_call("ail.review", json!({"node": leaf.to_string()}));
    let resp = server.handle(req).unwrap();

    // Must never be a JSON-RPC error.
    assert!(
        resp.error.is_none(),
        "ail.review must not return a JSON-RPC error for a leaf node; got: {:?}",
        resp.error
    );

    let result = resp.result.unwrap();

    // node_id must always be present.
    assert_eq!(
        result["node_id"].as_str(),
        Some(leaf.to_string().as_str()),
        "node_id must match the queried node"
    );

    // coverage must be null (leaf path has no score).
    assert!(
        result["coverage"].is_null(),
        "coverage must be null for a leaf node"
    );

    // status must be either "N/A" (with embeddings) or "Unavailable" (without).
    let status = result["status"].as_str().unwrap_or("");
    assert!(
        status == "N/A" || status == "Unavailable",
        "status must be 'N/A' or 'Unavailable' for a leaf node; got: {status}"
    );

    // suggestion must be a non-empty string.
    let suggestion = result["suggestion"].as_str().unwrap_or("");
    assert!(!suggestion.is_empty(), "suggestion must not be empty");
}

// ── t132_mcp_review_unavailable_when_embeddings_missing ──────────────────────

/// Without the `embeddings` feature, `ail.review` must return
/// `status = "Unavailable"` and must not panic, regardless of the node queried.
#[cfg(not(feature = "embeddings"))]
#[test]
fn t132_mcp_review_unavailable_when_embeddings_missing() {
    let mut graph = AilGraph::new();
    // A parent with children — would normally trigger scoring.
    let root = graph
        .add_node(Node::new(NodeId::new(), "root node", Pattern::Describe))
        .unwrap();
    let child = graph
        .add_node(Node::new(NodeId::new(), "child node intent", Pattern::Do))
        .unwrap();
    graph.add_edge(root, child, EdgeKind::Ev).unwrap();
    graph.set_root(root).unwrap();

    let server = memory_server(graph);

    let req = tools_call("ail.review", json!({"node": root.to_string()}));
    let resp = server.handle(req).unwrap();

    // Must not be a JSON-RPC error.
    assert!(
        resp.error.is_none(),
        "ail.review must not return a JSON-RPC error when embeddings are missing; got: {:?}",
        resp.error
    );

    let result = resp.result.unwrap();

    assert_eq!(
        result["status"].as_str(),
        Some("Unavailable"),
        "status must be 'Unavailable' when built without embeddings feature"
    );

    let suggestion = result["suggestion"].as_str().unwrap_or("");
    assert!(
        !suggestion.is_empty(),
        "suggestion must not be empty even for Unavailable status"
    );

    // coverage must be null in all Unavailable responses.
    assert!(
        result["coverage"].is_null(),
        "coverage must be null when status is Unavailable"
    );
}
