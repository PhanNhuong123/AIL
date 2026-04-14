use ail_contract::VerifiedGraph;
use ail_graph::Pattern;

use crate::errors::EmitError;
use crate::python::function::emit_do_function;
use crate::types::{EmitConfig, EmitOutput, EmittedFile, ImportSet};

/// Emit Python function definitions for all top-level `do` nodes in the verified graph.
///
/// A "top-level" Do node is one whose parent is not also a Do node — meaning it represents
/// a standalone function rather than a structural section inside another function.
///
/// Returns a single `generated/functions.py` file containing all function definitions,
/// in graph traversal order. Returns an empty output if no top-level Do nodes are found.
/// Returns accumulated errors if any Do nodes fail to emit.
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

        // Determine whether this Do node is a top-level function or a nested section.
        // A nested section has a parent whose pattern is also Do.
        let parent_pattern = graph
            .parent_of(node.id)
            // parent_of returns Result<Option<NodeId>>; treat errors as "no parent"
            .unwrap_or(None)
            .and_then(|pid| graph.get_node(pid).ok())
            .map(|p| p.pattern.clone());

        if parent_pattern == Some(Pattern::Do) {
            // Nested section Do — emitted inline from its parent's body; skip here.
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

    if functions.is_empty() {
        return Ok(EmitOutput { files: vec![] });
    }

    // Assemble the file: imports preamble then function definitions.
    let preamble = imports.render();
    let content = format!("{}\n\n\n{}\n", preamble, functions.join("\n\n\n"));

    Ok(EmitOutput {
        files: vec![EmittedFile {
            path: "generated/functions.py".to_owned(),
            content,
        }],
    })
}
