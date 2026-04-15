//! [`McpServer`] — the MCP server core.
//!
//! Holds project state and routes JSON-RPC 2.0 requests to the five tool
//! handlers. The server is single-threaded; mutability is tracked via
//! `RefCell`.

use std::cell::RefCell;
use std::path::PathBuf;

use serde_json::{json, Value};

use ail_graph::Bm25Index;

use crate::context::ProjectContext;
use crate::tools::{build, context, search, status, verify};
use crate::types::protocol::{
    JsonRpcError, JsonRpcRequest, JsonRpcResponse, INTERNAL_ERROR, INVALID_PARAMS, METHOD_NOT_FOUND,
};
use crate::types::tool_io::{BuildInput, ContextInput, SearchInput, VerifyInput};

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
}

impl McpServer {
    /// Create a new server rooted at `project_root` with an initial context.
    pub fn new(project_root: PathBuf, initial: ProjectContext) -> Self {
        Self {
            project_root,
            context: RefCell::new(initial),
            search_cache: RefCell::new(None),
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
                let input: SearchInput = serde_json::from_value(args)
                    .map_err(|e| JsonRpcError::new(INVALID_PARAMS, e.to_string()))?;
                let borrow = self.context.borrow();
                let out = search::run_search(borrow.graph(), &self.search_cache, &input);
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

    /// MCP `tools/list` response — full JSON Schema for all five tools.
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
