//! Pipeline refresh — re-runs the full AIL stage pipeline from `.ail` sources.
//!
//! Used by `ail.verify` and `ail.build` to ensure they operate on a fresh
//! view of the project after the user edits files.

use std::path::Path;

use ail_contract::verify;
use ail_graph::validation::validate_graph;
use ail_text::parse_directory;
use ail_types::type_check;

use crate::context::ProjectContext;

/// Re-run the full pipeline (`parse → validate → type_check → verify`) from
/// the `.ail` files under `root`.
///
/// Returns `Ok(ProjectContext::Verified(_))` on success, or `Err(errors)` with
/// all accumulated error strings when any stage fails.
pub(crate) fn refresh_from_path(root: &Path) -> Result<ProjectContext, Vec<String>> {
    // ── 1. Parse ─────────────────────────────────────────────────────────────
    let graph = parse_directory(root).map_err(|e| vec![e.to_string()])?;

    // ── 2. Validate ──────────────────────────────────────────────────────────
    let valid = validate_graph(graph)
        .map_err(|errs| errs.iter().map(|e| e.to_string()).collect::<Vec<_>>())?;

    // ── 3. Type-check (no pre-computed packets for the MCP path) ─────────────
    let typed = type_check(valid, &[])
        .map_err(|errs| errs.iter().map(|e| e.to_string()).collect::<Vec<_>>())?;

    // ── 4. Contract verification ──────────────────────────────────────────────
    let verified = verify(typed)
        .map_err(|errs| errs.iter().map(|e| e.to_string()).collect::<Vec<_>>())?;

    Ok(ProjectContext::Verified(verified))
}
