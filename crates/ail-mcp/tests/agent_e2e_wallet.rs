//! Integration test: agent-style writes on wallet_service (Phase 15 task 15.2).
//!
//! Simulates an agent flow that:
//! 1. Calls `ail.status` to capture the initial graph size.
//! 2. Issues three `ail.write` calls attaching nested Do children to `transfer_money`.
//! 3. Asserts the graph grew by exactly 3 nodes with unique IDs.
//! 4. Calls `ail.verify` on the mutated in-memory graph and asserts a clean result.
//!
//! `transfer_money` is a top-level `Do` node (has contracts). Its children are
//! nested Dos, which are exempt from the contract requirement (rule v005) and
//! therefore survive the full `validate → type_check → verify` pipeline.

use std::path::{Path, PathBuf};

use ail_graph::AilGraph;
use ail_mcp::{JsonRpcId, JsonRpcRequest, McpServer, ProjectContext};
use serde_json::{json, Value};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn wallet_src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("wallet_service")
        .join("src")
}

/// Parse the wallet_service/src directory into an AilGraph.
fn parse_wallet_graph() -> AilGraph {
    ail_text::parse_directory(&wallet_src_dir()).expect("parse wallet_service/src")
}

/// Build an in-memory MCP server backed by `graph`.
fn memory_server(graph: AilGraph) -> McpServer {
    McpServer::new(PathBuf::from("."), ProjectContext::Raw(graph))
}

/// Build a `tools/call` JSON-RPC 2.0 request.
fn tools_call_request(name: &str, args: Value, id: u64) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(JsonRpcId::Number(id as i64)),
        method: "tools/call".into(),
        params: Some(json!({"name": name, "arguments": args})),
    }
}

/// Find the NodeId of the `transfer_money` node in the graph by intent.
///
/// Panics with a clear message when not found or when the match is ambiguous.
fn find_transfer_money_id(graph: &AilGraph) -> String {
    let matches: Vec<String> = graph
        .all_nodes()
        .filter(|n| n.intent.to_lowercase().contains("transfer money"))
        .map(|n| n.id.to_string())
        .collect();

    assert!(
        !matches.is_empty(),
        "No node with intent containing 'transfer money' found in wallet_service graph"
    );
    assert!(
        matches.len() == 1,
        "Expected exactly one 'transfer money' node, found {}: {:?}",
        matches.len(),
        matches
    );

    matches[0].clone()
}

/// Extract `result` from a `tools/call` JSON-RPC response.
///
/// Panics when the response is absent or carries a JSON-RPC error.
fn extract_result(resp: Option<ail_mcp::JsonRpcResponse>) -> Value {
    let resp = resp.expect("server must return a response for requests with id");
    assert!(
        resp.error.is_none(),
        "JSON-RPC error on tools/call: {:?}",
        resp.error
    );
    resp.result
        .expect("tools/call response must have a result field")
}

/// Extract `node_count` from an `ail.status` response value.
fn extract_node_count(result: &Value) -> usize {
    result["node_count"]
        .as_u64()
        .expect("ail.status result must have a numeric node_count") as usize
}

/// Extract `node_id` (String) from an `ail.write` response value.
fn extract_write_node_id(result: &Value) -> String {
    result["node_id"]
        .as_str()
        .expect("ail.write result must have a string node_id")
        .to_owned()
}

/// Extract `status` (String) from an `ail.write` response value.
fn extract_write_status(result: &Value) -> &str {
    result["status"]
        .as_str()
        .expect("ail.write result must have a string status field")
}

/// Return true when `ail.verify` reports a clean result (`ok: true`).
fn verify_is_ok(result: &Value) -> bool {
    result["ok"].as_bool().unwrap_or(false)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Agent writes three nested Do children under `transfer_money` and confirms
/// the graph grew by exactly 3 nodes, each with a unique UUID.
#[test]
fn agent_flow_writes_three_children_under_transfer_money() {
    let graph = parse_wallet_graph();
    let transfer_money_id = find_transfer_money_id(&graph);
    let server = memory_server(graph);

    // 1. ail.status — capture initial node_count.
    let status_result =
        extract_result(server.handle(tools_call_request("ail.status", json!({}), 1)));
    let initial_count = extract_node_count(&status_result);

    // 2. Three ail.write calls attaching children to transfer_money.
    let children = [
        ("handle insufficient balance", "handle_insufficient_balance"),
        ("handle invalid user", "handle_invalid_user"),
        ("log transfer error", "log_transfer_error"),
    ];
    let mut new_ids: Vec<String> = Vec::with_capacity(3);
    for (idx, (intent, name)) in children.iter().enumerate() {
        let args = json!({
            "parent_id": transfer_money_id,
            "pattern":   "do",
            "intent":    intent,
            "metadata":  { "name": name },
        });
        let result =
            extract_result(server.handle(tools_call_request("ail.write", args, 10 + idx as u64)));
        assert_eq!(
            extract_write_status(&result),
            "created",
            "write must succeed for intent '{}'",
            intent
        );
        let node_id = extract_write_node_id(&result);
        assert!(
            !node_id.is_empty(),
            "node_id must be non-empty for intent '{}'",
            intent
        );
        new_ids.push(node_id);
    }

    // 3. ail.status again — node_count must have grown by exactly 3.
    let status2_result =
        extract_result(server.handle(tools_call_request("ail.status", json!({}), 99)));
    let final_count = extract_node_count(&status2_result);
    assert_eq!(
        final_count,
        initial_count + 3,
        "node_count must grow by exactly 3; before={}, after={}",
        initial_count,
        final_count
    );

    // 4. All 3 new node ids are unique.
    let unique: std::collections::HashSet<_> = new_ids.iter().collect();
    assert_eq!(
        unique.len(),
        3,
        "expected 3 unique new node ids; got {:?}",
        new_ids
    );
}

/// After agent writes, `ail.verify` promotes the in-memory graph through the
/// full pipeline (`validate → type_check → verify`) and must report clean.
///
/// Nested Do nodes (children of `transfer_money`) are exempt from the
/// before/after contract requirement (rule v005), so they do not break
/// verification.
#[test]
fn agent_flow_leaves_verify_clean() {
    let graph = parse_wallet_graph();
    let transfer_money_id = find_transfer_money_id(&graph);
    let server = memory_server(graph);

    // Attach 3 nested Do children via ail.write (sets dirty = true).
    for (idx, (intent, name)) in [
        ("handle insufficient balance", "handle_insufficient_balance"),
        ("handle invalid user", "handle_invalid_user"),
        ("log transfer error", "log_transfer_error"),
    ]
    .iter()
    .enumerate()
    {
        let args = json!({
            "parent_id": transfer_money_id,
            "pattern":   "do",
            "intent":    intent,
            "metadata":  { "name": name },
        });
        let result =
            extract_result(server.handle(tools_call_request("ail.write", args, 10 + idx as u64)));
        assert_eq!(
            extract_write_status(&result),
            "created",
            "write must succeed for intent '{}'",
            intent
        );
    }

    // ail.verify: dirty=true so it runs refresh_from_graph (in-memory edits preserved).
    let verify_result =
        extract_result(server.handle(tools_call_request("ail.verify", json!({}), 200)));
    assert!(
        verify_is_ok(&verify_result),
        "ail.verify must report clean after attaching 3 nested Do children; \
         response={}",
        serde_json::to_string_pretty(&verify_result).unwrap()
    );
}
