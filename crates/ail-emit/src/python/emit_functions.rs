use ail_contract::VerifiedGraph;
use ail_graph::Pattern;

use crate::errors::EmitError;
use crate::python::function::{emit_do_function, slugify_name};
use crate::python::source_map::emit_source_map;
use crate::python::test_gen::emit_test_file;
use crate::types::{EmitConfig, EmitOutput, EmittedFile, FileOwnership, ImportSet};

/// Emit Python function definitions for all top-level `do` nodes in the verified graph,
/// plus an optional pytest test stub file, a function-level source map, and the
/// `generated/__init__.py` package marker.
///
/// A "top-level" Do node is one whose parent is not also a Do node — meaning it represents
/// a standalone function rather than a structural section inside another function.
///
/// Returns:
/// - `generated/__init__.py` — package marker (absent when no top-level Do exists).
/// - `generated/functions.py` — all function definitions (absent when no top-level Do exists).
/// - `generated/test_contracts.py` — pytest stubs (absent when no Do has contracts).
/// - `generated/functions.ailmap.json` — function-level source map (absent when no top-level Do).
pub fn emit_function_definitions(
    verified: &VerifiedGraph,
    config: &EmitConfig,
) -> Result<EmitOutput, Vec<EmitError>> {
    let graph = verified.graph();

    let mut imports = ImportSet::new();
    let mut functions = Vec::new();
    let mut fn_names: Vec<String> = Vec::new();
    let mut errors = Vec::new();

    let all_nodes = graph.all_nodes_vec();
    for node in &all_nodes {
        if node.pattern != Pattern::Do {
            continue;
        }

        let parent_pattern = graph
            .parent(node.id)
            .ok()
            .flatten()
            .and_then(|pid| graph.get_node(pid).ok().flatten())
            .map(|p| p.pattern.clone());

        if parent_pattern == Some(Pattern::Do) {
            continue;
        }

        match emit_do_function(graph, node, config, &mut imports) {
            Ok(code) => {
                // Track the function name for __all__.
                if let Some(raw) = node.metadata.name.as_deref() {
                    fn_names.push(slugify_name(raw));
                }
                functions.push(code);
            }
            Err(errs) => errors.extend(errs),
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    let mut files: Vec<EmittedFile> = Vec::new();

    if !functions.is_empty() {
        // Append __all__ so that `from generated.functions import *` is safe and explicit.
        let all_names: Vec<String> = fn_names.iter().map(|n| format!("\"{n}\"")).collect();
        let preamble = imports.render();
        let body = functions.join("\n\n\n");
        let content = format!(
            "{preamble}\n\n\n{body}\n\n\n__all__ = [{}]\n",
            all_names.join(", ")
        );

        // functions.py comes first: callers and tests treat files[0] as the primary output.
        files.push(EmittedFile {
            path: "generated/functions.py".to_owned(),
            content,
            ownership: FileOwnership::Generated,
        });

        // generated/__init__.py makes `generated` a proper Python package.
        // Relative imports (`from .types import *`) are safe because both files define __all__.
        files.push(EmittedFile {
            path: "generated/__init__.py".to_owned(),
            content: GENERATED_INIT_PY.to_owned(),
            ownership: FileOwnership::Generated,
        });
    }

    // Optional pytest test stubs.
    if let Some(test_file) = emit_test_file(verified, config) {
        files.push(test_file);
    }

    // Optional function-level source map.
    if let Some(map_file) = emit_source_map(verified) {
        files.push(map_file);
    }

    Ok(EmitOutput { files })
}

/// Content of `generated/__init__.py`.
///
/// Makes `generated/` a proper Python package so that relative imports work
/// (e.g. `from .types import *` inside `functions.py`). Uses `*` imports
/// because both `types.py` and `functions.py` define `__all__`.
const GENERATED_INIT_PY: &str = "\
# Auto-generated — do not edit.
from __future__ import annotations

from .types import *
from .functions import *
";
