use std::collections::HashMap;

use ail_contract::VerifiedGraph;
use ail_graph::{GraphBackend, NodeId, Pattern};

use crate::errors::EmitError;
use crate::python::expression_parser::{parse_fetch_expression, parse_update_expression};
use crate::types::{EmitConfig, EmitOutput, EmittedFile, FileOwnership};
use crate::typescript::import_tracker::{ImportTracker, TypeKind};
use crate::typescript::ts_function::emit_ts_do_function;
use crate::typescript::type_map::{resolve_ts_type, to_snake_case};

// ── Repository scaffolding types ───────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum RepoMethodKind {
    Find,
    Save,
    Update,
    Remove,
}

#[derive(Debug, Clone)]
struct RepoOp {
    method_kind: RepoMethodKind,
    entity_type: String,
}

// ── Public entry point ─────────────────────────────────────────────────────────

/// Emit TypeScript function definitions for all top-level `do` nodes.
///
/// Produces:
/// - `fn/{snake}.ts` — one file per top-level Do node
/// - `fn/index.ts` — barrel re-export
/// - `repos/{source}_repository.ts` — one interface per unique repository source
/// - `repos/index.ts` — barrel re-export (when repos exist)
pub fn emit_ts_function_definitions(
    verified: &VerifiedGraph,
    config: &EmitConfig,
) -> Result<EmitOutput, Vec<EmitError>> {
    let graph = verified.graph();
    let all_nodes = graph.all_nodes_vec();

    // First pass: build type registry (same as emit_ts_type_definitions).
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

    // Second pass: collect repository ops for interface generation.
    let mut repo_ops: HashMap<String, Vec<RepoOp>> = HashMap::new();
    for node in &all_nodes {
        if node.pattern == Pattern::Do {
            collect_repo_ops_recursive(graph, node.id, &mut repo_ops);
        }
    }

    let mut files: Vec<EmittedFile> = Vec::new();
    let mut errors: Vec<EmitError> = Vec::new();

    // Third pass: emit each top-level Do node.
    for node in &all_nodes {
        if node.pattern != Pattern::Do {
            continue;
        }
        // Skip nested Do nodes (parent is also Do).
        let parent_is_do = graph
            .parent(node.id)
            .ok()
            .flatten()
            .and_then(|pid| graph.get_node(pid).ok().flatten())
            .map(|p| p.pattern == Pattern::Do)
            .unwrap_or(false);
        if parent_is_do {
            continue;
        }

        let raw_name = match node.metadata.name.as_deref() {
            Some(n) => n,
            None => {
                errors.push(EmitError::TsDoNodeMissingName { node_id: node.id });
                continue;
            }
        };
        // Derive snake_case file stem: normalise spaces to underscores first so that
        // space-separated intent names ("transfer money safely") produce the expected
        // path ("transfer_money_safely") rather than a path with literal spaces.
        let snake = to_snake_case(&raw_name.replace(' ', "_"));

        let mut tracker = ImportTracker::new();
        let mut helpers: Vec<String> = Vec::new();

        match emit_ts_do_function(
            graph,
            node,
            &type_registry,
            config,
            &mut tracker,
            &mut helpers,
            true,
        ) {
            Ok(fn_code) => {
                let all_code = if helpers.is_empty() {
                    fn_code
                } else {
                    format!("{}\n\n{}", helpers.join("\n\n"), fn_code)
                };
                let imports = tracker.render();
                let content = assemble_fn_file(&imports, &all_code);
                files.push(EmittedFile {
                    path: format!("fn/{snake}.ts"),
                    content,
                    ownership: FileOwnership::Generated,
                });
            }
            Err(errs) => errors.extend(errs),
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    if files.is_empty() {
        return Ok(EmitOutput { files: vec![] });
    }

    // fn/index.ts barrel.
    let fn_barrel = render_fn_barrel(&files);
    files.push(EmittedFile {
        path: "fn/index.ts".to_owned(),
        content: fn_barrel,
        ownership: FileOwnership::Generated,
    });

    // repos/*.ts interface files.
    let mut repo_snakes: Vec<String> = Vec::new();
    let mut sorted_sources: Vec<(&String, &Vec<RepoOp>)> = repo_ops.iter().collect();
    sorted_sources.sort_by_key(|(k, _)| k.as_str());

    for (source, ops) in sorted_sources {
        let snake_source = to_snake_case(&capitalize_first(source));
        let interface_content = emit_repo_interface(source, ops);
        files.push(EmittedFile {
            path: format!("repos/{snake_source}_repository.ts"),
            content: interface_content,
            ownership: FileOwnership::Generated,
        });
        repo_snakes.push(format!("{snake_source}_repository"));
    }

    if !repo_snakes.is_empty() {
        let repo_barrel = render_repo_barrel(&repo_snakes);
        files.push(EmittedFile {
            path: "repos/index.ts".to_owned(),
            content: repo_barrel,
            ownership: FileOwnership::Generated,
        });
    }

    Ok(EmitOutput { files })
}

// ── Repository op collection ───────────────────────────────────────────────────

fn collect_repo_ops_recursive(
    graph: &dyn GraphBackend,
    node_id: NodeId,
    repo_ops: &mut HashMap<String, Vec<RepoOp>>,
) {
    let node = match graph.get_node(node_id).ok().flatten() {
        Some(n) => n,
        None => return,
    };

    let expr = node.expression.as_ref().map(|e| e.0.as_str()).unwrap_or("");

    match node.pattern {
        Pattern::Fetch => {
            let (source_opt, _) = parse_fetch_expression(expr);
            let source = source_opt.unwrap_or_else(|| "repository".to_owned());
            let entity = node.metadata.return_type.clone().unwrap_or_default();
            if !entity.is_empty() {
                repo_ops.entry(source).or_default().push(RepoOp {
                    method_kind: RepoMethodKind::Find,
                    entity_type: entity,
                });
            }
        }
        Pattern::Save => {
            let source = expr
                .strip_prefix("to ")
                .and_then(|s| s.split(' ').next())
                .unwrap_or("repository")
                .to_owned();
            let entity = node
                .metadata
                .return_type
                .as_deref()
                .or(node.metadata.name.as_deref())
                .map(capitalize_first)
                .unwrap_or_else(|| "Entity".to_owned());
            repo_ops.entry(source).or_default().push(RepoOp {
                method_kind: RepoMethodKind::Save,
                entity_type: entity,
            });
        }
        Pattern::Update => {
            let (source_opt, _, _) = parse_update_expression(expr);
            let source = source_opt.unwrap_or_else(|| "repository".to_owned());
            let entity = node.metadata.return_type.clone().unwrap_or_default();
            if !entity.is_empty() {
                repo_ops.entry(source).or_default().push(RepoOp {
                    method_kind: RepoMethodKind::Update,
                    entity_type: entity,
                });
            }
        }
        Pattern::Remove => {
            let (source_opt, _) = parse_fetch_expression(expr);
            let source = source_opt.unwrap_or_else(|| "repository".to_owned());
            let entity = node.metadata.return_type.clone().unwrap_or_default();
            if !entity.is_empty() {
                repo_ops.entry(source).or_default().push(RepoOp {
                    method_kind: RepoMethodKind::Remove,
                    entity_type: entity,
                });
            }
        }
        _ => {}
    }

    if let Some(children) = node.children {
        for child_id in children {
            collect_repo_ops_recursive(graph, child_id, repo_ops);
        }
    }
}

// ── Repository interface emission ────────────────────────────��────────────────

fn emit_repo_interface(source: &str, ops: &[RepoOp]) -> String {
    let pascal = capitalize_first(source);
    let interface_name = format!("{pascal}Repository");

    let mut methods: Vec<String> = Vec::new();

    // Deduplicate ops by (kind, entity_type).
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for op in ops {
        let key = format!("{:?}:{}", op.method_kind, op.entity_type);
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);

        let entity = &op.entity_type;
        let ts_type = resolve_ts_type(entity);

        let method_sig = match op.method_kind {
            RepoMethodKind::Find => format!(
                "  find{entity}(where: Record<string, unknown>): Promise<{ts_type}>;"
            ),
            RepoMethodKind::Save => format!(
                "  save{entity}(data: Record<string, unknown>): Promise<void>;"
            ),
            RepoMethodKind::Update => format!(
                "  update{entity}(where: Record<string, unknown>, data: Record<string, unknown>): Promise<void>;"
            ),
            RepoMethodKind::Remove => format!(
                "  remove{entity}(where: Record<string, unknown>): Promise<void>;"
            ),
        };
        methods.push(method_sig);
    }

    let methods_str = methods.join("\n");
    format!("export interface {interface_name} {{\n{methods_str}\n}}\n")
}

// ── File assembly helpers ─────────────────────────────────���────────────────────

fn assemble_fn_file(imports: &str, code: &str) -> String {
    if imports.is_empty() {
        format!("{code}\n")
    } else {
        format!("{imports}\n\n{code}\n")
    }
}

fn render_fn_barrel(files: &[EmittedFile]) -> String {
    let lines: Vec<String> = files
        .iter()
        .filter(|f| {
            f.path.starts_with("fn/") && f.path.ends_with(".ts") && !f.path.ends_with("index.ts")
        })
        .map(|f| {
            let stem = f
                .path
                .strip_prefix("fn/")
                .and_then(|s| s.strip_suffix(".ts"))
                .unwrap_or("");
            format!("export * from './{stem}';")
        })
        .collect();
    format!("{}\n", lines.join("\n"))
}

fn render_repo_barrel(snakes: &[String]) -> String {
    let lines: Vec<String> = snakes
        .iter()
        .map(|s| format!("export * from './{s}';"))
        .collect();
    format!("{}\n", lines.join("\n"))
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let upper: String = first.to_uppercase().collect();
            upper + chars.as_str()
        }
    }
}
