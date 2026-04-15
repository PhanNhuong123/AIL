//! `ail.verify` tool — re-parse and verify the full project.
//!
//! Always re-runs the full pipeline from disk so the AI gets a fresh result
//! after editing `.ail` files. On success the server context is promoted to
//! `Verified`; on failure the old context is kept and errors are returned.

use std::cell::RefCell;
use std::path::Path;

use ail_graph::Bm25Index;

use crate::context::ProjectContext;
use crate::pipeline::refresh_from_path;
use crate::types::tool_io::{VerifyInput, VerifyOutput};

/// Re-run the pipeline from `project_root` and return the verification result.
///
/// The `context` is updated in place on success (promoted to `Verified`).
/// The `search_cache` is cleared whenever context changes so the next search
/// rebuilds from fresh graph data.
pub(crate) fn run_verify(
    project_root: &Path,
    context: &RefCell<ProjectContext>,
    search_cache: &RefCell<Option<Bm25Index>>,
    _input: &VerifyInput, // `file` hint ignored in v0.1; always verifies whole project
) -> VerifyOutput {
    match refresh_from_path(project_root) {
        Ok(new_ctx) => {
            *context.borrow_mut() = new_ctx;
            *search_cache.borrow_mut() = None; // invalidate stale index
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
