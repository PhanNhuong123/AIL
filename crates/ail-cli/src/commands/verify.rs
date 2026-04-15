//! `ail verify [file]` — run the full pipeline and report success or errors.
//!
//! v0.1: always verifies the entire project, regardless of the optional `file`
//! argument. The argument is accepted for UX alignment with the spec; incremental
//! per-file verification is planned for v0.2.

use std::path::Path;

use ail_graph::validation::validate_graph;
use ail_text::parse_directory;
use ail_types::type_check;

use crate::error::CliError;

/// Entry point for `ail verify`.
///
/// `file` is accepted but ignored in v0.1 — the full project is always verified.
pub fn run_verify(root: &Path, _file: Option<&Path>) -> Result<(), CliError> {
    let graph = parse_directory(root).map_err(|e| CliError::Pipeline {
        errors: e.to_string(),
    })?;

    let node_count = graph.node_count();

    let valid = validate_graph(graph).map_err(|errs| CliError::Pipeline {
        errors: errs
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    })?;

    let edge_count = valid.graph().edge_count();

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
