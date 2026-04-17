//! `ail verify [file]` — run the full pipeline and report success or errors.
//!
//! v2.0 behavior:
//! - By default auto-detects the backend via `ail.config.toml [database] backend`
//!   (see [`project::resolve_backend`]). A `project.ail.db` next to the config
//!   selects SQLite; otherwise the filesystem `src/` tree is used.
//! - `--from-db <path>` forces SQLite regardless of config.
//! - The optional `file` argument is accepted for spec alignment but ignored;
//!   incremental per-file verify is v0.2 work.

use std::path::Path;

use ail_graph::validation::validate_graph;
use ail_types::type_check;

use crate::commands::project::{load_graph, resolve_backend};
use crate::error::CliError;

/// Entry point for `ail verify`.
///
/// `file` is accepted but ignored in v2.0 — the full project is always verified.
/// `from_db` forces the SQLite backend; without it, the backend is auto-detected
/// from the project configuration.
pub fn run_verify(
    root: &Path,
    _file: Option<&Path>,
    from_db: Option<&Path>,
) -> Result<(), CliError> {
    let backend = resolve_backend(root, from_db)?;
    let graph = load_graph(&backend)?;

    let node_count = graph.node_count();
    let edge_count = graph.edge_count();

    let valid = validate_graph(graph).map_err(|errs| CliError::Pipeline {
        errors: errs
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    })?;

    let typed = type_check(valid, &[]).map_err(|errs| CliError::Pipeline {
        errors: errs
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    })?;

    ail_contract::verify(typed).map_err(|errs| CliError::Pipeline {
        errors: errs
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    })?;

    println!("Verified OK — {node_count} nodes, {edge_count} edges.");
    Ok(())
}
