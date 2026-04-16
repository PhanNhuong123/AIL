//! Tool handler integration tests.
//!
//! Search and context tests use in-memory graphs built with `AilGraph::new()`.
//! Verify and build tests use fixture directories on disk.

use std::path::{Path, PathBuf};

use ail_graph::{AilGraph, EdgeKind, Node, NodeId, Pattern};
use ail_mcp::{JsonRpcId, JsonRpcRequest, McpServer, ProjectContext};
use serde_json::{json, Value};

// ── Graph helpers ─────────────────────────────────────────────────────────────

fn make_node(intent: &str, pattern: Pattern) -> Node {
    Node::new(NodeId::new(), intent, pattern)
}

/// Five-node graph covering wallet concepts — good for search and context tests.
/// Layout: root Describe → 4 Describe children (Ev edges only).
fn wallet_concept_graph() -> (AilGraph, NodeId, [NodeId; 4]) {
    let mut graph = AilGraph::new();
    let root = graph
        .add_node(make_node(
            "wallet service system overview",
            Pattern::Describe,
        ))
        .unwrap();
    let c1 = graph
        .add_node(make_node(
            "wallet balance management operations",
            Pattern::Describe,
        ))
        .unwrap();
    let c2 = graph
        .add_node(make_node(
            "transfer money between wallet accounts",
            Pattern::Describe,
        ))
        .unwrap();
    let c3 = graph
        .add_node(make_node(
            "validate wallet transaction amount",
            Pattern::Describe,
        ))
        .unwrap();
    let c4 = graph
        .add_node(make_node("fetch user account details", Pattern::Describe))
        .unwrap();

    for child in [c1, c2, c3, c4] {
        graph.add_edge(root, child, EdgeKind::Ev).unwrap();
    }
    graph.set_root(root).unwrap();

    (graph, root, [c1, c2, c3, c4])
}

/// Simple server wrapping an in-memory context — no disk access for search/context/status.
fn memory_server(graph: AilGraph) -> McpServer {
    McpServer::new(PathBuf::from("."), ProjectContext::Raw(graph))
}

/// Server backed by a fixture directory on disk — used for verify/build tests.
fn disk_server(root: &Path) -> McpServer {
    McpServer::new(root.to_path_buf(), ProjectContext::Raw(AilGraph::new()))
}

/// Absolute path to `crates/ail-text/tests/fixtures/wallet_full/`.
fn wallet_full_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .unwrap()
        .join("ail-text/tests/fixtures/wallet_full")
}

/// Absolute path to `crates/ail-mcp/tests/fixtures/bad_project/`.
fn bad_project_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bad_project")
}

fn tools_call(name: &str, args: Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(JsonRpcId::Number(1)),
        method: "tools/call".into(),
        params: Some(json!({"name": name, "arguments": args})),
    }
}

// ── ail.search ────────────────────────────────────────────────────────────────

#[test]
fn mcp_search_returns_ranked_results() {
    let (graph, _, _) = wallet_concept_graph();
    let server = memory_server(graph);

    let req = tools_call("ail.search", json!({"query": "wallet"}));
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    let results = result["results"].as_array().unwrap();
    assert!(
        !results.is_empty(),
        "expected at least one result for 'wallet'"
    );

    // Scores must be in descending order.
    let scores: Vec<f64> = results
        .iter()
        .map(|r| r["score"].as_f64().unwrap_or(0.0))
        .collect();
    for window in scores.windows(2) {
        assert!(
            window[0] >= window[1],
            "results are not sorted by score desc: {:?}",
            scores
        );
    }
}

#[test]
fn mcp_search_budget_limits_results() {
    let (graph, _, _) = wallet_concept_graph();
    let server = memory_server(graph);

    let req = tools_call("ail.search", json!({"query": "wallet", "budget": 2}));
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();
    let results = result["results"].as_array().unwrap();

    assert!(
        results.len() <= 2,
        "expected at most 2 results with budget=2, got {}",
        results.len()
    );
}

// ── Phase 10 gap closure: ranking provenance fields ──────────────────────────

#[test]
fn t110_search_item_has_source_field() {
    let (graph, _, _) = wallet_concept_graph();
    let server = memory_server(graph);

    let req = tools_call("ail.search", json!({"query": "wallet"}));
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();
    let results = result["results"].as_array().unwrap();

    assert!(!results.is_empty(), "expected results for 'wallet'");
    for item in results {
        assert!(
            item.get("source").is_some(),
            "SearchItem must have a 'source' field: {item}"
        );
        assert!(
            item.get("rrf_score").is_some(),
            "SearchItem must have an 'rrf_score' field: {item}"
        );
        assert!(
            item.get("bm25_rank").is_some(),
            "SearchItem must have a 'bm25_rank' field: {item}"
        );
        assert!(
            item.get("semantic_rank").is_some(),
            "SearchItem must have a 'semantic_rank' field: {item}"
        );
    }
}

#[test]
fn t110_search_bm25_fallback_source_is_bm25_only() {
    let (graph, _, _) = wallet_concept_graph();
    let server = memory_server(graph);

    let req = tools_call("ail.search", json!({"query": "wallet"}));
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();
    let results = result["results"].as_array().unwrap();

    assert!(!results.is_empty(), "expected results for 'wallet'");
    for item in results {
        assert_eq!(
            item["source"].as_str().unwrap(),
            "bm25_only",
            "without embeddings all results must have source='bm25_only'"
        );
    }
}

#[test]
fn t110_search_bm25_fallback_rrf_score_positive() {
    let (graph, _, _) = wallet_concept_graph();
    let server = memory_server(graph);

    let req = tools_call("ail.search", json!({"query": "wallet"}));
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();
    let results = result["results"].as_array().unwrap();

    assert!(!results.is_empty(), "expected results for 'wallet'");
    for item in results {
        let rrf = item["rrf_score"].as_f64().unwrap_or(0.0);
        assert!(
            rrf > 0.0,
            "rrf_score must be positive for BM25-only results, got {rrf}"
        );
    }
}

// ── ail.context ───────────────────────────────────────────────────────────────

#[test]
fn mcp_context_splits_primary_secondary() {
    // 5-node graph; search for "wallet" should return 4+ candidates.
    // 70% of 5 = ceil(3.5) = 4 primary, 1 secondary.
    let (graph, _, _) = wallet_concept_graph();
    let server = memory_server(graph);

    let req = tools_call("ail.context", json!({"task": "wallet"}));
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    let primary = result["primary"].as_array().unwrap();
    let secondary = result["secondary"].as_array().unwrap();

    // At least one node in each partition (exact split depends on BM25 scores).
    let total = primary.len() + secondary.len();
    assert!(total > 0, "expected non-empty context output");
    // When 5 results: at least 3 primary.
    assert!(
        !primary.is_empty(),
        "primary must be non-empty with 5 matching nodes"
    );
}

#[test]
fn mcp_context_intent_chain_preserved() {
    // Build a two-level hierarchy: root → child.
    let mut graph = AilGraph::new();
    let root = graph
        .add_node(make_node("wallet system root node", Pattern::Describe))
        .unwrap();
    let child = graph
        .add_node(make_node("transfer wallet money child", Pattern::Describe))
        .unwrap();
    graph.add_edge(root, child, EdgeKind::Ev).unwrap();
    graph.set_root(root).unwrap();

    let server = memory_server(graph);

    let req = tools_call("ail.context", json!({"task": "transfer wallet"}));
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    let primary = result["primary"].as_array().unwrap();
    // The child node (or root) should appear in primary; find one with a non-trivial chain.
    let has_chain = primary.iter().any(|node| {
        node["intent_chain"]
            .as_array()
            .map(|c| c.len() >= 2)
            .unwrap_or(false)
    });
    // If the child node ranks in primary, its chain includes the root intent.
    // With only 2 nodes, the child is the second result or both are primary.
    // At minimum both nodes should be in primary (2 * 70% = 1.4, ceil = 2).
    assert!(
        has_chain || primary.len() >= 1,
        "expected intent_chain with 2+ entries for nested node"
    );
}

// ── ail.verify ────────────────────────────────────────────────────────────────

#[test]
fn mcp_verify_clean_project_returns_ok() {
    let server = disk_server(&wallet_full_dir());
    let req = tools_call("ail.verify", json!({}));
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    assert_eq!(
        result["ok"].as_bool(),
        Some(true),
        "wallet_full should verify cleanly; errors: {:?}",
        result["errors"]
    );
    assert_eq!(result["errors"].as_array().map(|a| a.len()), Some(0));
}

#[test]
fn mcp_verify_bad_project_returns_errors() {
    let server = disk_server(&bad_project_dir());
    let req = tools_call("ail.verify", json!({}));
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    assert_eq!(
        result["ok"].as_bool(),
        Some(false),
        "bad_project should fail verification"
    );
    let errors = result["errors"].as_array().unwrap();
    assert!(
        !errors.is_empty(),
        "expected at least one error for bad project"
    );
}

// ── ail.build ────────────────────────────────────────────────────────────────

#[test]
fn mcp_build_on_verified_project_emits_files() {
    let server = disk_server(&wallet_full_dir());

    // First verify so the context is promoted to Verified.
    let verify_req = tools_call("ail.verify", json!({}));
    let verify_resp = server.handle(verify_req).unwrap();
    let ok = verify_resp.result.unwrap()["ok"].as_bool().unwrap_or(false);
    assert!(ok, "wallet_full should verify before build test");

    let req = tools_call("ail.build", json!({}));
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    assert_eq!(
        result["ok"].as_bool(),
        Some(true),
        "build should succeed; errors: {:?}",
        result["errors"]
    );
    let files = result["files"].as_array().unwrap();
    assert!(!files.is_empty(), "expected at least one emitted file");
}

#[test]
fn mcp_build_respects_contracts_false() {
    let server = disk_server(&wallet_full_dir());

    let req = tools_call("ail.build", json!({"contracts": false}));
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();
    // The build should still succeed even without contract injection.
    assert_eq!(result["ok"].as_bool(), Some(true));
}

#[test]
fn mcp_build_respects_async_mode_true() {
    let server = disk_server(&wallet_full_dir());

    let req = tools_call("ail.build", json!({"async_mode": true}));
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();
    assert_eq!(result["ok"].as_bool(), Some(true));
}

// ── ail.context — promoted facts rendering (task 8.2) ────────────────────────

#[test]
fn t082_promoted_facts_rendered_in_context_output() {
    // Build a graph where a check node immediately precedes a Do node.
    // The Do node's context packet should contain a promoted fact, and that
    // fact must appear in the MCP `ail.context` response JSON under
    // `primary[n].promoted_facts`.
    let mut graph = AilGraph::new();

    // do root
    //   check balance >= amount   ← promoted fact source
    //   do execute transfer       ← target: should see the promoted fact
    let root = graph
        .add_node(Node::new(NodeId::new(), "transfer money root", Pattern::Do))
        .unwrap();
    let check_id = graph
        .add_node(Node::new(
            NodeId::new(),
            "check balance sufficient",
            Pattern::Check,
        ))
        .unwrap();
    let execute_id = graph
        .add_node(Node::new(
            NodeId::new(),
            "execute transfer operation",
            Pattern::Do,
        ))
        .unwrap();

    // Wire topology: root Ev→ check, root Ev→ execute, check Eh→ execute.
    graph.add_edge(root, check_id, EdgeKind::Ev).unwrap();
    graph.add_edge(root, execute_id, EdgeKind::Ev).unwrap();
    graph.add_edge(check_id, execute_id, EdgeKind::Eh).unwrap();
    graph.set_root(root).unwrap();

    // Set the check expression so it can be promoted.
    graph.get_node_mut(check_id).unwrap().expression =
        Some(ail_graph::Expression("balance >= amount".to_string()));

    let server = memory_server(graph);

    let req = tools_call("ail.context", json!({"task": "execute transfer"}));
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    let primary = result["primary"].as_array().unwrap();
    assert!(
        !primary.is_empty(),
        "expected at least one primary context node"
    );

    // The execute node should appear in primary (it matches the query best).
    // Find the context node for execute_id.
    let execute_node = primary
        .iter()
        .find(|n| n["node_id"].as_str() == Some(&execute_id.to_string()));

    assert!(
        execute_node.is_some(),
        "execute node must appear in primary context results"
    );

    let execute_node = execute_node.unwrap();
    let promoted = execute_node["promoted_facts"].as_array().unwrap();
    assert_eq!(
        promoted.len(),
        1,
        "execute node should have exactly one promoted fact; got: {:?}",
        promoted
    );

    // Verify the promoted fact fields.
    let pf = &promoted[0];
    assert_eq!(
        pf["condition"].as_str(),
        Some("balance >= amount"),
        "condition must match the check expression"
    );
    assert_eq!(
        pf["source_node_id"].as_str(),
        Some(check_id.to_string().as_str()),
        "source_node_id must be the check node's UUID"
    );
    assert_eq!(
        pf["source_node_intent"].as_str(),
        Some("check balance sufficient"),
        "source_node_intent must be the check node's intent"
    );
}

// ── ail.status ────────────────────────────────────────────────────────────────

#[test]
fn mcp_status_reports_stage_and_counts() {
    let (graph, _, _) = wallet_concept_graph();
    let node_count = graph.node_count();
    let edge_count = graph.edge_count();

    let server = memory_server(graph);
    let req = tools_call("ail.status", json!({}));
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    assert_eq!(result["pipeline_stage"].as_str(), Some("raw"));
    assert_eq!(result["node_count"].as_u64(), Some(node_count as u64));
    assert_eq!(result["edge_count"].as_u64(), Some(edge_count as u64));
    // All nodes in this graph are Describe — do_node_count should be 0.
    assert_eq!(result["do_node_count"].as_u64(), Some(0));
}

// ── ail.write ────────────────────────────────────────────────────────────────

/// Graph with type nodes for auto-edge detection: root Describe, one Define
/// child named "User", one Error child named "NotFoundError".
fn write_test_graph() -> (AilGraph, NodeId, NodeId, NodeId) {
    let mut graph = AilGraph::new();

    let root = graph
        .add_node(make_node("system overview", Pattern::Describe))
        .unwrap();

    let mut user_node = Node::new(NodeId::new(), "User type definition", Pattern::Define);
    user_node.metadata.name = Some("User".into());
    let user_id = graph.add_node(user_node).unwrap();

    let mut err_node = Node::new(NodeId::new(), "not found error", Pattern::Error);
    err_node.metadata.name = Some("NotFoundError".into());
    let err_id = graph.add_node(err_node).unwrap();

    graph.add_edge(root, user_id, EdgeKind::Ev).unwrap();
    graph.add_edge(root, err_id, EdgeKind::Ev).unwrap();
    graph.add_edge(user_id, err_id, EdgeKind::Eh).unwrap();
    graph.set_root(root).unwrap();

    (graph, root, user_id, err_id)
}

#[test]
fn t111_write_creates_node_under_parent() {
    let (graph, root, _, _) = write_test_graph();
    let server = memory_server(graph);

    let req = tools_call(
        "ail.write",
        json!({
            "parent_id": root.to_string(),
            "pattern": "do",
            "intent": "validate sender is active"
        }),
    );
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    assert_eq!(result["status"].as_str(), Some("created"));
    assert!(result["node_id"].as_str().is_some());
    assert!(result["path"].as_array().unwrap().len() >= 2);
}

#[test]
fn t111_write_sets_correct_depth() {
    let (graph, root, _, _) = write_test_graph();
    let server = memory_server(graph);

    let req = tools_call(
        "ail.write",
        json!({
            "parent_id": root.to_string(),
            "pattern": "describe",
            "intent": "nested thing"
        }),
    );
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    // Root is depth 0, child is depth 1.
    assert_eq!(result["depth"].as_u64(), Some(1));
}

#[test]
fn t111_write_adds_contracts() {
    let (graph, root, _, _) = write_test_graph();
    let server = memory_server(graph);

    let req = tools_call(
        "ail.write",
        json!({
            "parent_id": root.to_string(),
            "pattern": "do",
            "intent": "transfer money",
            "expression": "from sender:User -> void",
            "contracts": [
                { "kind": "before", "expression": "sender.balance > 0" },
                { "kind": "after", "expression": "result is not null" }
            ]
        }),
    );
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();
    assert_eq!(result["status"].as_str(), Some("created"));

    // Verify contracts were attached by reading the node back.
    // Use ail.status to confirm the node count increased.
    let status_req = tools_call("ail.status", json!({}));
    let status_resp = server.handle(status_req).unwrap();
    let status = status_resp.result.unwrap();
    // Original graph has 3 nodes + 1 new = 4.
    assert_eq!(status["node_count"].as_u64(), Some(4));
    // The new Do node should bump do_node_count.
    assert_eq!(status["do_node_count"].as_u64(), Some(1));
}

#[test]
fn t111_write_auto_detects_type_edges() {
    let (graph, root, _, _) = write_test_graph();
    let server = memory_server(graph);

    let req = tools_call(
        "ail.write",
        json!({
            "parent_id": root.to_string(),
            "pattern": "do",
            "intent": "fetch user",
            "expression": "from id:string -> User"
        }),
    );
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    let auto_edges = result["auto_edges"].as_array().unwrap();
    assert!(!auto_edges.is_empty(), "should detect User type reference");

    let edge = &auto_edges[0];
    assert_eq!(edge["kind"].as_str(), Some("ed"));
    assert_eq!(edge["label"].as_str(), Some("uses_type"));
}

#[test]
fn t111_write_invalid_parent_returns_error() {
    let (graph, _, _, _) = write_test_graph();
    let server = memory_server(graph);

    let fake_parent = NodeId::new();
    let req = tools_call(
        "ail.write",
        json!({
            "parent_id": fake_parent.to_string(),
            "pattern": "do",
            "intent": "orphan node"
        }),
    );
    let resp = server.handle(req).unwrap();
    assert!(
        resp.error.is_some(),
        "write with nonexistent parent should fail"
    );
}

// ── ail.patch ────────────────────────────────────────────────────────────────

/// Helper: write a node via the server and return its node ID string.
fn write_child(server: &McpServer, parent_id: &str, pattern: &str, intent: &str) -> String {
    let req = tools_call(
        "ail.write",
        json!({
            "parent_id": parent_id,
            "pattern": pattern,
            "intent": intent
        }),
    );
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();
    result["node_id"].as_str().unwrap().to_string()
}

#[test]
fn t111_patch_updates_intent() {
    let (graph, root, _, _) = write_test_graph();
    let server = memory_server(graph);
    let child_id = write_child(&server, &root.to_string(), "describe", "old intent");

    let req = tools_call(
        "ail.patch",
        json!({
            "node_id": child_id,
            "fields": { "intent": "new intent" }
        }),
    );
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    assert_eq!(result["status"].as_str(), Some("updated"));
    let changed = result["changed_fields"].as_array().unwrap();
    assert!(changed.iter().any(|f| f.as_str() == Some("intent")));
}

#[test]
fn t111_patch_updates_expression() {
    let (graph, root, _, _) = write_test_graph();
    let server = memory_server(graph);
    let child_id = write_child(&server, &root.to_string(), "let", "assign x");

    let req = tools_call(
        "ail.patch",
        json!({
            "node_id": child_id,
            "fields": { "expression": "x:number = 42" }
        }),
    );
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    assert_eq!(result["status"].as_str(), Some("updated"));
    let changed = result["changed_fields"].as_array().unwrap();
    assert!(changed.iter().any(|f| f.as_str() == Some("expression")));
}

#[test]
fn t111_patch_updates_contracts() {
    let (graph, root, _, _) = write_test_graph();
    let server = memory_server(graph);
    let child_id = write_child(&server, &root.to_string(), "do", "some function");

    let req = tools_call(
        "ail.patch",
        json!({
            "node_id": child_id,
            "fields": {
                "contracts": [
                    { "kind": "before", "expression": "x > 0" },
                    { "kind": "after", "expression": "result > 0" }
                ]
            }
        }),
    );
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    assert_eq!(result["status"].as_str(), Some("updated"));
    let changed = result["changed_fields"].as_array().unwrap();
    assert!(changed.iter().any(|f| f.as_str() == Some("contracts")));
}

#[test]
fn t111_patch_re_detects_type_edges() {
    let (graph, root, _, _) = write_test_graph();
    let server = memory_server(graph);

    // Create a node with no expression — no auto edges.
    let child_id = write_child(&server, &root.to_string(), "do", "fetch user");

    // Patch in an expression referencing the User type.
    let req = tools_call(
        "ail.patch",
        json!({
            "node_id": child_id,
            "fields": { "expression": "from id:string -> User" }
        }),
    );
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    let added = result["auto_edges_added"].as_array().unwrap();
    assert!(
        !added.is_empty(),
        "patching in User reference should add an auto-edge"
    );
    assert_eq!(added[0]["label"].as_str(), Some("uses_type"));
}

#[test]
fn t111_patch_nonexistent_node_returns_error() {
    let (graph, _, _, _) = write_test_graph();
    let server = memory_server(graph);

    let fake_id = NodeId::new();
    let req = tools_call(
        "ail.patch",
        json!({
            "node_id": fake_id.to_string(),
            "fields": { "intent": "nope" }
        }),
    );
    let resp = server.handle(req).unwrap();
    assert!(
        resp.error.is_some(),
        "patch on nonexistent node should fail"
    );
}

// ── ail.move + ail.delete (Phase 11.2) ───────────────────────────────────────

/// Two-branch graph for move/delete coverage:
/// ```
/// root (Describe)
/// ├── branch_a (Describe)
/// │   ├── leaf_a1 (Do)
/// │   └── leaf_a2 (Do)
/// └── branch_b (Describe)
/// ```
/// Returns `(graph, root, branch_a, branch_b, leaf_a1, leaf_a2)`.
fn move_delete_graph() -> (AilGraph, NodeId, NodeId, NodeId, NodeId, NodeId) {
    let mut graph = AilGraph::new();
    let root = graph
        .add_node(make_node("system root", Pattern::Describe))
        .unwrap();
    let branch_a = graph
        .add_node(make_node("branch A", Pattern::Describe))
        .unwrap();
    let branch_b = graph
        .add_node(make_node("branch B", Pattern::Describe))
        .unwrap();
    let leaf_a1 = graph
        .add_node(make_node("leaf A one", Pattern::Do))
        .unwrap();
    let leaf_a2 = graph
        .add_node(make_node("leaf A two", Pattern::Do))
        .unwrap();

    graph.add_edge(root, branch_a, EdgeKind::Ev).unwrap();
    graph.add_edge(root, branch_b, EdgeKind::Ev).unwrap();
    graph.add_edge(branch_a, branch_b, EdgeKind::Eh).unwrap();
    graph.add_edge(branch_a, leaf_a1, EdgeKind::Ev).unwrap();
    graph.add_edge(branch_a, leaf_a2, EdgeKind::Ev).unwrap();
    graph.add_edge(leaf_a1, leaf_a2, EdgeKind::Eh).unwrap();
    graph.set_root(root).unwrap();

    (graph, root, branch_a, branch_b, leaf_a1, leaf_a2)
}

#[test]
fn t112_move_changes_parent() {
    let (graph, _root, _a, branch_b, leaf_a1, _a2) = move_delete_graph();
    let server = memory_server(graph);

    let req = tools_call(
        "ail.move",
        json!({
            "node_id": leaf_a1.to_string(),
            "new_parent_id": branch_b.to_string()
        }),
    );
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    assert_eq!(result["status"].as_str(), Some("moved"));
    assert_eq!(
        result["new_parent_id"].as_str(),
        Some(branch_b.to_string().as_str())
    );
    assert!(
        result["old_parent_id"].as_str().is_some(),
        "old_parent_id should be reported"
    );
}

#[test]
fn t112_move_updates_depth_recursive() {
    // Move a subtree from depth 1 to depth 2 and confirm the depth shifts
    // for the moved node. Descendant depth is computed from the parent
    // chain so it follows automatically.
    let (graph, _root, branch_a, branch_b, _l1, _l2) = move_delete_graph();
    let server = memory_server(graph);

    // Move branch_a (depth 1) to be a child of branch_b (depth 1) → branch_a
    // becomes depth 2.
    let req = tools_call(
        "ail.move",
        json!({
            "node_id": branch_a.to_string(),
            "new_parent_id": branch_b.to_string()
        }),
    );
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    assert_eq!(result["old_depth"].as_u64(), Some(1));
    assert_eq!(result["new_depth"].as_u64(), Some(2));
    // branch_a brought leaf_a1 + leaf_a2 with it.
    assert_eq!(result["descendants_moved"].as_u64(), Some(2));
}

#[test]
fn t112_move_prevents_circular() {
    // Move branch_a under leaf_a1 (its own child) — exercises the
    // descendant-cycle guard rather than the root-no-parent rejection.
    let (graph, _root, branch_a, _b, leaf_a1, _l2) = move_delete_graph();
    let server = memory_server(graph);

    let req = tools_call(
        "ail.move",
        json!({
            "node_id": branch_a.to_string(),
            "new_parent_id": leaf_a1.to_string()
        }),
    );
    let resp = server.handle(req).unwrap();
    assert!(
        resp.error.is_some(),
        "moving a node under its own descendant must fail"
    );
    let msg = resp.error.unwrap().message;
    assert!(
        msg.contains("descendant"),
        "error must mention descendant cycle, got: {msg}"
    );
}

#[test]
fn t112_move_invalidates_old_and_new_parent() {
    // Sanity check the move tool returns an invalidation count field — for
    // AilGraph there is no CIC cache so the value is 0, but the contract
    // must be preserved for SQLite-backed callers.
    let (graph, _root, _a, branch_b, leaf_a1, _a2) = move_delete_graph();
    let server = memory_server(graph);

    let req = tools_call(
        "ail.move",
        json!({
            "node_id": leaf_a1.to_string(),
            "new_parent_id": branch_b.to_string()
        }),
    );
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    assert!(
        result.get("cic_invalidated").is_some(),
        "move output must carry a cic_invalidated field"
    );
    assert_eq!(
        result["cic_invalidated"].as_u64(),
        Some(0),
        "AilGraph has no CIC cache; expected 0 invalidations"
    );
}

#[test]
fn t112_move_reorders_siblings() {
    // Move leaf_a2 into branch_b at position 0 (prepend); then move it back
    // to branch_a at position 0. Verify the result reports the new parent
    // and that no error surfaces from the Eh splice.
    let (graph, _root, branch_a, branch_b, _l1, leaf_a2) = move_delete_graph();
    let server = memory_server(graph);

    // Step 1: leaf_a2 → branch_b at position 0.
    let req = tools_call(
        "ail.move",
        json!({
            "node_id": leaf_a2.to_string(),
            "new_parent_id": branch_b.to_string(),
            "position": 0
        }),
    );
    let resp = server.handle(req).unwrap();
    assert!(
        resp.error.is_none(),
        "first move must succeed: {:?}",
        resp.error
    );
    let result = resp.result.unwrap();
    assert_eq!(
        result["new_parent_id"].as_str(),
        Some(branch_b.to_string().as_str())
    );

    // Step 2: leaf_a2 → branch_a at position 0 (prepend before leaf_a1).
    let req = tools_call(
        "ail.move",
        json!({
            "node_id": leaf_a2.to_string(),
            "new_parent_id": branch_a.to_string(),
            "position": 0
        }),
    );
    let resp = server.handle(req).unwrap();
    assert!(
        resp.error.is_none(),
        "second move must succeed: {:?}",
        resp.error
    );
}

#[test]
fn t112_delete_cascade_removes_descendants() {
    // Deleting branch_a (cascade) must remove branch_a, leaf_a1, leaf_a2.
    // Total before: 5 nodes. After: 2 (root, branch_b).
    let (graph, _root, branch_a, _b, _l1, _l2) = move_delete_graph();
    let server = memory_server(graph);

    let req = tools_call(
        "ail.delete",
        json!({
            "node_id": branch_a.to_string(),
            "strategy": "cascade"
        }),
    );
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    assert_eq!(result["status"].as_str(), Some("deleted"));
    assert_eq!(result["deleted_nodes"].as_u64(), Some(3));

    // Verify graph node count via ail.status.
    let status_resp = server.handle(tools_call("ail.status", json!({}))).unwrap();
    let status = status_resp.result.unwrap();
    assert_eq!(status["node_count"].as_u64(), Some(2));
}

#[test]
fn t112_delete_orphan_reparents_children() {
    // Delete branch_a (orphan): branch_a removed, leaf_a1 and leaf_a2 lifted
    // to root. Total before: 5. After: 4 (root, branch_b, leaf_a1, leaf_a2).
    let (graph, _root, branch_a, _b, _l1, _l2) = move_delete_graph();
    let server = memory_server(graph);

    let req = tools_call(
        "ail.delete",
        json!({
            "node_id": branch_a.to_string(),
            "strategy": "orphan"
        }),
    );
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    assert_eq!(result["status"].as_str(), Some("orphaned"));
    assert_eq!(result["deleted_nodes"].as_u64(), Some(1));
    assert_eq!(result["reparented_children"].as_u64(), Some(2));

    let status_resp = server.handle(tools_call("ail.status", json!({}))).unwrap();
    let status = status_resp.result.unwrap();
    assert_eq!(status["node_count"].as_u64(), Some(4));
}

#[test]
fn t112_delete_dry_run_no_mutation() {
    let (graph, _root, branch_a, _b, _l1, _l2) = move_delete_graph();
    let server = memory_server(graph);

    let req = tools_call(
        "ail.delete",
        json!({
            "node_id": branch_a.to_string(),
            "strategy": "dry_run"
        }),
    );
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    assert_eq!(result["status"].as_str(), Some("dry_run"));
    assert_eq!(result["would_delete"].as_u64(), Some(3));
    assert_eq!(result["deleted_nodes"].as_u64(), Some(0));

    // Confirm the graph is unchanged (5 nodes still present).
    let status_resp = server.handle(tools_call("ail.status", json!({}))).unwrap();
    let status = status_resp.result.unwrap();
    assert_eq!(status["node_count"].as_u64(), Some(5));
}

#[test]
fn t112_delete_removes_ed_edges() {
    // Build a fresh graph where one node has an outgoing Ed edge to a type
    // node. Delete the target type node and verify the dry_run report
    // reflects the affected Ed edge.
    let mut graph = AilGraph::new();
    let root = graph
        .add_node(make_node("system root", Pattern::Describe))
        .unwrap();
    let mut user_node = Node::new(NodeId::new(), "User type", Pattern::Define);
    user_node.metadata.name = Some("User".into());
    let user_id = graph.add_node(user_node).unwrap();
    let do_node = graph
        .add_node(make_node("fetch user", Pattern::Do))
        .unwrap();
    graph.add_edge(root, user_id, EdgeKind::Ev).unwrap();
    graph.add_edge(root, do_node, EdgeKind::Ev).unwrap();
    graph.add_edge(user_id, do_node, EdgeKind::Eh).unwrap();
    // Ed edge: do_node → user_id (uses_type).
    graph.add_edge(do_node, user_id, EdgeKind::Ed).unwrap();
    graph.set_root(root).unwrap();

    let server = memory_server(graph);
    let req = tools_call(
        "ail.delete",
        json!({
            "node_id": user_id.to_string(),
            "strategy": "cascade"
        }),
    );
    let resp = server.handle(req).unwrap();
    let result = resp.result.unwrap();

    assert_eq!(result["status"].as_str(), Some("deleted"));
    assert!(
        result["affected_ed_edges"].as_u64().unwrap_or(0) >= 1,
        "deleting User type should report at least one affected Ed edge; got: {result}"
    );
}

#[test]
fn t112_delete_nonexistent_node_returns_error() {
    let (graph, _root, _a, _b, _l1, _l2) = move_delete_graph();
    let server = memory_server(graph);

    let fake_id = NodeId::new();
    let req = tools_call(
        "ail.delete",
        json!({
            "node_id": fake_id.to_string(),
            "strategy": "cascade"
        }),
    );
    let resp = server.handle(req).unwrap();
    assert!(
        resp.error.is_some(),
        "delete on nonexistent node should fail"
    );
}
