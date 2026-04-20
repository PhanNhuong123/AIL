use std::collections::BTreeMap;

use ail_contract::VerifiedGraph;
use ail_graph::{compute_context_packet_for_backend, GraphBackend, NodeId, Pattern};

use crate::ids::IdMap;
use crate::rollup::{rollup, rollup_from_contracts};
use crate::types::graph_json::{
    ClusterJson, ErrorRefJson, ExternalJson, FunctionJson, GraphJson, ModuleJson, ProjectJson,
    RelationJson, StepJson, TypeRefJson,
};
use crate::types::node_detail::{
    InheritedRule, NodeDetail, ReceivesEntry, ReturnsEntry, RuleEntry, RuleSource,
    VerificationDetail,
};
use crate::types::patch::{GraphPatchJson, PatchItem};
use crate::types::status::Status;

const DEFAULT_CLUSTER_ID: &str = "default";
const DEFAULT_CLUSTER_COLOR: &str = "#2997ff";

/// Serialize a `VerifiedGraph` into a `GraphJson` for the IDE frontend.
///
/// `project_name` is used as the project title and default cluster name.
///
/// Graph structure handling:
/// - The root node (a Describe container) and each Describe subdir are treated
///   as "modules".
/// - `Do` children of a module node become `FunctionJson` entries.
/// - `Define`/`Describe`-with-name children become `TypeRefJson`.
/// - `Error` children become `ErrorRefJson`.
pub fn serialize_graph(graph: &VerifiedGraph, project_name: &str) -> GraphJson {
    let backend = graph.graph();
    let id_map = IdMap::build(backend);

    let cluster = ClusterJson {
        id: DEFAULT_CLUSTER_ID.to_string(),
        name: project_name.to_string(),
        color: DEFAULT_CLUSTER_COLOR.to_string(),
    };

    let mut modules: Vec<ModuleJson> = Vec::new();
    let mut all_types: Vec<TypeRefJson> = Vec::new();
    let mut all_errors: Vec<ErrorRefJson> = Vec::new();
    let mut relations: Vec<RelationJson> = Vec::new();
    let mut detail: BTreeMap<String, NodeDetail> = BTreeMap::new();

    // Walk root nodes — each root is a Describe container that becomes a module.
    let root_ids = backend.root_nodes().unwrap_or_default();
    for root_id in root_ids {
        collect_module(
            backend,
            root_id,
            &id_map,
            project_name,
            &mut modules,
            &mut all_types,
            &mut all_errors,
            &mut relations,
            &mut detail,
        );
    }

    let module_statuses: Vec<Status> = modules.iter().map(|m| m.status).collect();
    let project_status = rollup(&module_statuses);
    let fn_count: usize = modules.iter().map(|m| m.functions.len()).sum();
    let total_node_count = backend.node_count();

    let project = ProjectJson {
        id: project_name.to_lowercase().replace(' ', "_"),
        name: project_name.to_string(),
        description: String::new(),
        node_count: total_node_count,
        module_count: modules.len(),
        fn_count,
        status: project_status,
    };

    GraphJson {
        project,
        clusters: vec![cluster],
        modules,
        externals: Vec::<ExternalJson>::new(),
        relations,
        types: all_types,
        errors: all_errors,
        detail,
    }
}

/// Recursively collect module data starting from a Describe container node.
///
/// If the container has Do children it is treated as a module directly.
/// Describe-with-no-name subdirectory containers are walked to find nested modules.
#[allow(clippy::too_many_arguments)]
fn collect_module(
    backend: &dyn GraphBackend,
    container_id: NodeId,
    id_map: &IdMap,
    project_name: &str,
    modules: &mut Vec<ModuleJson>,
    all_types: &mut Vec<TypeRefJson>,
    all_errors: &mut Vec<ErrorRefJson>,
    relations: &mut Vec<RelationJson>,
    detail: &mut BTreeMap<String, NodeDetail>,
) {
    let Some(container_node) = backend.get_node(container_id).ok().flatten() else {
        // Container node itself not found — graph corruption; skip.
        // TODO(16.x): propagate BridgeError::NodeNotFound once
        // collect_module returns Result.
        #[cfg(debug_assertions)]
        eprintln!("[ail-ui-bridge] WARNING: container node {container_id} not found; skipping");
        return;
    };

    let children = backend.children(container_id).unwrap_or_default();
    if children.is_empty() {
        return;
    }

    // Check if any direct children are Do functions.
    let has_do_children = children.iter().any(|&id| {
        backend
            .get_node(id)
            .ok()
            .flatten()
            .map(|n| n.pattern == Pattern::Do)
            .unwrap_or(false)
    });

    // Check if any direct children are unnamed Describe containers (subdirs).
    let subdir_children: Vec<NodeId> = children
        .iter()
        .copied()
        .filter(|&id| {
            backend
                .get_node(id)
                .ok()
                .flatten()
                .map(|n| n.pattern == Pattern::Describe && n.metadata.name.is_none())
                .unwrap_or(false)
        })
        .collect();

    if has_do_children {
        // This container IS a module — process its Do children as functions.
        let module_path = id_map.get_path(container_id).to_string();
        let mut functions: Vec<FunctionJson> = Vec::new();
        let mut fn_statuses: Vec<Status> = Vec::new();
        let mut node_count: usize = 1;

        for child_id in &children {
            let Some(child_node) = backend.get_node(*child_id).ok().flatten() else {
                // Child listed by parent but not retrievable — graph corruption.
                // The pipeline would normally reject this before we get here.
                // TODO(16.x): propagate BridgeError::NodeNotFound once
                // collect_module returns Result.
                #[cfg(debug_assertions)]
                eprintln!("[ail-ui-bridge] WARNING: child node {child_id} not found; skipping");
                continue;
            };
            node_count += 1;

            match child_node.pattern {
                Pattern::Do => {
                    let fn_path = id_map.get_path(*child_id).to_string();
                    let (steps, fn_status) = collect_steps(backend, *child_id, id_map, detail);

                    let fn_name = child_node
                        .metadata
                        .name
                        .as_deref()
                        .unwrap_or(&child_node.intent)
                        .to_string();

                    let fn_detail = build_node_detail(backend, *child_id, fn_status);
                    detail.insert(fn_path.clone(), fn_detail);

                    fn_statuses.push(fn_status);
                    functions.push(FunctionJson {
                        id: fn_path,
                        name: fn_name,
                        status: fn_status,
                        steps: Some(steps),
                    });

                    collect_relations(backend, *child_id, id_map, relations);
                }
                Pattern::Define | Pattern::Describe if child_node.metadata.name.is_some() => {
                    let type_path = id_map.get_path(*child_id).to_string();
                    let type_name = child_node
                        .metadata
                        .name
                        .as_deref()
                        .unwrap_or(&child_node.intent)
                        .to_string();
                    all_types.push(TypeRefJson {
                        id: type_path,
                        name: type_name,
                        status: Status::Ok,
                    });
                }
                Pattern::Error => {
                    let error_path = id_map.get_path(*child_id).to_string();
                    let error_name = child_node
                        .metadata
                        .name
                        .as_deref()
                        .unwrap_or(&child_node.intent)
                        .to_string();
                    all_errors.push(ErrorRefJson {
                        id: error_path,
                        name: error_name,
                        status: Status::Ok,
                    });
                }
                _ => {}
            }
        }

        let module_status = rollup(&fn_statuses);
        let module_name = container_node
            .metadata
            .name
            .as_deref()
            .unwrap_or(&container_node.intent)
            .to_string();

        let mod_detail = build_node_detail(backend, container_id, module_status);
        detail.insert(module_path.clone(), mod_detail);

        modules.push(ModuleJson {
            id: module_path,
            name: module_name,
            description: container_node.intent.clone(),
            cluster: DEFAULT_CLUSTER_ID.to_string(),
            cluster_name: project_name.to_string(),
            cluster_color: DEFAULT_CLUSTER_COLOR.to_string(),
            status: module_status,
            node_count,
            functions,
        });
    } else {
        // No Do children — recurse into unnamed Describe subdir containers.
        for sub_id in subdir_children {
            collect_module(
                backend,
                sub_id,
                id_map,
                project_name,
                modules,
                all_types,
                all_errors,
                relations,
                detail,
            );
        }
    }
}

/// Collect steps for a `Do` node, returning `(steps, function_status)`.
///
/// # Status derivation
///
/// A `VerifiedGraph` is only produced when all static contract checks pass and,
/// when the `z3-verify` feature is enabled, all Z3 satisfiability checks pass.
/// If any check fails, `verify()` returns `Err` and the pipeline surfaces a
/// `BridgeError::PipelineError` before serialisation runs.
///
/// Therefore every step reachable here has, at minimum, passed the static
/// checker. Step-level status is derived via `rollup_from_contracts`:
/// - Steps with no contracts: `Ok`.
/// - Steps with contracts in a verified graph: `Ok` (contracts satisfied).
///
/// TODO(16.x): Thread per-node Z3 counterexample results through
/// `VerifiedGraph` so individual steps can show `Fail` / `Warn` without
/// aborting the whole pipeline. Tracked in the Phase 16 roadmap.
fn collect_steps(
    backend: &dyn GraphBackend,
    fn_id: NodeId,
    id_map: &IdMap,
    detail: &mut BTreeMap<String, NodeDetail>,
) -> (Vec<StepJson>, Status) {
    let children = backend.children(fn_id).unwrap_or_default();
    let mut steps: Vec<StepJson> = Vec::new();
    let mut step_statuses: Vec<Status> = Vec::new();

    for child_id in children {
        let Some(child_node) = backend.get_node(child_id).ok().flatten() else {
            // Child listed by parent but not retrievable — graph corruption.
            // The pipeline would normally reject this before we get here.
            // TODO(16.x): propagate BridgeError::NodeNotFound once
            // collect_steps returns Result.
            #[cfg(debug_assertions)]
            eprintln!("[ail-ui-bridge] WARNING: step node {child_id} not found; skipping");
            continue;
        };
        // Skip Promise nodes — they are contracts, not steps.
        if child_node.pattern == Pattern::Promise {
            continue;
        }

        let step_path = id_map.get_path(child_id).to_string();
        let step_name = child_node
            .metadata
            .name
            .as_deref()
            .unwrap_or(&child_node.intent)
            .to_string();

        // All nodes in a VerifiedGraph have passed contract checks (see
        // doc-comment above). Use rollup_from_contracts(false) to make the
        // derivation explicit rather than hardcoding Status::Ok.
        // TODO(16.x): replace `false` with real per-node failure data once
        // VerifiedGraph exposes Z3 counterexamples at node granularity.
        let step_status = rollup_from_contracts(false);
        step_statuses.push(step_status);

        let step_detail = build_node_detail(backend, child_id, step_status);
        detail.insert(step_path.clone(), step_detail);

        steps.push(StepJson {
            id: step_path,
            name: step_name,
            status: step_status,
            intent: child_node.intent.clone(),
            branch: None,
        });
    }

    let fn_status = rollup(&step_statuses);
    (steps, fn_status)
}

/// Build a `NodeDetail` for any node using the CIC context packet.
fn build_node_detail(backend: &dyn GraphBackend, id: NodeId, status: Status) -> NodeDetail {
    let Some(node) = backend.get_node(id).ok().flatten() else {
        return NodeDetail {
            name: String::new(),
            status,
            description: String::new(),
            receives: Vec::new(),
            returns: Vec::new(),
            rules: Vec::new(),
            inherited: Vec::new(),
            proven: Vec::new(),
            verification: VerificationDetail {
                ok: status != Status::Fail,
                counterexample: None,
            },
            code: None,
        };
    };

    let name = node
        .metadata
        .name
        .as_deref()
        .unwrap_or(&node.intent)
        .to_string();

    let receives: Vec<ReceivesEntry> = node
        .metadata
        .params
        .iter()
        .map(|p| ReceivesEntry {
            name: p.name.clone(),
            desc: p.type_ref.clone(),
        })
        .collect();

    let returns: Vec<ReturnsEntry> = node
        .metadata
        .return_type
        .as_deref()
        .map(|rt| {
            vec![ReturnsEntry {
                name: rt.to_string(),
                desc: String::new(),
            }]
        })
        .unwrap_or_default();

    let own_rules: Vec<RuleEntry> = node
        .contracts
        .iter()
        .map(|c| RuleEntry {
            text: c.expression.0.clone(),
            source: RuleSource::Own,
        })
        .collect();

    // Compute CIC packet for inherited rules and proven facts.
    let (inherited, proven) = if let Ok(packet) = compute_context_packet_for_backend(backend, id) {
        let inh: Vec<InheritedRule> = packet
            .inherited_constraints
            .iter()
            .map(|c| InheritedRule {
                text: c.expression.0.clone(),
                from: c.origin_node.to_string(),
            })
            .collect();
        let prov: Vec<String> = packet
            .promoted_facts
            .iter()
            .map(|f| f.condition.0.clone())
            .collect();
        (inh, prov)
    } else {
        (Vec::new(), Vec::new())
    };

    let mut rules = own_rules;
    for inh_rule in &inherited {
        rules.push(RuleEntry {
            text: inh_rule.text.clone(),
            source: RuleSource::Inherited,
        });
    }

    NodeDetail {
        name,
        status,
        description: node.intent.clone(),
        receives,
        returns,
        rules,
        inherited,
        proven,
        verification: VerificationDetail {
            ok: status != Status::Fail,
            counterexample: None,
        },
        code: None,
    }
}

/// Collect `Ed` cross-references from a node as `RelationJson` entries.
fn collect_relations(
    backend: &dyn GraphBackend,
    from_id: NodeId,
    id_map: &IdMap,
    relations: &mut Vec<RelationJson>,
) {
    let refs = backend.outgoing_diagonal_refs(from_id).unwrap_or_default();
    let from_path = id_map.get_path(from_id).to_string();

    for target_id in refs {
        let to_path = id_map.get_path(target_id).to_string();
        if to_path.is_empty() {
            continue;
        }

        let style = backend.get_node(target_id).ok().flatten().and_then(|n| {
            if n.pattern == Pattern::Describe {
                Some("data".to_string())
            } else {
                None
            }
        });

        relations.push(RelationJson {
            from: from_path.clone(),
            to: to_path,
            label: String::new(),
            style,
        });
    }
}

/// Compute an incremental diff between two `GraphJson` values.
///
/// A function present only in `next` is added; only in `prev` is removed;
/// in both with a changed status is modified. Module-level adds/removes use
/// `PatchItem::Module`. No `Full` variant exists (constraint 16.1-C).
pub fn diff_graph(prev: &GraphJson, next: &GraphJson) -> GraphPatchJson {
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut added: Vec<PatchItem> = Vec::new();
    let mut modified: Vec<PatchItem> = Vec::new();
    let mut removed: Vec<String> = Vec::new();

    // Build function lookup maps keyed by function id.
    let prev_fns: BTreeMap<String, &FunctionJson> = prev
        .modules
        .iter()
        .flat_map(|m| m.functions.iter().map(|f| (f.id.clone(), f)))
        .collect();
    let next_fns: BTreeMap<String, &FunctionJson> = next
        .modules
        .iter()
        .flat_map(|m| m.functions.iter().map(|f| (f.id.clone(), f)))
        .collect();

    // Build module lookup maps.
    let prev_mods: BTreeMap<String, &ModuleJson> =
        prev.modules.iter().map(|m| (m.id.clone(), m)).collect();
    let next_mods: BTreeMap<String, &ModuleJson> =
        next.modules.iter().map(|m| (m.id.clone(), m)).collect();

    // Module-level additions.
    for (mid, m) in &next_mods {
        if !prev_mods.contains_key(mid) {
            added.push(PatchItem::Module((*m).clone()));
        }
    }
    // Module-level removals.
    for mid in prev_mods.keys() {
        if !next_mods.contains_key(mid) {
            removed.push(mid.clone());
        }
    }

    // Function-level changes.
    for (fid, next_fn) in &next_fns {
        match prev_fns.get(fid) {
            None => added.push(PatchItem::Function((*next_fn).clone())),
            Some(prev_fn) => {
                if prev_fn.status != next_fn.status {
                    modified.push(PatchItem::Function((*next_fn).clone()));
                }
            }
        }
    }
    for fid in prev_fns.keys() {
        if !next_fns.contains_key(fid) {
            removed.push(fid.clone());
        }
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    GraphPatchJson {
        added,
        modified,
        removed,
        timestamp,
    }
}
