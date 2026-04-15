//! `ail-mcp` — MCP server exposing AIL capabilities to AI tools over stdio.
//!
//! This crate wraps the AIL compiler pipeline as a set of MCP tools that AI
//! assistants (Claude, Cursor, etc.) can call. All protocol serialization and
//! error conversion happens here; compiler logic stays in the underlying crates.
//!
//! ## Provided MCP tools
//!
//! | Tool | Description |
//! |------|-------------|
//! | `ail.search` | BM25 full-text search over the project graph |
//! | `ail.context` | Return the CIC context packet for a node |
//! | `ail.verify` | Run contract verification on a function |
//! | `ail.build` | Trigger a full build and return generated file paths |
//! | `ail.status` | Return the pipeline stage reached and graph statistics |
//!
//! ## Entry points
//!
//! - [`serve`] — run a single JSON-RPC request/response cycle.
//! - [`run_stdio_loop`] — serve indefinitely over stdin/stdout (used by `ail serve`).
//! - [`McpServer`] — stateful server wrapping project context and tool dispatch.

pub mod context;
pub(crate) mod pipeline;
pub mod server;
pub mod transport;
pub mod types;
mod tools;

pub use context::ProjectContext;
pub use server::McpServer;
pub use transport::{run_stdio_loop, serve};
pub use types::{
    JsonRpcError, JsonRpcId, JsonRpcRequest, JsonRpcResponse, INTERNAL_ERROR, INVALID_PARAMS,
    METHOD_NOT_FOUND, PARSE_ERROR,
};
