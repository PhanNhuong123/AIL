//! AI workflow simulation tests for the MCP server.
//!
//! These tests simulate how an AI assistant would interact with `ail-mcp` in a
//! real development session. Each test corresponds to one step (or set of steps)
//! in the canonical AI workflow:
//!
//! 1. `ail.status`  — read the project index to understand current state
//! 2. `ail.verify`  — load and validate the project graph from disk
//! 3. `ail.search`  — find relevant concept nodes by keyword
//! 4. `ail.context` — retrieve CIC packets for the found nodes
//! 5. Write a new `.ail` file to disk (AI-generated content)
//! 6. `ail.verify`  — re-validate so the new file is picked up
//! 7. `ail.build`   — emit Python artefacts
//!
//! Failure-path tests (7–11) cover calls made in the wrong order or with bad
//! input — exactly the edge cases an AI agent will hit in practice.

use std::fs;
use std::path::{Path, PathBuf};

use ail_graph::AilGraph;
use ail_mcp::{JsonRpcId, JsonRpcRequest, McpServer, ProjectContext};
use serde_json::{json, Value};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Absolute path to the complete `wallet_full` fixture used by ail-text tests.
///
/// Contains 5 `.ail` files: `positive_amount`, `wallet_balance`, `user`,
/// `transfer_result`, and `transfer_money` (a Do node with 3 contracts).
fn wallet_full_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .unwrap()
        .join("ail-text/tests/fixtures/wallet_full")
}

/// Absolute path to the `bad_project` fixture (a Do node with no contracts).
fn bad_project_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bad_project")
}

/// Copy a fixture directory tree into a fresh `TempDir`.
fn copy_fixture_to_temp(src: &Path) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("failed to create TempDir");
    copy_dir_all(src, tmp.path()).expect("fixture copy failed");
    tmp
}

/// Create an MCP server rooted at `root` with a raw (empty) initial context.
///
/// Calling `ail.verify` on this server will trigger `refresh_from_path(root)`,
/// reading every `.ail` file in the directory.
fn fresh_server(root: &Path) -> McpServer {
    McpServer::new(root.to_path_buf(), ProjectContext::Raw(AilGraph::new()))
}

/// Build a `tools/call` JSON-RPC request for the given tool name and arguments.
fn tool_call(name: &str, args: Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(JsonRpcId::Number(1)),
        method: "tools/call".into(),
        params: Some(json!({"name": name, "arguments": args})),
    }
}

/// Recursively copy directory `src` into `dst`.
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

// ── Happy-path workflow tests ─────────────────────────────────────────────────

/// Step 1 + Step 2: verify loads the project graph; status reflects the new
/// pipeline stage and a non-zero node count.
#[test]
fn ai_workflow_verify_loads_project_graph() {
    let server = fresh_server(&wallet_full_dir());

    // Before verify the context is Raw — status shows "raw".
    let before = server
        .handle(tool_call("ail.status", json!({})))
        .unwrap()
        .result
        .unwrap();
    assert_eq!(before["pipeline_stage"].as_str(), Some("raw"));
    assert_eq!(before["node_count"].as_u64(), Some(0));

    // After verify the context becomes Verified — status shows "verified" with nodes.
    let verify = server
        .handle(tool_call("ail.verify", json!({})))
        .unwrap()
        .result
        .unwrap();
    assert!(
        verify["ok"].as_bool().unwrap_or(false),
        "wallet_full should verify cleanly; errors: {:?}",
        verify["errors"]
    );

    let after = server
        .handle(tool_call("ail.status", json!({})))
        .unwrap()
        .result
        .unwrap();
    assert_eq!(after["pipeline_stage"].as_str(), Some("verified"));
    assert!(
        after["node_count"].as_u64().unwrap_or(0) > 0,
        "node_count must be non-zero after loading wallet_full"
    );
}

/// Step 3: search after verify finds concept nodes by keyword.
#[test]
fn ai_workflow_search_after_verify_finds_concepts() {
    let server = fresh_server(&wallet_full_dir());
    server.handle(tool_call("ail.verify", json!({}))).unwrap(); // load graph

    let resp = server
        .handle(tool_call("ail.search", json!({"query": "wallet"})))
        .unwrap()
        .result
        .unwrap();

    let results = resp["results"].as_array().unwrap();
    assert!(
        !results.is_empty(),
        "search for 'wallet' should return at least one result on verified graph"
    );

    // Results must be in descending score order.
    let scores: Vec<f64> = results
        .iter()
        .map(|r| r["score"].as_f64().unwrap_or(0.0))
        .collect();
    for window in scores.windows(2) {
        assert!(
            window[0] >= window[1],
            "search results must be sorted by score descending: {:?}",
            scores
        );
    }
}

/// Step 4: context returns primary packets with intent fields present.
#[test]
fn ai_workflow_context_returns_primary_and_secondary() {
    let server = fresh_server(&wallet_full_dir());
    server.handle(tool_call("ail.verify", json!({}))).unwrap();

    let resp = server
        .handle(tool_call(
            "ail.context",
            json!({"task": "transfer money between wallets"}),
        ))
        .unwrap()
        .result
        .unwrap();

    let primary = resp["primary"].as_array().unwrap();
    assert!(
        !primary.is_empty(),
        "context 'transfer money' must return at least one primary packet"
    );

    // Every primary node should carry an intent field.
    for node in primary {
        assert!(
            node.get("intent").is_some(),
            "primary packet missing 'intent' field: {:?}",
            node
        );
    }
}

/// Step 5 (write) + Step 6 (re-verify): the AI writes a new type definition file
/// to the project directory; the subsequent verify call picks it up successfully.
#[test]
fn ai_workflow_write_new_ail_then_reverify_succeeds() {
    let tmp = copy_fixture_to_temp(&wallet_full_dir());
    let server = fresh_server(tmp.path());

    // Baseline verify.
    let first = server
        .handle(tool_call("ail.verify", json!({})))
        .unwrap()
        .result
        .unwrap();
    assert!(
        first["ok"].as_bool().unwrap_or(false),
        "baseline verify should succeed"
    );

    // AI writes a new valid type definition.
    fs::write(
        tmp.path().join("fee_amount.ail"),
        "define FeeAmount:number where value >= 0\n",
    )
    .expect("write fee_amount.ail");

    // Re-verify — refresh_from_path re-reads all .ail files including the new one.
    let second = server
        .handle(tool_call("ail.verify", json!({})))
        .unwrap()
        .result
        .unwrap();
    assert!(
        second["ok"].as_bool().unwrap_or(false),
        "verify should succeed after adding a valid .ail file; errors: {:?}",
        second["errors"]
    );
}

/// Step 7 (build): build after verify returns ok and lists Python output paths.
#[test]
fn ai_workflow_build_after_verify_emits_python_paths() {
    let server = fresh_server(&wallet_full_dir());
    server.handle(tool_call("ail.verify", json!({}))).unwrap();

    let resp = server
        .handle(tool_call("ail.build", json!({})))
        .unwrap()
        .result
        .unwrap();

    assert!(
        resp["ok"].as_bool().unwrap_or(false),
        "build should succeed on verified wallet_full; errors: {:?}",
        resp["errors"]
    );

    let files = resp["files"].as_array().unwrap();
    assert!(
        !files.is_empty(),
        "build must return at least one emitted file"
    );

    let paths: Vec<&str> = files.iter().filter_map(|f| f["path"].as_str()).collect();
    assert!(
        paths.iter().any(|p| p.contains("types.py")),
        "expected 'types.py' in build output; got: {:?}",
        paths
    );
    assert!(
        paths.iter().any(|p| p.contains("functions.py")),
        "expected 'functions.py' in build output; got: {:?}",
        paths
    );
}

/// Complete AI session: status (raw) → verify → search → context → write → re-verify → build.
///
/// This is the canonical "AI adds a new type to an existing project" workflow.
#[test]
fn ai_workflow_full_session() {
    let tmp = copy_fixture_to_temp(&wallet_full_dir());
    let server = fresh_server(tmp.path());

    // 1. Status before loading: raw stage, 0 nodes.
    let status0 = server
        .handle(tool_call("ail.status", json!({})))
        .unwrap()
        .result
        .unwrap();
    assert_eq!(status0["pipeline_stage"].as_str(), Some("raw"));

    // 2. Verify to load the project.
    let verify1 = server
        .handle(tool_call("ail.verify", json!({})))
        .unwrap()
        .result
        .unwrap();
    assert!(verify1["ok"].as_bool().unwrap_or(false));

    // 3. Search for relevant concepts.
    let search = server
        .handle(tool_call("ail.search", json!({"query": "transfer"})))
        .unwrap()
        .result
        .unwrap();
    assert!(
        !search["results"].as_array().unwrap().is_empty(),
        "search for 'transfer' should find nodes"
    );

    // 4. Retrieve CIC context for the task.
    let ctx = server
        .handle(tool_call("ail.context", json!({"task": "transfer money"})))
        .unwrap()
        .result
        .unwrap();
    assert!(
        !ctx["primary"].as_array().unwrap().is_empty(),
        "context must return primary packets"
    );

    // 5. AI writes a new type definition based on the context.
    fs::write(
        tmp.path().join("refund_amount.ail"),
        "define RefundAmount:number where value > 0\n",
    )
    .expect("write refund_amount.ail");

    // 6. Re-verify to include the new file.
    let verify2 = server
        .handle(tool_call("ail.verify", json!({})))
        .unwrap()
        .result
        .unwrap();
    assert!(
        verify2["ok"].as_bool().unwrap_or(false),
        "re-verify should succeed after adding refund_amount.ail"
    );

    // 7. Build to emit Python.
    let build = server
        .handle(tool_call("ail.build", json!({})))
        .unwrap()
        .result
        .unwrap();
    assert!(
        build["ok"].as_bool().unwrap_or(false),
        "build should succeed after full AI session"
    );
    assert!(
        !build["files"].as_array().unwrap().is_empty(),
        "build must emit at least one file"
    );
}

// ── Failure-path tests ────────────────────────────────────────────────────────

/// Search on an empty Raw context (before any verify call) returns no results.
///
/// The BM25 index is built from the in-memory graph. When that graph is empty
/// (Raw context, no disk read yet), there are no indexed nodes to match against.
#[test]
fn ai_workflow_search_before_verify_returns_empty() {
    // Server backed by wallet_full on disk but context never loaded (Raw).
    let server = fresh_server(&wallet_full_dir());

    let resp = server
        .handle(tool_call("ail.search", json!({"query": "wallet"})))
        .unwrap()
        .result
        .unwrap();

    let results = resp["results"].as_array().unwrap();
    assert!(
        results.is_empty(),
        "search before verify must return empty results (empty in-memory graph)"
    );
}

/// Build on a project that fails pipeline verification returns errors.
///
/// The bad_project fixture has a Do node with no contracts. When ail.build
/// triggers a pipeline refresh it encounters a validation error.
#[test]
fn ai_workflow_build_on_bad_project_returns_errors() {
    let server = fresh_server(&bad_project_dir());

    let resp = server
        .handle(tool_call("ail.build", json!({})))
        .unwrap()
        .result
        .unwrap();

    assert!(
        !resp["ok"].as_bool().unwrap_or(true),
        "build on bad_project should fail"
    );
    let errors = resp["errors"].as_array().unwrap();
    assert!(
        !errors.is_empty(),
        "build failure must report at least one error"
    );
}

/// Writing a Do node without contracts to the project causes verify to fail.
#[test]
fn ai_workflow_write_bad_ail_then_verify_fails() {
    let tmp = copy_fixture_to_temp(&wallet_full_dir());
    let server = fresh_server(tmp.path());

    // Baseline succeeds.
    let ok = server
        .handle(tool_call("ail.verify", json!({})))
        .unwrap()
        .result
        .unwrap();
    assert!(ok["ok"].as_bool().unwrap_or(false));

    // AI writes a broken Do node (missing required pre/post contracts).
    fs::write(
        tmp.path().join("bad.ail"),
        "do broken function\n  from x:number\n  -> number\n\n  let result:number = x + 1\n",
    )
    .expect("write bad.ail");

    let fail = server
        .handle(tool_call("ail.verify", json!({})))
        .unwrap()
        .result
        .unwrap();
    assert!(
        !fail["ok"].as_bool().unwrap_or(true),
        "verify should fail after adding a Do node without contracts"
    );
    assert!(
        !fail["errors"].as_array().unwrap().is_empty(),
        "errors array must be non-empty on verify failure"
    );
}

/// Verify on an empty directory succeeds.
///
/// `parse_directory` always creates one structural container node (a Describe
/// node) for the root directory itself — even when the directory contains no
/// `.ail` files.  So the graph is not strictly empty; it has exactly 1 node.
/// The important invariant is that verify returns `ok: true` and does not panic
/// or error, and the pipeline stage advances to "verified".
#[test]
fn ai_workflow_verify_on_empty_dir_succeeds() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let server = fresh_server(tmp.path());

    let resp = server
        .handle(tool_call("ail.verify", json!({})))
        .unwrap()
        .result
        .unwrap();

    assert!(
        resp["ok"].as_bool().unwrap_or(false),
        "verify on an empty directory should succeed (container-only graph is valid)"
    );

    // Status must show "verified" stage with the 1 synthetic container node.
    let status = server
        .handle(tool_call("ail.status", json!({})))
        .unwrap()
        .result
        .unwrap();
    assert_eq!(status["pipeline_stage"].as_str(), Some("verified"));
    // node_count is 1 because parse_directory creates a structural container node.
    assert_eq!(status["node_count"].as_u64(), Some(1));
}

/// Context called on a raw/empty graph returns an empty primary array.
///
/// Without nodes in the graph the BM25 search returns nothing, so both the
/// primary and secondary context partitions should be empty.
#[test]
fn ai_workflow_context_before_verify_returns_empty_primary() {
    let server = fresh_server(&wallet_full_dir()); // context never loaded from disk

    let resp = server
        .handle(tool_call("ail.context", json!({"task": "transfer money"})))
        .unwrap()
        .result
        .unwrap();

    let primary = resp["primary"].as_array().unwrap();
    assert!(
        primary.is_empty(),
        "context before verify must return empty primary (no nodes in graph)"
    );
}

/// Regression: `ail.write` then `ail.verify` must preserve the in-memory edit
/// instead of discarding it by re-parsing disk. After verify, the new node
/// must still be in the graph and discoverable via `ail.search`.
#[test]
fn t_write_survives_verify() {
    let tmp = copy_fixture_to_temp(&wallet_full_dir());
    let server = fresh_server(tmp.path());

    server.handle(tool_call("ail.verify", json!({}))).unwrap();

    let search = server
        .handle(tool_call("ail.search", json!({"query": "wallet"})))
        .unwrap()
        .result
        .unwrap();
    let parent_id = search["results"]
        .as_array()
        .and_then(|r| r.first())
        .and_then(|r| r["node_id"].as_str())
        .expect("wallet_full must have at least one search hit")
        .to_owned();

    let unique_intent = "verify-survival sentinel node for phase-11 regression";
    let write_resp = server
        .handle(tool_call(
            "ail.write",
            json!({
                "parent_id": parent_id,
                "pattern": "describe",
                "intent": unique_intent
            }),
        ))
        .unwrap()
        .result
        .unwrap();
    let new_id = write_resp["node_id"].as_str().unwrap().to_owned();

    // The write demoted context to Raw and set dirty=true.
    let status_after_write = server
        .handle(tool_call("ail.status", json!({})))
        .unwrap()
        .result
        .unwrap();
    assert_eq!(status_after_write["pipeline_stage"].as_str(), Some("raw"));

    // Verify must promote to Verified WITHOUT re-parsing disk (which would
    // erase the new node). Bug repro: pre-fix, this promoted to Verified via
    // disk re-parse, silently dropping the sentinel node.
    let verify_resp = server
        .handle(tool_call("ail.verify", json!({})))
        .unwrap()
        .result
        .unwrap();
    assert!(
        verify_resp["ok"].as_bool().unwrap_or(false),
        "verify should succeed after in-memory write; errors: {:?}",
        verify_resp["errors"]
    );

    let status_after_verify = server
        .handle(tool_call("ail.status", json!({})))
        .unwrap()
        .result
        .unwrap();
    assert_eq!(
        status_after_verify["pipeline_stage"].as_str(),
        Some("verified")
    );

    // Proof of survival: search for the sentinel intent.
    let search_after = server
        .handle(tool_call(
            "ail.search",
            json!({"query": "verify-survival sentinel"}),
        ))
        .unwrap()
        .result
        .unwrap();
    let found = search_after["results"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["node_id"].as_str() == Some(&new_id));
    assert!(
        found,
        "search must find the in-memory written node after verify; got: {:?}",
        search_after["results"]
    );
}
