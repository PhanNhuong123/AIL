use std::collections::HashMap;

use ail_contract::VerifiedGraph;
use ail_graph::Pattern;

use crate::errors::EmitError;
use crate::types::{EmitOutput, EmittedFile, FileOwnership};
use crate::typescript::define::emit_define_node;
use crate::typescript::describe::emit_describe_node;
use crate::typescript::error::emit_error_node;
use crate::typescript::import_tracker::{ImportTracker, TypeKind};
use crate::typescript::type_map::to_snake_case;

/// Emit TypeScript type definitions for all Define, Describe, and Error nodes
/// in the verified graph.
///
/// Produces one `.ts` file per type node in `types/` (Define, Describe) or
/// `errors/` (Error), plus a barrel `index.ts` for each output folder.
///
/// Import statements in each file are generated from graph type references —
/// no text scanning.
pub fn emit_ts_type_definitions(verified: &VerifiedGraph) -> Result<EmitOutput, Vec<EmitError>> {
    let graph = verified.graph();
    let all_nodes = graph.all_nodes_vec();

    // First pass: build a registry of all named type nodes and their kinds.
    // Used by describe/error emitters to decide which symbols to import.
    let mut type_registry: HashMap<String, TypeKind> = HashMap::new();
    for node in &all_nodes {
        if let Some(name) = &node.metadata.name {
            let kind = match node.pattern {
                Pattern::Define => TypeKind::Define,
                Pattern::Describe => TypeKind::Describe,
                Pattern::Error => TypeKind::Error,
                _ => continue,
            };
            type_registry.insert(name.clone(), kind);
        }
    }

    let mut files: Vec<EmittedFile> = Vec::new();
    let mut errors: Vec<EmitError> = Vec::new();

    // Barrel export symbol lists.
    let mut type_barrel: Vec<(String, String)> = Vec::new(); // (name, snake_name)
    let mut error_barrel: Vec<String> = Vec::new(); // snake_name

    // Second pass: emit each type node.
    for node in &all_nodes {
        match node.pattern {
            Pattern::Define => {
                let name = match node.metadata.name.as_deref() {
                    Some(n) => n,
                    None => {
                        errors.push(EmitError::TsDefineNodeMissingName { node_id: node.id });
                        continue;
                    }
                };
                let snake = to_snake_case(name);
                let mut tracker = ImportTracker::new();

                match emit_define_node(node, &mut tracker) {
                    Ok(body) => {
                        let imports = tracker.render();
                        let content = assemble_file(&imports, &body);
                        files.push(EmittedFile {
                            path: format!("types/{snake}.ts"),
                            content,
                            ownership: FileOwnership::Generated,
                        });
                        type_barrel.push((name.to_owned(), snake));
                    }
                    Err(e) => errors.push(e),
                }
            }

            Pattern::Describe => {
                // Skip unnamed container nodes.
                let name = match node.metadata.name.as_deref() {
                    Some(n) => n,
                    None => continue,
                };
                let snake = to_snake_case(name);
                let mut tracker = ImportTracker::new();

                match emit_describe_node(node, &mut tracker, &type_registry) {
                    Ok(body) => {
                        let imports = tracker.render();
                        let content = assemble_file(&imports, &body);
                        files.push(EmittedFile {
                            path: format!("types/{snake}.ts"),
                            content,
                            ownership: FileOwnership::Generated,
                        });
                        type_barrel.push((name.to_owned(), snake));
                    }
                    Err(e) => errors.push(e),
                }
            }

            Pattern::Error => {
                let name = match node.metadata.name.as_deref() {
                    Some(n) => n,
                    None => {
                        errors.push(EmitError::TsErrorNodeMissingName { node_id: node.id });
                        continue;
                    }
                };
                let snake = to_snake_case(name);
                let mut tracker = ImportTracker::new();

                match emit_error_node(node, &mut tracker, &type_registry) {
                    Ok(body) => {
                        let imports = tracker.render();
                        let content = assemble_file(&imports, &body);
                        files.push(EmittedFile {
                            path: format!("errors/{snake}.ts"),
                            content,
                            ownership: FileOwnership::Generated,
                        });
                        error_barrel.push(snake);
                    }
                    Err(e) => errors.push(e),
                }
            }

            _ => {}
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    if files.is_empty() {
        return Ok(EmitOutput { files: vec![] });
    }

    // Emit barrel index files.
    if !type_barrel.is_empty() {
        let barrel_content = render_types_barrel(&type_barrel);
        files.push(EmittedFile {
            path: "types/index.ts".to_owned(),
            content: barrel_content,
            ownership: FileOwnership::Generated,
        });
    }

    if !error_barrel.is_empty() {
        let barrel_content = render_errors_barrel(&error_barrel);
        files.push(EmittedFile {
            path: "errors/index.ts".to_owned(),
            content: barrel_content,
            ownership: FileOwnership::Generated,
        });
    }

    Ok(EmitOutput { files })
}

/// Assemble a complete TypeScript file from an import block and a body block.
fn assemble_file(imports: &str, body: &str) -> String {
    if imports.is_empty() {
        format!("{body}\n")
    } else {
        format!("{imports}\n\n{body}\n")
    }
}

/// Render a barrel `index.ts` for the `types/` folder.
///
/// For `define` types, the barrel re-exports the type, factory, and predicate.
/// For `describe` types, it re-exports the interface and factory.
/// Since we can't distinguish them here without the pattern, we export all
/// names from each module using `export * from './module'` for simplicity.
fn render_types_barrel(entries: &[(String, String)]) -> String {
    let lines: Vec<String> = entries
        .iter()
        .map(|(_, snake)| format!("export * from './{snake}';"))
        .collect();
    format!("{}\n", lines.join("\n"))
}

/// Render a barrel `index.ts` for the `errors/` folder.
fn render_errors_barrel(snakes: &[String]) -> String {
    let lines: Vec<String> = snakes
        .iter()
        .map(|snake| format!("export * from './{snake}';"))
        .collect();
    format!("{}\n", lines.join("\n"))
}
