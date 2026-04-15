//! `ail serve` — start the AIL MCP server over stdio.
//!
//! Attempts to pre-load the project pipeline so AI tools can search and query
//! the graph immediately. Falls back to an empty context when the project has
//! errors; callers can run `ail verify` to diagnose those errors.

use std::path::Path;

use ail_graph::AilGraph;
use ail_mcp::{serve, ProjectContext};

use crate::error::CliError;

/// Entry point for `ail serve`.
///
/// Blocks until stdin reaches EOF (the MCP client disconnects).
pub fn run_serve(root: &Path) -> Result<(), CliError> {
    let initial = load_context(root);
    serve(root.to_path_buf(), initial).map_err(CliError::Io)
}

/// Try to parse and verify the project; fall back to an empty raw context.
fn load_context(root: &Path) -> ProjectContext {
    match crate::commands::build::run_pipeline(root) {
        Ok(verified) => ProjectContext::Verified(verified),
        Err(_) => {
            eprintln!(
                "[serve] Project has errors — starting with empty context. \
                 Run `ail verify` to diagnose."
            );
            ProjectContext::Raw(AilGraph::new())
        }
    }
}
