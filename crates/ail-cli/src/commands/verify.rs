//! `ail verify [file]` — run the full pipeline and report success or errors.
//!
//! v2.0 behavior:
//! - By default auto-detects the backend via `ail.config.toml [database] backend`
//!   (see [`project::resolve_backend`]). A `project.ail.db` next to the config
//!   selects SQLite; otherwise the filesystem `src/` tree is used.
//! - `--from-db <path>` forces SQLite regardless of config.
//! - The optional `file` argument is accepted for spec alignment but ignored;
//!   incremental per-file verify is v0.2 work.
//! - With `--features z3-verify`: when Z3 verification fails with an UNSAT class
//!   error (C010, C011, C015), the error body is augmented — never replaced — with
//!   sheaf localization diagnostics (Phase 17.3). Invariant 17.3-A.

use std::path::Path;

use ail_graph::validation::validate_graph;
use ail_types::type_check;

use crate::commands::project::{load_graph, resolve_backend};
use crate::error::CliError;

#[cfg(feature = "z3-verify")]
use super::sheaf::short_id;

/// Entry point for `ail verify`.
///
/// `file` is accepted but ignored in v2.0 — the full project is always verified.
/// `from_db` forces the SQLite backend; without it, the backend is auto-detected
/// from the project configuration.
/// `format` is `"text"` (default human-readable) or `"json"` (machine-readable
/// envelope `{ "ok": bool, "node_count": u, "edge_count": u, "errors"?: [str] }`).
pub fn run_verify(
    root: &Path,
    _file: Option<&Path>,
    from_db: Option<&Path>,
    format: &str,
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

    // Under z3-verify, clone the typed graph before verify() consumes it so
    // that augment_with_sheaf can re-use it on UNSAT failures (HIGH-1).
    #[cfg(feature = "z3-verify")]
    let typed_for_augment = typed.clone();

    match ail_contract::verify(typed) {
        Ok(_) => {
            match format {
                "json" => {
                    println!(
                        "{{\"ok\":true,\"node_count\":{node_count},\"edge_count\":{edge_count}}}"
                    );
                }
                _ => {
                    println!("Verified OK — {node_count} nodes, {edge_count} edges.");
                }
            }
            Ok(())
        }
        Err(errs) => {
            let lines: Vec<String> = errs.iter().map(|e| e.to_string()).collect();
            let base = lines.join("\n");

            #[cfg(feature = "z3-verify")]
            let body = if has_unsat_class(&errs) {
                augment_with_sheaf(&base, &typed_for_augment)
            } else {
                base
            };
            #[cfg(not(feature = "z3-verify"))]
            let body = base;

            if format == "json" {
                // Hand-rolled JSON to avoid adding a serde_json dep just for
                // this surface; only escapes `"` and `\` in error strings.
                let escape = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
                let arr = lines
                    .iter()
                    .map(|l| format!("\"{}\"", escape(l)))
                    .collect::<Vec<_>>()
                    .join(",");
                println!(
                    "{{\"ok\":false,\"node_count\":{node_count},\"edge_count\":{edge_count},\"errors\":[{arr}]}}"
                );
            }

            Err(CliError::Pipeline { errors: body })
        }
    }
}

// ── z3-verify-gated helpers ───────────────────────────────────────────────────

/// Return `true` when any error in `errs` is an UNSAT-class Z3 verification
/// error that maps to a sheaf-localizable contradiction.
///
/// Classes that trigger augmentation: C010 (`UnsatTypeConstraints`),
/// C011 (`ContradictoryPreconditions`), C015 (`PromotedFactContradiction`).
/// Classes that do NOT: C012 `PostconditionNotEntailed`, C013 `SolverTimeout`,
/// C014 `EncodingFailed`, and any `StaticCheck` variant.
#[cfg(feature = "z3-verify")]
fn has_unsat_class(errs: &[ail_contract::ContractStageError]) -> bool {
    use ail_contract::{ContractStageError as CSE, VerifyError as VE};
    errs.iter().any(|e| {
        matches!(
            e,
            CSE::Z3Verify(VE::UnsatTypeConstraints { .. })
                | CSE::Z3Verify(VE::ContradictoryPreconditions { .. })
                | CSE::Z3Verify(VE::PromotedFactContradiction { .. })
        )
    })
}

/// Append a sheaf-localization block to `base`. Best-effort: on any internal
/// failure the block is still appended (with a diagnostic note) so that
/// `base` — the original diagnostics — is always the prefix (invariant 17.3-A).
#[cfg(feature = "z3-verify")]
fn augment_with_sheaf(base: &str, typed: &ail_types::TypedGraph) -> String {
    let (nerve, obstructions) = ail_contract::analyze_sheaf_obstructions(typed);
    let mut out = String::from(base);
    out.push_str("\n\nSheaf localization (Phase 17.3):");
    out.push_str(&format_sheaf_localization(&nerve, &obstructions));
    out
}

/// Format a compact sheaf localization block for the `ail verify` error body.
#[cfg(feature = "z3-verify")]
fn format_sheaf_localization(
    nerve: &ail_contract::CechNerve,
    obstructions: &[ail_contract::ObstructionResult],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "\n  Sections: {}  Overlaps: {}",
        nerve.sections.len(),
        nerve.overlaps.len()
    ));

    for obs in obstructions {
        let a_prefix = short_id(&obs.node_a.to_string());
        let b_prefix = short_id(&obs.node_b.to_string());
        let idx = obs.overlap_index;

        match &obs.status {
            ail_contract::ObstructionStatus::Consistent => {
                out.push_str(&format!(
                    "\n  [Consistent] {a_prefix}…→{b_prefix}… (overlap {idx})"
                ));
            }
            ail_contract::ObstructionStatus::Contradictory {
                conflicting_a,
                conflicting_b,
            } => {
                out.push_str(&format!(
                    "\n  [Contradictory] {a_prefix}…→{b_prefix}… (overlap {idx})"
                ));
                for c in conflicting_a {
                    out.push_str(&format!("\n    node_a contributed: {c}"));
                }
                for c in conflicting_b {
                    out.push_str(&format!("\n    node_b contributed: {c}"));
                }
            }
            ail_contract::ObstructionStatus::Unknown { reason } => {
                out.push_str(&format!(
                    "\n  [Unknown] {a_prefix}…→{b_prefix}… (overlap {idx}): {reason}"
                ));
            }
        }
    }

    out
}
