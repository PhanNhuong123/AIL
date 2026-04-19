//! Phase 15 Task 15.1 — End-to-end `ail.review` MCP tests on
//! `examples/wallet_service/`.
//!
//! Exercises the `ail.review` tool with the real wallet_service graph and
//! validates the `ReviewOutput` JSON schema for:
//! - A leaf node (no children) — must return `status: "N/A"` or `"Unavailable"`
//!   and `coverage: null`.
//! - A non-leaf node with 3 synthetic children attached — schema shape is
//!   validated whether embeddings are available or not.
//! - A feature-gated full-schema check that runs only with the `embeddings`
//!   feature and ONNX model files present.
//!
//! Children are attached via `AilGraph::add_node` + `add_edge(…, EdgeKind::Ev)`
//! at test-time; the source `examples/wallet_service/src/*.ail` files remain
//! unchanged (they are the canonical flat v2.0 fixture).

use std::path::{Path, PathBuf};

use ail_graph::{AilGraph, EdgeKind, Node, NodeId, Pattern};
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

fn parse_wallet_graph() -> AilGraph {
    let dir = wallet_src_dir();
    ail_text::parse_directory(&dir).expect("parse wallet_service/src")
}

/// Iterate all nodes; return the id of the node whose intent is "transfer money"
/// (the canonical intent from `transfer_money.ail`). Matches case-insensitively
/// on the exact phrase "transfer money" to avoid ambiguity with `TransferResult`.
fn resolve_transfer_money(graph: &AilGraph) -> NodeId {
    let matches: Vec<NodeId> = graph
        .all_nodes()
        .filter(|n| n.intent.to_lowercase().contains("transfer money"))
        .map(|n| n.id)
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

    matches[0]
}

/// Attach three synthetic Ev children to `parent` and return their ids.
fn attach_three_children(graph: &mut AilGraph, parent: NodeId) -> [NodeId; 3] {
    let intents = [
        "validate users have sufficient balance",
        "execute transfer between wallets",
        "save transfer result to ledger",
    ];
    let mut ids = [NodeId::new(); 3];
    for (i, intent) in intents.iter().enumerate() {
        let child_id = graph
            .add_node(Node::new(NodeId::new(), *intent, Pattern::Do))
            .expect("add child node");
        graph
            .add_edge(parent, child_id, EdgeKind::Ev)
            .expect("add Ev edge");
        ids[i] = child_id;
    }
    ids
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

// ── t151_mcp_review_wallet_leaf_node_schema_shape ─────────────────────────────

/// Verify the JSON schema of `ail.review` for a real wallet_service leaf node.
///
/// `transfer_money` as parsed from disk has no Ev children — it is a leaf.
/// The response must carry `coverage: null` and `status` of either `"N/A"` (with
/// embeddings) or `"Unavailable"` (without), plus a non-empty suggestion.
#[test]
fn t151_mcp_review_wallet_leaf_node_schema_shape() {
    let graph = parse_wallet_graph();
    let transfer_id = resolve_transfer_money(&graph);

    let server = memory_server(graph);
    let req = tools_call("ail.review", json!({"node": transfer_id.to_string()}));
    let resp = server.handle(req).unwrap();

    assert!(
        resp.error.is_none(),
        "ail.review must not return a JSON-RPC error; got: {:?}",
        resp.error
    );

    let result = resp.result.unwrap();

    // node_id must round-trip.
    assert_eq!(
        result["node_id"].as_str(),
        Some(transfer_id.to_string().as_str()),
        "node_id must match the queried node uuid"
    );

    // Leaf path: coverage must be null.
    assert!(
        result["coverage"].is_null(),
        "coverage must be null for a leaf node; got: {}",
        result["coverage"]
    );

    // status must be "N/A" (with embeddings) or "Unavailable" (without).
    let status = result["status"].as_str().unwrap_or("");
    assert!(
        status == "N/A" || status == "Unavailable",
        "status must be 'N/A' or 'Unavailable' for a leaf node; got: {status}"
    );

    // children_coverage must be an empty array for a leaf.
    let children = result["children_coverage"]
        .as_array()
        .expect("children_coverage must be a JSON array");
    assert!(
        children.is_empty(),
        "children_coverage must be empty for a leaf node; got {} items",
        children.len()
    );

    // missing must be an empty array for a leaf.
    let missing = result["missing"]
        .as_array()
        .expect("missing must be a JSON array");
    assert!(
        missing.is_empty(),
        "missing must be empty for a leaf node; got {} items",
        missing.len()
    );

    // suggestion must be non-empty.
    let suggestion = result["suggestion"].as_str().unwrap_or("");
    assert!(!suggestion.is_empty(), "suggestion must not be empty");

    // skip_serializing_if fields must be absent or null when not set.
    if let Some(v) = result.get("empty_parent") {
        assert!(
            v.is_null() || v.as_bool() == Some(false),
            "empty_parent must be absent, null, or false; got: {v}"
        );
    }
    if let Some(v) = result.get("degenerate_basis_fallback") {
        assert!(
            v.is_null() || v.as_bool() == Some(false),
            "degenerate_basis_fallback must be absent, null, or false; got: {v}"
        );
    }
}

// ── t151_mcp_review_wallet_with_children_schema_shape ────────────────────────

/// With 3 synthetic Ev children attached to transfer_money, verify every
/// required schema field is a well-typed JSON value.
///
/// Does NOT assert specific numeric coverage — embeddings may or may not be
/// available in CI. Both the `"Unavailable"` path and the scored path are
/// validated for structural correctness.
#[test]
fn t151_mcp_review_wallet_with_children_schema_shape() {
    let mut graph = parse_wallet_graph();
    let transfer_id = resolve_transfer_money(&graph);
    let child_ids = attach_three_children(&mut graph, transfer_id);

    let server = memory_server(graph);
    let req = tools_call("ail.review", json!({"node": transfer_id.to_string()}));
    let resp = server.handle(req).unwrap();

    assert!(
        resp.error.is_none(),
        "ail.review must not return a JSON-RPC error; got: {:?}",
        resp.error
    );

    let result = resp.result.unwrap();

    // node_id must round-trip.
    assert_eq!(
        result["node_id"].as_str(),
        Some(transfer_id.to_string().as_str()),
        "node_id must match the queried node uuid"
    );

    // status must be one of the valid non-leaf values.
    let status = result["status"].as_str().unwrap_or("");
    let valid_statuses = ["Full", "Partial", "Weak", "Unavailable"];
    assert!(
        valid_statuses.contains(&status),
        "status must be one of {valid_statuses:?} for a node with children; got: {status}"
    );

    if status == "Unavailable" {
        // Unavailable path: suggestion still required; arrays still present.
        let suggestion = result["suggestion"].as_str().unwrap_or("");
        assert!(
            !suggestion.is_empty(),
            "suggestion must not be empty even for Unavailable"
        );

        assert!(
            result["missing"].is_array(),
            "missing must be a JSON array even for Unavailable"
        );

        assert!(
            result["children_coverage"].is_array(),
            "children_coverage must always be an array"
        );
    } else {
        // Scored path: coverage must be f64 in [0.0, 1.0].
        let coverage = result["coverage"]
            .as_f64()
            .expect("coverage must be a number when status is Full/Partial/Weak");
        assert!(
            (0.0..=1.0).contains(&coverage),
            "coverage must be in [0.0, 1.0]; got: {coverage}"
        );

        // children_coverage must have exactly 3 entries.
        let children = result["children_coverage"]
            .as_array()
            .expect("children_coverage must be a JSON array");
        assert_eq!(
            children.len(),
            3,
            "children_coverage must have 3 entries; got {}",
            children.len()
        );

        // Each child entry must have the required fields with correct types.
        let child_id_strings: Vec<String> = child_ids.iter().map(|id| id.to_string()).collect();
        for (i, item) in children.iter().enumerate() {
            let item_node_id = item["node_id"]
                .as_str()
                .unwrap_or_else(|| panic!("children_coverage[{i}].node_id must be a string"));
            assert!(
                child_id_strings.contains(&item_node_id.to_string()),
                "children_coverage[{i}].node_id '{item_node_id}' not in expected child ids"
            );

            assert!(
                item["intent_preview"].as_str().is_some(),
                "children_coverage[{i}].intent_preview must be a string"
            );

            let contribution = item["contribution"]
                .as_f64()
                .unwrap_or_else(|| panic!("children_coverage[{i}].contribution must be f64"));
            assert!(
                contribution.is_finite(),
                "children_coverage[{i}].contribution must be finite; got: {contribution}"
            );
        }

        // missing must be an array.
        assert!(result["missing"].is_array(), "missing must be a JSON array");

        // suggestion must be non-empty.
        let suggestion = result["suggestion"].as_str().unwrap_or("");
        assert!(!suggestion.is_empty(), "suggestion must not be empty");
    }
}

// ── t151_mcp_review_wallet_with_children_full_schema_embeddings ───────────────

/// Strict validation with ONNX available: status is scored (not Unavailable),
/// coverage is in [0.0, 1.0], all 3 children appear in children_coverage.
///
/// Requires the `embeddings` feature AND model files present at
/// `~/.ail/models/all-MiniLM-L6-v2/`. The test is `#[ignore]` so it never
/// blocks CI; run it explicitly with `cargo test -- --ignored`.
#[cfg(feature = "embeddings")]
#[ignore]
#[test]
fn t151_mcp_review_wallet_with_children_full_schema_embeddings() {
    // Skip gracefully when the ONNX model files are absent.
    if ail_search::OnnxEmbeddingProvider::ensure_model().is_err() {
        eprintln!("[skip] ONNX model unavailable — skipping embeddings-strict test");
        return;
    }

    let mut graph = parse_wallet_graph();
    let transfer_id = resolve_transfer_money(&graph);
    let child_ids = attach_three_children(&mut graph, transfer_id);

    let server = memory_server(graph);
    let req = tools_call("ail.review", json!({"node": transfer_id.to_string()}));
    let resp = server.handle(req).unwrap();

    assert!(
        resp.error.is_none(),
        "ail.review must not return a JSON-RPC error; got: {:?}",
        resp.error
    );

    let result = resp.result.unwrap();

    // With real ONNX embeddings status must be a scored value (not Unavailable).
    let status = result["status"].as_str().unwrap_or("");
    assert!(
        matches!(status, "Full" | "Partial" | "Weak"),
        "status must be Full/Partial/Weak with embeddings available; got: {status}"
    );

    // coverage must be a finite f64 in [0.0, 1.0].
    let coverage = result["coverage"]
        .as_f64()
        .expect("coverage must be a number with embeddings");
    assert!(
        (0.0..=1.0).contains(&coverage),
        "coverage must be in [0.0, 1.0]; got: {coverage}"
    );

    // children_coverage must have exactly 3 entries.
    let children = result["children_coverage"]
        .as_array()
        .expect("children_coverage must be a JSON array");
    assert_eq!(
        children.len(),
        3,
        "children_coverage must have 3 entries; got {}",
        children.len()
    );

    // Each child entry is checked strictly.
    let child_id_strings: Vec<String> = child_ids.iter().map(|id| id.to_string()).collect();
    for (i, item) in children.iter().enumerate() {
        let item_node_id = item["node_id"]
            .as_str()
            .unwrap_or_else(|| panic!("children_coverage[{i}].node_id must be a string"));
        assert!(
            child_id_strings.contains(&item_node_id.to_string()),
            "children_coverage[{i}].node_id '{item_node_id}' not among expected child ids"
        );

        let preview = item["intent_preview"].as_str().unwrap_or_else(|| {
            panic!("children_coverage[{i}].intent_preview must be a non-null string")
        });
        assert!(
            !preview.is_empty(),
            "children_coverage[{i}].intent_preview must not be empty"
        );

        let contribution = item["contribution"]
            .as_f64()
            .unwrap_or_else(|| panic!("children_coverage[{i}].contribution must be f64"));
        assert!(
            contribution.is_finite(),
            "children_coverage[{i}].contribution must be finite; got: {contribution}"
        );
    }

    // missing must be a valid array; entries (if any) have the required fields.
    let missing = result["missing"]
        .as_array()
        .expect("missing must be a JSON array");
    for (i, item) in missing.iter().enumerate() {
        assert!(
            item["concept"].as_str().is_some(),
            "missing[{i}].concept must be a string"
        );
        let similarity = item["similarity"]
            .as_f64()
            .unwrap_or_else(|| panic!("missing[{i}].similarity must be f64"));
        assert!(
            similarity.is_finite(),
            "missing[{i}].similarity must be finite; got: {similarity}"
        );
    }

    // suggestion must be non-empty.
    let suggestion = result["suggestion"].as_str().unwrap_or("");
    assert!(!suggestion.is_empty(), "suggestion must not be empty");
}
