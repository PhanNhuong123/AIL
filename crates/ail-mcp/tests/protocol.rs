//! JSON-RPC 2.0 protocol-layer tests.
//!
//! These tests verify serde behaviour and the MCP server dispatch logic
//! without touching the pipeline. A minimal server is built with an empty
//! in-memory graph so no disk access is needed.

use std::path::PathBuf;

use ail_graph::AilGraph;
use ail_mcp::{
    JsonRpcId, JsonRpcRequest, JsonRpcResponse, McpServer, ProjectContext, METHOD_NOT_FOUND,
};
use serde_json::{json, Value};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn empty_server() -> McpServer {
    McpServer::new(PathBuf::from("."), ProjectContext::Raw(AilGraph::new()))
}

fn make_request(method: &str, params: Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(JsonRpcId::Number(1)),
        method: method.into(),
        params: Some(params),
    }
}

// ── Serde roundtrips ─────────────────────────────────────────────────────────

#[test]
fn roundtrip_jsonrpc_request() {
    let json_str = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"ail.search","arguments":{"query":"wallet"}}}"#;
    let req: JsonRpcRequest = serde_json::from_str(json_str).unwrap();
    assert_eq!(req.method, "tools/call");
    assert_eq!(req.id, Some(JsonRpcId::Number(1)));
    let round = serde_json::to_string(&req).unwrap();
    let re: JsonRpcRequest = serde_json::from_str(&round).unwrap();
    assert_eq!(re.method, "tools/call");
}

#[test]
fn roundtrip_jsonrpc_response_result() {
    let resp = JsonRpcResponse::ok(Some(JsonRpcId::Number(42)), json!({"ok": true, "count": 3}));
    let s = serde_json::to_string(&resp).unwrap();
    let back: JsonRpcResponse = serde_json::from_str(&s).unwrap();
    assert_eq!(back.id, Some(JsonRpcId::Number(42)));
    assert!(back.result.is_some());
    assert!(back.error.is_none());
}

#[test]
fn roundtrip_jsonrpc_response_error() {
    use ail_mcp::JsonRpcError;
    let resp = JsonRpcResponse::err(
        Some(JsonRpcId::Str("req-abc".into())),
        JsonRpcError::new(METHOD_NOT_FOUND, "unknown method"),
    );
    let s = serde_json::to_string(&resp).unwrap();
    let back: JsonRpcResponse = serde_json::from_str(&s).unwrap();
    assert!(back.result.is_none());
    let err = back.error.unwrap();
    assert_eq!(err.code, METHOD_NOT_FOUND);
    assert_eq!(err.message, "unknown method");
}

// ── Server dispatch ───────────────────────────────────────────────────────────

#[test]
fn mcp_server_handles_tools_list() {
    let server = empty_server();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(JsonRpcId::Number(2)),
        method: "tools/list".into(),
        params: None,
    };
    let resp = server.handle(req).expect("expected a response");
    let result = resp.result.unwrap();
    let tools = result["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"ail.search"));
    assert!(names.contains(&"ail.context"));
    assert!(names.contains(&"ail.verify"));
    assert!(names.contains(&"ail.build"));
    assert!(names.contains(&"ail.status"));
}

#[test]
fn mcp_server_handles_initialize() {
    let server = empty_server();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(JsonRpcId::Number(3)),
        method: "initialize".into(),
        params: None,
    };
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();
    assert!(result.get("protocolVersion").is_some());
    assert_eq!(result["serverInfo"]["name"].as_str(), Some("ail-mcp"));
}

#[test]
fn mcp_server_notification_returns_none() {
    let server = empty_server();
    // `initialized` is a notification (no id).
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: None,
        method: "initialized".into(),
        params: None,
    };
    let resp = server.handle(req);
    assert!(resp.is_none(), "notifications must not produce a response");
}

#[test]
fn mcp_server_dispatch_unknown_tool_returns_error() {
    let server = empty_server();
    let req = make_request(
        "tools/call",
        json!({"name": "ail.nonexistent", "arguments": {}}),
    );
    let resp = server.handle(req).unwrap();
    assert!(resp.result.is_none());
    let err = resp.error.unwrap();
    assert_eq!(err.code, METHOD_NOT_FOUND);
}

#[test]
fn mcp_server_unknown_method_with_id_returns_error() {
    let server = empty_server();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(JsonRpcId::Number(99)),
        method: "doesNotExist".into(),
        params: None,
    };
    let resp = server.handle(req).unwrap();
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, METHOD_NOT_FOUND);
}
