//! `ail sheaf` — compute and display the Čech nerve (and H1 obstructions) for a project.
//!
//! Usage:
//!   ail sheaf [--node <NAME_OR_ID>] [--format text|json] [--from-db <PATH>]
//!
//! The command runs validate → type_check → verify on the project. If any stage
//! fails it returns `CliError::Pipeline`. On success it builds the Čech nerve via
//! `ail_contract::build_nerve` and optionally scopes it to a subtree via
//! `ail_contract::filter_to_subtree`.
//!
//! With `--features z3-verify`, H1 obstruction detection also runs and the
//! `obstructions` field in JSON output becomes an array. On default features it
//! is `null`.

mod render_json;
mod render_text;

use std::path::Path;

use ail_contract::{build_nerve, filter_to_subtree, CechNerve};
use ail_graph::{validation::validate_graph, GraphBackend, NodeId};
use ail_types::type_check;

use crate::commands::project::{load_graph, resolve_backend};
use crate::error::CliError;

// ── Public entry point ────────────────────────────────────────────────────────

/// Entry point for `ail sheaf`.
///
/// Runs the full pipeline. Returns `CliError::Pipeline` on any stage failure.
pub fn run_sheaf(
    cwd: &Path,
    node: Option<String>,
    format: Option<String>,
    from_db: Option<&Path>,
) -> Result<(), CliError> {
    // Validate format early so we fail fast before running the pipeline.
    let fmt = parse_format(format.as_deref())?;

    let backend = resolve_backend(cwd, from_db)?;
    let graph = load_graph(&backend)?;

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

    let verified = ail_contract::verify(typed).map_err(|errs| CliError::Pipeline {
        errors: errs
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    })?;

    // Build the full nerve.
    let full_nerve = build_nerve(&verified);

    // Resolve --node scoping.
    let (nerve, scope_kind, scope_root_id, scope_root_query) = if let Some(ref name_or_id) = node {
        let root_id = resolve_node_id(verified.graph(), name_or_id)?;

        let filtered = filter_to_subtree(&full_nerve, root_id, verified.graph());

        // LOW-8: warn if the resolved node is not a Do node (no sections in subtree).
        if filtered.sections.is_empty() && filtered.overlaps.is_empty() {
            eprintln!("note: `{name_or_id}` is not a Do node — empty subtree returned.");
        }

        (
            filtered,
            "subtree",
            Some(root_id.to_string()),
            Some(name_or_id.as_str()),
        )
    } else {
        (full_nerve, "full", None, None)
    };

    // Run obstruction detection (z3-verify only).
    #[cfg(feature = "z3-verify")]
    let obstructions_vec = ail_contract::detect_obstructions(&nerve, &verified);

    // Build output.
    let output = render_sheaf_output(
        &nerve,
        scope_kind,
        scope_root_id.as_deref(),
        scope_root_query,
        #[cfg(feature = "z3-verify")]
        &obstructions_vec,
        fmt,
    );

    println!("{output}");
    Ok(())
}

/// Render the sheaf output as a `String`.
///
/// Exposed as `pub` so that integration tests in `tests/sheaf_cmd.rs` can call
/// it directly without capturing stdout.
pub fn render_sheaf_output(
    nerve: &CechNerve,
    scope_kind: &str,
    scope_root_id: Option<&str>,
    scope_root_query: Option<&str>,
    #[cfg(feature = "z3-verify")] obstructions: &[ail_contract::ObstructionResult],
    fmt: OutputFormat,
) -> String {
    match fmt {
        OutputFormat::Text => render_text::render_text(
            nerve,
            scope_kind,
            scope_root_id,
            scope_root_query,
            #[cfg(feature = "z3-verify")]
            obstructions,
        ),
        OutputFormat::Json => render_json::render_json(
            nerve,
            scope_kind,
            scope_root_id,
            scope_root_query,
            #[cfg(feature = "z3-verify")]
            obstructions,
        ),
    }
}

// ── Format parsing ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Text,
    Json,
}

fn parse_format(value: Option<&str>) -> Result<OutputFormat, CliError> {
    match value {
        None | Some("text") => Ok(OutputFormat::Text),
        Some("json") => Ok(OutputFormat::Json),
        Some(v) => Err(CliError::Pipeline {
            errors: format!("invalid --format value `{v}`. Expected: text, json"),
        }),
    }
}

// ── Node resolution ───────────────────────────────────────────────────────────

/// Resolve a display name or UUID string to a [`NodeId`] using the graph backend.
///
/// Tries exact UUID parse first, then falls back to case-insensitive name match.
fn resolve_node_id(graph: &dyn GraphBackend, name_or_id: &str) -> Result<NodeId, CliError> {
    // Try exact UUID parse.
    if let Ok(id) = name_or_id.parse::<NodeId>() {
        if graph
            .get_node(id)
            .map_err(|e| CliError::Pipeline {
                errors: format!("get_node: {e}"),
            })?
            .is_some()
        {
            return Ok(id);
        }
    }

    // Fall back to case-insensitive name/intent search.
    let ids = graph.all_node_ids().map_err(|e| CliError::Pipeline {
        errors: format!("all_node_ids: {e}"),
    })?;

    for id in ids {
        let node = match graph.get_node(id).map_err(|e| CliError::Pipeline {
            errors: format!("get_node: {e}"),
        })? {
            Some(n) => n,
            None => continue,
        };

        let matches_name = node
            .metadata
            .name
            .as_deref()
            .map(|n| n.eq_ignore_ascii_case(name_or_id))
            .unwrap_or(false);
        let matches_intent = node.intent.eq_ignore_ascii_case(name_or_id);

        if matches_name || matches_intent {
            return Ok(id);
        }
    }

    Err(CliError::Pipeline {
        errors: format!("node `{name_or_id}` not found in project"),
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Return the first 8 characters of a UUID string as an owned `String`.
///
/// Exposed as `pub(super)` so that `commands::verify` can reuse it for
/// compact node-id display in sheaf localization blocks (Fix #6 / IMPORTANT-8).
pub(super) fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}
