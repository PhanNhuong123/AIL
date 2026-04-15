use ail_contract::VerifiedGraph;
use ail_graph::Pattern;

use crate::errors::EmitError;
use crate::python::function::emit_do_function;
use crate::python::source_map::emit_source_map;
use crate::python::test_gen::emit_test_file;
use crate::types::{EmitConfig, EmitOutput, EmittedFile, ImportSet};

/// Emit Python function definitions for all top-level `do` nodes in the verified graph,
/// plus an optional pytest test stub file and a function-level source map.
///
/// A "top-level" Do node is one whose parent is not also a Do node — meaning it represents
/// a standalone function rather than a structural section inside another function.
///
/// Returns:
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
    let mut errors = Vec::new();

    for node in graph.all_nodes() {
        if node.pattern != Pattern::Do {
            continue;
        }

        let parent_pattern = graph
            .parent_of(node.id)
            .unwrap_or(None)
            .and_then(|pid| graph.get_node(pid).ok())
            .map(|p| p.pattern.clone());

        if parent_pattern == Some(Pattern::Do) {
            continue;
        }

        match emit_do_function(graph, node, config, &mut imports) {
            Ok(code) => functions.push(code),
            Err(errs) => errors.extend(errs),
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    let mut files: Vec<EmittedFile> = Vec::new();

    if !functions.is_empty() {
        let preamble = imports.render();
        let content = format!("{}\n\n\n{}\n", preamble, functions.join("\n\n\n"));
        files.push(EmittedFile {
            path: "generated/functions.py".to_owned(),
            content,
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
