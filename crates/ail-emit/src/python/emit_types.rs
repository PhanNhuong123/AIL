use ail_contract::VerifiedGraph;
use ail_graph::Pattern;

use crate::errors::EmitError;
use crate::python::define::emit_define_node;
use crate::python::describe::emit_describe_node;
use crate::python::error::emit_error_node;
use crate::types::{EmitOutput, EmittedFile, FileOwnership, ImportSet};

/// Emit Python type definitions for all Define, Describe, and Error nodes
/// in the verified graph.
///
/// Returns a single `generated/types.py` file containing all type classes,
/// ordered: Define first, then Describe, then Error. An `__all__` list is
/// appended so that `from generated.types import *` is safe and explicit.
/// Returns accumulated errors if any nodes are missing required metadata.
pub fn emit_type_definitions(verified: &VerifiedGraph) -> Result<EmitOutput, Vec<EmitError>> {
    let graph = verified.graph();

    let mut imports = ImportSet::new();
    let mut define_classes = Vec::new();
    let mut describe_classes = Vec::new();
    let mut error_classes = Vec::new();
    // Parallel name lists for `__all__`.
    let mut define_names: Vec<String> = Vec::new();
    let mut describe_names: Vec<String> = Vec::new();
    let mut error_names: Vec<String> = Vec::new();
    let mut errors = Vec::new();

    let all_nodes = graph.all_nodes_vec();
    for node in &all_nodes {
        match node.pattern {
            Pattern::Define => match emit_define_node(node, &mut imports) {
                Ok(code) => {
                    if let Some(name) = &node.metadata.name {
                        define_names.push(name.clone());
                    }
                    define_classes.push(code);
                }
                Err(e) => errors.push(e),
            },
            Pattern::Describe => {
                // Skip directory containers (Describe nodes with name=None
                // used for grouping in the graph, not actual type definitions).
                if node.metadata.name.is_none() {
                    continue;
                }
                match emit_describe_node(node, &mut imports) {
                    Ok(code) => {
                        if let Some(name) = &node.metadata.name {
                            describe_names.push(name.clone());
                        }
                        describe_classes.push(code);
                    }
                    Err(e) => errors.push(e),
                }
            }
            Pattern::Error => match emit_error_node(node, &mut imports) {
                Ok(code) => {
                    if let Some(name) = &node.metadata.name {
                        error_names.push(name.clone());
                    }
                    error_classes.push(code);
                }
                Err(e) => errors.push(e),
            },
            _ => {}
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    // If no type-defining nodes were found, return empty output.
    if define_classes.is_empty() && describe_classes.is_empty() && error_classes.is_empty() {
        return Ok(EmitOutput { files: vec![] });
    }

    // Assemble the file: imports, then classes in order.
    let preamble = imports.render();
    let mut sections = Vec::new();
    sections.push(preamble);

    for class in define_classes {
        sections.push(class);
    }
    for class in describe_classes {
        sections.push(class);
    }
    for class in error_classes {
        sections.push(class);
    }

    // Append `__all__` so that `from generated.types import *` is explicit and safe.
    let all_names: Vec<String> = define_names
        .into_iter()
        .chain(describe_names)
        .chain(error_names)
        .map(|n| format!("\"{n}\""))
        .collect();

    let content = format!(
        "{}\n\n\n__all__ = [{}]\n",
        sections.join("\n\n\n"),
        all_names.join(", ")
    );

    Ok(EmitOutput {
        files: vec![EmittedFile {
            path: "generated/types.py".to_owned(),
            content,
            ownership: FileOwnership::Generated,
        }],
    })
}
