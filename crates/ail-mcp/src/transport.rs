//! Stdio JSON-RPC 2.0 transport for the MCP server.
//!
//! `ail-mcp` is launched as a subprocess by MCP clients (Claude Desktop,
//! Cursor, etc.). The client writes newline-delimited JSON requests to stdin
//! and reads newline-delimited JSON responses from stdout.
//!
//! [`run_stdio_loop`] implements this transport. It reads one JSON object per
//! line, forwards it to [`crate::McpServer::handle`], and writes the response
//! (if any) followed by a newline.

use std::io::{BufRead, Write};

use crate::server::McpServer;
use crate::types::protocol::{JsonRpcError, JsonRpcResponse, PARSE_ERROR};

/// Run the MCP stdio transport loop.
///
/// Blocks until stdin reaches EOF. Each incoming line is deserialized as a
/// [`crate::types::protocol::JsonRpcRequest`] and dispatched to `server`.
/// Responses are written to stdout, one JSON object per line.
///
/// Parse errors for individual lines produce a JSON-RPC parse error response
/// rather than terminating the loop.
pub fn run_stdio_loop(server: &McpServer) -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());

    for line in stdin.lock().lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let response = match serde_json::from_str(trimmed) {
            Ok(req) => server.handle(req),
            Err(e) => {
                // Malformed JSON — return a parse error (id is unknown).
                Some(JsonRpcResponse::err(
                    None,
                    JsonRpcError::new(PARSE_ERROR, format!("Parse error: {e}")),
                ))
            }
        };

        if let Some(resp) = response {
            serde_json::to_writer(&mut out, &resp)?;
            writeln!(out)?;
            out.flush()?;
        }
    }

    Ok(())
}

/// Convenience: build a minimal server from a project root and run the loop.
///
/// `initial` is the pipeline stage to start with (typically
/// `ProjectContext::Raw(AilGraph::new())` when launching fresh; callers that
/// have pre-built a graph can pass a higher stage).
pub fn serve(
    project_root: std::path::PathBuf,
    initial: crate::context::ProjectContext,
) -> std::io::Result<()> {
    let server = McpServer::new(project_root, initial);
    run_stdio_loop(&server)
}
