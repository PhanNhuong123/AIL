//! `ail.verify` tool — re-parse and verify the full project.
//!
//! Always re-runs the full pipeline from disk so the AI gets a fresh result
//! after editing `.ail` files. On success the server context is promoted to
//! `Verified`; on failure the old context is kept and errors are returned.

use std::cell::{Cell, RefCell};
use std::path::Path;

use ail_graph::Bm25Index;
use ail_search::EmbeddingIndex;

use crate::context::ProjectContext;
use crate::pipeline::{refresh_from_graph, refresh_from_path};
use crate::types::tool_io::{VerifyInput, VerifyOutput};

/// Re-run the pipeline and return the verification result.
///
/// If `dirty` is true, the pipeline runs against the in-memory graph
/// (preserving MCP write edits); otherwise it re-parses `.ail` files from
/// `project_root`. On success, `context` is promoted to `Verified`, `dirty`
/// is cleared, and both caches are invalidated. On failure the old context
/// is kept unchanged.
pub(crate) fn run_verify(
    project_root: &Path,
    context: &RefCell<ProjectContext>,
    search_cache: &RefCell<Option<Bm25Index>>,
    embedding_cache: &RefCell<Option<EmbeddingIndex>>,
    dirty: &Cell<bool>,
    _input: &VerifyInput, // `file` hint ignored in v0.1; always verifies whole project
) -> VerifyOutput {
    let result = if dirty.get() {
        let graph = context.borrow().graph().clone();
        refresh_from_graph(graph)
    } else {
        refresh_from_path(project_root)
    };
    match result {
        Ok(new_ctx) => {
            *context.borrow_mut() = new_ctx;
            dirty.set(false);
            *search_cache.borrow_mut() = None; // invalidate stale BM25 index
            *embedding_cache.borrow_mut() = None; // invalidate stale embeddings
            VerifyOutput {
                ok: true,
                errors: Vec::new(),
                static_checks_only: !cfg!(feature = "z3-verify"),
            }
        }
        Err(errors) => VerifyOutput {
            ok: false,
            errors,
            static_checks_only: true,
        },
    }
}
