//! [`McpServer`] — the MCP server core.
//!
//! Holds project state and routes JSON-RPC 2.0 requests to the ten tool
//! handlers (five read + five write). The server is single-threaded;
//! mutability is tracked via `RefCell`.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
#[cfg(feature = "embeddings")]
use std::sync::Arc;

use serde_json::{json, Value};

use ail_graph::Bm25Index;
use ail_search::EmbeddingIndex;

use crate::context::ProjectContext;
use crate::tools::{batch, build, context, review, search, status, structure, verify, write};
use crate::types::protocol::{
    JsonRpcError, JsonRpcRequest, JsonRpcResponse, INTERNAL_ERROR, INVALID_PARAMS, METHOD_NOT_FOUND,
};
use crate::types::tool_io::{
    BatchInput, BuildInput, ContextInput, DeleteInput, MoveInput, PatchInput, ReviewInput,
    SearchInput, VerifyInput, WriteInput,
};

/// The MCP server.
///
/// Call [`McpServer::handle`] once per incoming JSON-RPC line.  The server
/// mutates its own context on successful pipeline refreshes; callers must not
/// share the server across threads.
pub struct McpServer {
    project_root: PathBuf,
    context: RefCell<ProjectContext>,
    /// Lazy BM25 index. Built on first `ail.search` call; cleared when the
    /// pipeline is refreshed so the next search reflects the updated graph.
    search_cache: RefCell<Option<Bm25Index>>,
    /// Lazy embedding index. Loaded once from `project_root/*.ail.db` when the
    /// `embeddings` feature is active and model files are present. Always `None`
    /// without the feature or when the DB has no compatible vectors.
    embedding_cache: RefCell<Option<EmbeddingIndex>>,
    /// Cached ONNX embedding provider for `ail.review`.  Built at most once per
    /// server session and reused across calls.  Always `None` without the
    /// `embeddings` feature or when model files are absent.
    #[cfg(feature = "embeddings")]
    review_provider_cache: RefCell<Option<Arc<ail_search::OnnxEmbeddingProvider>>>,
    /// `true` after any write tool mutates the in-memory graph. Verify and
    /// build use this to decide between `refresh_from_graph` (preserve edits)
    /// and `refresh_from_path` (re-parse disk). Cleared on successful refresh.
    dirty: Cell<bool>,
}

impl McpServer {
    /// Create a new server rooted at `project_root` with an initial context.
    pub fn new(project_root: PathBuf, initial: ProjectContext) -> Self {
        Self {
            project_root,
            context: RefCell::new(initial),
            search_cache: RefCell::new(None),
            embedding_cache: RefCell::new(None),
            #[cfg(feature = "embeddings")]
            review_provider_cache: RefCell::new(None),
            dirty: Cell::new(false),
        }
    }

    /// Process one JSON-RPC request and return a response, or `None` for
    /// notifications (requests without an `id`).
    pub fn handle(&self, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
        let id = req.id.clone();
        let is_notification = id.is_none();

        let result: Result<Value, JsonRpcError> = match req.method.as_str() {
            "initialize" => Ok(Self::initialize()),
            "initialized" => return None, // notification — no response
            "tools/list" => Ok(Self::tools_list()),
            "tools/call" => {
                let params = req.params.unwrap_or(Value::Null);
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(Value::Object(serde_json::Map::new()));
                self.dispatch_tool(&name, args)
            }
            _ if is_notification => return None,
            _ => Err(JsonRpcError::new(
                METHOD_NOT_FOUND,
                format!("Method not found: {}", req.method),
            )),
        };

        let resp = match result {
            Ok(v) => JsonRpcResponse::ok(id, v),
            Err(e) => JsonRpcResponse::err(id, e),
        };
        Some(resp)
    }

    // ── Tool dispatch ─────────────────────────────────────────────────────────

    fn dispatch_tool(&self, name: &str, args: Value) -> Result<Value, JsonRpcError> {
        match name {
            "ail.search" => {
                // Feature-gated: lazily populate the embedding index from the
                // project DB + ONNX model. The mut borrow ends before the
                // shared borrow below, avoiding a RefCell panic.
                #[cfg(feature = "embeddings")]
                {
                    let mut emb = self.embedding_cache.borrow_mut();
                    if emb.is_none() {
                        *emb = search::try_load_embedding_index(&self.project_root);
                    }
                }

                let input: SearchInput = serde_json::from_value(args)
                    .map_err(|e| JsonRpcError::new(INVALID_PARAMS, e.to_string()))?;
                let borrow = self.context.borrow();
                let emb = self.embedding_cache.borrow();
                let out =
                    search::run_search(borrow.graph(), &self.search_cache, emb.as_ref(), &input);
                serde_json::to_value(out)
                    .map_err(|e| JsonRpcError::new(INTERNAL_ERROR, e.to_string()))
            }

            "ail.review" => {
                // Lazily build and cache the ONNX provider so subsequent calls
                // skip the expensive model-load path.
                #[cfg(feature = "embeddings")]
                {
                    let mut cache = self.review_provider_cache.borrow_mut();
                    if cache.is_none() {
                        *cache = review::try_build_provider();
                    }
                }

                let input: ReviewInput = serde_json::from_value(args)
                    .map_err(|e| JsonRpcError::new(INVALID_PARAMS, e.to_string()))?;
                let borrow = self.context.borrow();
                #[cfg(feature = "embeddings")]
                let provider_borrow = self.review_provider_cache.borrow();
                #[cfg(feature = "embeddings")]
                let out = review::handle_review(
                    borrow.graph(),
                    input,
                    provider_borrow.as_ref().map(Arc::as_ref),
                );
                #[cfg(not(feature = "embeddings"))]
                let out = review::handle_review(borrow.graph(), input);
                serde_json::to_value(out)
                    .map_err(|e| JsonRpcError::new(INTERNAL_ERROR, e.to_string()))
            }

            "ail.context" => {
                let input: ContextInput = serde_json::from_value(args)
                    .map_err(|e| JsonRpcError::new(INVALID_PARAMS, e.to_string()))?;
                let borrow = self.context.borrow();
                let out = context::run_context(borrow.graph(), &input);
                serde_json::to_value(out)
                    .map_err(|e| JsonRpcError::new(INTERNAL_ERROR, e.to_string()))
            }

            "ail.verify" => {
                let input: VerifyInput = serde_json::from_value(args)
                    .map_err(|e| JsonRpcError::new(INVALID_PARAMS, e.to_string()))?;
                let out = verify::run_verify(
                    &self.project_root,
                    &self.context,
                    &self.search_cache,
                    &self.embedding_cache,
                    &self.dirty,
                    &input,
                );
                serde_json::to_value(out)
                    .map_err(|e| JsonRpcError::new(INTERNAL_ERROR, e.to_string()))
            }

            "ail.build" => {
                let input: BuildInput = serde_json::from_value(args)
                    .map_err(|e| JsonRpcError::new(INVALID_PARAMS, e.to_string()))?;
                let out = build::run_build(
                    &self.project_root,
                    &self.context,
                    &self.search_cache,
                    &self.embedding_cache,
                    &self.dirty,
                    &input,
                );
                serde_json::to_value(out)
                    .map_err(|e| JsonRpcError::new(INTERNAL_ERROR, e.to_string()))
            }

            "ail.status" => {
                let borrow = self.context.borrow();
                let out = status::run_status(&borrow);
                serde_json::to_value(out)
                    .map_err(|e| JsonRpcError::new(INTERNAL_ERROR, e.to_string()))
            }

            "ail.write" => {
                let input: WriteInput = serde_json::from_value(args)
                    .map_err(|e| JsonRpcError::new(INVALID_PARAMS, e.to_string()))?;
                let out = {
                    let mut ctx = self.context.borrow_mut();
                    let graph = ctx.graph_mut();
                    write::run_write(graph, &input)
                };
                self.dirty.set(true);
                // Clear search caches — the graph was mutated.
                *self.search_cache.borrow_mut() = None;
                *self.embedding_cache.borrow_mut() = None;
                match out {
                    Ok(output) => serde_json::to_value(output)
                        .map_err(|e| JsonRpcError::new(INTERNAL_ERROR, e.to_string())),
                    Err(e) => Err(JsonRpcError::new(INVALID_PARAMS, e)),
                }
            }

            "ail.patch" => {
                let input: PatchInput = serde_json::from_value(args)
                    .map_err(|e| JsonRpcError::new(INVALID_PARAMS, e.to_string()))?;
                let out = {
                    let mut ctx = self.context.borrow_mut();
                    let graph = ctx.graph_mut();
                    write::run_patch(graph, &input)
                };
                self.dirty.set(true);
                *self.search_cache.borrow_mut() = None;
                *self.embedding_cache.borrow_mut() = None;
                match out {
                    Ok(output) => serde_json::to_value(output)
                        .map_err(|e| JsonRpcError::new(INTERNAL_ERROR, e.to_string())),
                    Err(e) => Err(JsonRpcError::new(INVALID_PARAMS, e)),
                }
            }

            "ail.move" => {
                let input: MoveInput = serde_json::from_value(args)
                    .map_err(|e| JsonRpcError::new(INVALID_PARAMS, e.to_string()))?;
                let out = {
                    let mut ctx = self.context.borrow_mut();
                    let graph = ctx.graph_mut();
                    structure::run_move(graph, &input)
                };
                self.dirty.set(true);
                *self.search_cache.borrow_mut() = None;
                *self.embedding_cache.borrow_mut() = None;
                match out {
                    Ok(output) => serde_json::to_value(output)
                        .map_err(|e| JsonRpcError::new(INTERNAL_ERROR, e.to_string())),
                    Err(e) => Err(JsonRpcError::new(INVALID_PARAMS, e)),
                }
            }

            "ail.delete" => {
                let input: DeleteInput = serde_json::from_value(args)
                    .map_err(|e| JsonRpcError::new(INVALID_PARAMS, e.to_string()))?;
                let is_dry_run = input.strategy.as_deref() == Some("dry_run");

                let out = if is_dry_run {
                    // No mutation, no demotion: borrow immutably.
                    let ctx = self.context.borrow();
                    structure::run_delete_dry_run(ctx.graph(), &input)
                } else {
                    let mut ctx = self.context.borrow_mut();
                    let graph = ctx.graph_mut();
                    structure::run_delete(graph, &input)
                };
                if !is_dry_run {
                    self.dirty.set(true);
                    *self.search_cache.borrow_mut() = None;
                    *self.embedding_cache.borrow_mut() = None;
                }
                match out {
                    Ok(output) => serde_json::to_value(output)
                        .map_err(|e| JsonRpcError::new(INTERNAL_ERROR, e.to_string())),
                    Err(e) => Err(JsonRpcError::new(INVALID_PARAMS, e)),
                }
            }

            "ail.batch" => {
                let input: BatchInput = serde_json::from_value(args)
                    .map_err(|e| JsonRpcError::new(INVALID_PARAMS, e.to_string()))?;
                let output = {
                    let mut ctx = self.context.borrow_mut();
                    let graph = ctx.graph_mut();
                    batch::run_batch(graph, &input)
                };
                // The in-memory snapshot rollback restores the graph on
                // failure, but the pipeline has already been demoted to Raw
                // by `graph_mut()` and the search caches must be cleared to
                // reflect either the successful batch or the restored state.
                self.dirty.set(true);
                *self.search_cache.borrow_mut() = None;
                *self.embedding_cache.borrow_mut() = None;
                serde_json::to_value(output)
                    .map_err(|e| JsonRpcError::new(INTERNAL_ERROR, e.to_string()))
            }

            _ => Err(JsonRpcError::new(
                METHOD_NOT_FOUND,
                format!("Unknown tool: {name}"),
            )),
        }
    }

    // ── Static responses ──────────────────────────────────────────────────────

    /// MCP `initialize` response — protocol handshake.
    fn initialize() -> Value {
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "ail-mcp",
                "version": env!("CARGO_PKG_VERSION")
            }
        })
    }

    /// MCP `tools/list` response — full JSON Schema for all ten tools.
    fn tools_list() -> Value {
        json!({
            "tools": [
                {
                    "name": "ail.search",
                    "description": "BM25 semantic search over AIL project nodes",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query":  { "type": "string",  "description": "Search terms" },
                            "budget": { "type": "integer", "description": "Max results (default 10)" }
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "ail.review",
                    "description": "Review semantic coverage for a graph node; returns score, child contributions, missing aspects, and an action suggestion.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "node": { "type": "string", "description": "Node ID or name to review." }
                        },
                        "required": ["node"]
                    }
                },
                {
                    "name": "ail.context",
                    "description": "CIC context packets formatted for AI task planning",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task":          { "type": "string",  "description": "Task or concept to find context for" },
                            "budget_tokens": { "type": "integer", "description": "Max output size in approximate tokens (default 4096)" }
                        },
                        "required": ["task"]
                    }
                },
                {
                    "name": "ail.verify",
                    "description": "Re-parse and verify the full project; returns errors or ok",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "file": { "type": "string", "description": "Path hint (v0.1: always verifies whole project)" }
                        }
                    }
                },
                {
                    "name": "ail.build",
                    "description": "Emit Python files from the verified project graph",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "target":     { "type": "string",  "description": "Emission target (v0.1: python only)" },
                            "contracts":  { "type": "boolean", "description": "Inject contract checks (default true)" },
                            "async_mode": { "type": "boolean", "description": "Emit async functions (default false)" }
                        }
                    }
                },
                {
                    "name": "ail.status",
                    "description": "Project pipeline stage and node/edge counts",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    },
                    "outputSchema": {
                        "type": "object",
                        "properties": {
                            "pipeline_stage": { "type": "string" },
                            "node_count":     { "type": "integer" },
                            "edge_count":     { "type": "integer" },
                            "do_node_count":  { "type": "integer" },
                            "root_id":        { "type": ["string", "null"], "description": "UUID of the graph root node; absent when no root is set" }
                        }
                    }
                },
                {
                    "name": "ail.write",
                    "description": "Create a new node under an existing parent",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "parent_id":  { "type": "string",  "description": "Node ID of the parent" },
                            "pattern":    { "type": "string",  "description": "AIL pattern (do, define, describe, check, let, ...)" },
                            "intent":     { "type": "string",  "description": "Human-readable intent" },
                            "expression": { "type": "string",  "description": "Raw expression text (optional)" },
                            "position":   { "type": "integer", "description": "0-based position among siblings (default: append)" },
                            "contracts":  {
                                "type": "array",
                                "description": "Contracts to attach",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "kind":       { "type": "string", "description": "before, after, or always" },
                                        "expression": { "type": "string", "description": "Contract expression" }
                                    },
                                    "required": ["kind", "expression"]
                                }
                            },
                            "metadata": {
                                "type": "object",
                                "description": "Partial NodeMetadata (name, params, return_type, fields, carries, following_template_name, using_pattern_name, ...). Shallow-merged into the default."
                            }
                        },
                        "required": ["parent_id", "pattern", "intent"]
                    }
                },
                {
                    "name": "ail.patch",
                    "description": "Update fields on an existing node",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "node_id": { "type": "string", "description": "Node ID to patch" },
                            "fields": {
                                "type": "object",
                                "description": "Fields to update (only provided fields are changed)",
                                "properties": {
                                    "intent":     { "type": "string",  "description": "New intent text" },
                                    "expression": { "type": "string",  "description": "New expression text" },
                                    "pattern":    { "type": "string",  "description": "New pattern (caution: may break structure)" },
                                    "contracts":  {
                                        "type": "array",
                                        "description": "Replace all contracts",
                                        "items": {
                                            "type": "object",
                                            "properties": {
                                                "kind":       { "type": "string" },
                                                "expression": { "type": "string" }
                                            },
                                            "required": ["kind", "expression"]
                                        }
                                    },
                                    "metadata": { "type": "object", "description": "Shallow-merge into existing metadata" }
                                }
                            }
                        },
                        "required": ["node_id", "fields"]
                    }
                },
                {
                    "name": "ail.move",
                    "description": "Move a node under a new parent and optional sibling position",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "node_id":       { "type": "string",  "description": "Node ID to move" },
                            "new_parent_id": { "type": "string",  "description": "New parent's node ID" },
                            "position":      { "type": "integer", "description": "0-based position among new siblings (default: append)" }
                        },
                        "required": ["node_id", "new_parent_id"]
                    }
                },
                {
                    "name": "ail.delete",
                    "description": "Delete a node with cascade, orphan, or dry_run strategy",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "node_id":  { "type": "string", "description": "Node ID to delete" },
                            "strategy": { "type": "string", "description": "cascade (default), orphan, or dry_run" }
                        },
                        "required": ["node_id"]
                    }
                },
                {
                    "name": "ail.batch",
                    "description": "Run ordered graph mutations atomically with post-batch auto-edge refresh",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "operations": {
                                "type": "array",
                                "description": "Ordered list of write/patch/move/delete ops (executed in order; rolled back on first failure)",
                                "items": {
                                    "type": "object",
                                    "description": "One batch operation. The `op` field selects the tool; remaining fields match that tool's input schema.",
                                    "properties": {
                                        "op": { "type": "string", "description": "write, patch, move, or delete" }
                                    },
                                    "required": ["op"]
                                }
                            }
                        },
                        "required": ["operations"]
                    }
                }
            ]
        })
    }
}

/// Helper so tests can inspect the `JsonRpcId` round-trip.
#[cfg(test)]
impl McpServer {
    pub fn project_root(&self) -> &std::path::Path {
        &self.project_root
    }
}
