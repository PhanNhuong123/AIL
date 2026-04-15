//! `ail.build` tool — emit Python from the verified project graph.
//!
//! Uses the already-verified context when available; otherwise runs a full
//! pipeline refresh from disk first.

use std::cell::RefCell;
use std::path::Path;

use ail_emit::{
    emit_function_definitions, emit_type_definitions, ContractMode, EmitConfig,
};
use ail_graph::Bm25Index;

use crate::context::ProjectContext;
use crate::pipeline::refresh_from_path;
use crate::types::tool_io::{BuildFile, BuildInput, BuildOutput};

/// Emit Python files for the project.
///
/// If the context is already `Verified`, emission runs immediately. Otherwise
/// the full pipeline is refreshed from `project_root` first.
pub(crate) fn run_build(
    project_root: &Path,
    context: &RefCell<ProjectContext>,
    search_cache: &RefCell<Option<Bm25Index>>,
    input: &BuildInput,
) -> BuildOutput {
    // Ensure the context is Verified (refresh if needed).
    if context.borrow().as_verified().is_none() {
        match refresh_from_path(project_root) {
            Ok(new_ctx) => {
                *context.borrow_mut() = new_ctx;
                *search_cache.borrow_mut() = None;
            }
            Err(errors) => {
                return BuildOutput {
                    ok: false,
                    files: Vec::new(),
                    errors,
                };
            }
        }
    }

    let borrow = context.borrow();
    let verified = match borrow.as_verified() {
        Some(v) => v,
        None => {
            // Should be unreachable after the refresh above.
            return BuildOutput {
                ok: false,
                files: Vec::new(),
                errors: vec!["Graph must be fully verified before building".into()],
            };
        }
    };

    let config = EmitConfig {
        async_mode: input.async_mode.unwrap_or(false),
        contract_mode: if input.contracts.unwrap_or(true) {
            ContractMode::On
        } else {
            ContractMode::Off
        },
    };

    let mut all_files: Vec<BuildFile> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    match emit_type_definitions(verified) {
        Ok(out) => {
            for f in out.files {
                all_files.push(BuildFile {
                    path: f.path.clone(),
                    ownership: format!("{:?}", f.ownership),
                    size_bytes: f.content.len(),
                });
            }
        }
        Err(errs) => errors.extend(errs.iter().map(|e| e.to_string())),
    }

    match emit_function_definitions(verified, &config) {
        Ok(out) => {
            for f in out.files {
                all_files.push(BuildFile {
                    path: f.path.clone(),
                    ownership: format!("{:?}", f.ownership),
                    size_bytes: f.content.len(),
                });
            }
        }
        Err(errs) => errors.extend(errs.iter().map(|e| e.to_string())),
    }

    if errors.is_empty() {
        BuildOutput {
            ok: true,
            files: all_files,
            errors: Vec::new(),
        }
    } else {
        BuildOutput {
            ok: false,
            files: all_files,
            errors,
        }
    }
}
