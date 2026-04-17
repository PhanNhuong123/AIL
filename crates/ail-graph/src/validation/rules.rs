use std::collections::{HashMap, HashSet, VecDeque};

use crate::constants::BUILTIN_TYPE_NAMES;
use crate::errors::ValidationError;
use crate::graph::{AilGraph, GraphBackend};
use crate::types::{ContractKind, NodeId, Pattern};

/// Run all validation rules against `graph` and return every error found.
/// Returns an empty `Vec` when the graph is fully valid.
pub(crate) fn run_all_rules(graph: &AilGraph) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    check_intent_non_empty(graph, &mut errors);
    let reachable = check_ev_tree_and_collect_reachable(graph, &mut errors);
    check_all_nodes_reachable(graph, &reachable, &mut errors);
    check_only_leaves_have_expressions(graph, &mut errors);
    check_do_has_pre_and_post_contracts(graph, &mut errors);
    check_type_refs_resolve(graph, &mut errors);
    check_no_duplicate_names_in_scope(graph, &mut errors);
    check_following_template_phases(graph, &mut errors);
    check_using_do_nodes(graph, &mut errors);
    errors
}

// ─── v001: every node has a non-empty intent ──────────────────────────────────

fn check_intent_non_empty(graph: &AilGraph, errors: &mut Vec<ValidationError>) {
    for node in graph.all_nodes() {
        if node.intent.trim().is_empty() {
            errors.push(ValidationError::EmptyIntent { node_id: node.id });
        }
    }
}

// ─── v002: Ev edges form a tree  +  v003 reachability helper ─────────────────

/// BFS from root following Ev edges. Returns the set of reachable node IDs.
/// Pushes `MissingRoot`, `EvMultipleParents`, and `EvCycleDetected` as needed.
///
/// The `queued` set prevents adding the same node to the queue more than once,
/// which avoids:
/// - Duplicate `EvMultipleParents` errors (one per parent, not one total)
/// - Spurious `EvCycleDetected` on diamond-shaped Ev graphs (two paths to the
///   same node is a multi-parent violation, not a cycle)
fn check_ev_tree_and_collect_reachable(
    graph: &AilGraph,
    errors: &mut Vec<ValidationError>,
) -> HashSet<NodeId> {
    let Some(root_id) = graph.root_id() else {
        errors.push(ValidationError::MissingRoot);
        return HashSet::new();
    };

    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut queued: HashSet<NodeId> = HashSet::new();
    let mut queue: VecDeque<NodeId> = VecDeque::new();
    queued.insert(root_id);
    queue.push_back(root_id);

    while let Some(current_id) = queue.pop_front() {
        visited.insert(current_id);

        let children = graph.children_of(current_id).unwrap_or_default();

        for child_id in children {
            // Emit EvMultipleParents once per child (checked on first encounter).
            let parents = collect_ev_parents(graph, child_id);
            if parents.len() > 1 {
                errors.push(ValidationError::EvMultipleParents {
                    node_id: child_id,
                    parents,
                });
            }

            if visited.contains(&child_id) {
                // Back-edge to an already-processed node — genuine cycle.
                errors.push(ValidationError::EvCycleDetected {
                    cycle: vec![child_id],
                });
            } else if !queued.contains(&child_id) {
                // First encounter — schedule for processing.
                queued.insert(child_id);
                queue.push_back(child_id);
            }
            // Already queued but not yet visited = diamond path.
            // Caught by EvMultipleParents above; do not re-queue.
        }
    }

    visited
}

/// Collect all node IDs that have an Ev edge pointing to `node_id`.
fn collect_ev_parents(graph: &AilGraph, node_id: NodeId) -> Vec<NodeId> {
    // `parent_of` returns the single Ev parent (or None).
    // To detect multiple parents we need to look at the raw edge count, but
    // `parent_of` only returns the first one found. We rely on `children_of`
    // uniqueness guarantees from petgraph; a true multi-parent scenario is only
    // possible if `add_edge` was called twice. We check by collecting all
    // incoming Ev edges via a helper that returns a Vec.
    collect_all_ev_parents(graph, node_id)
}

/// Walk all edges in the graph to find every Ev-parent of `node_id`.
///
/// This is O(E) but validation is a one-time pass; correctness > speed here.
fn collect_all_ev_parents(graph: &AilGraph, target_id: NodeId) -> Vec<NodeId> {
    let mut parents = Vec::new();
    for candidate_id in graph.node_ids() {
        let children = graph.children_of(candidate_id).unwrap_or_default();
        if children.contains(&target_id) {
            parents.push(candidate_id);
        }
    }
    parents
}

// ─── v003: all nodes reachable from root ─────────────────────────────────────

fn check_all_nodes_reachable(
    graph: &AilGraph,
    reachable: &HashSet<NodeId>,
    errors: &mut Vec<ValidationError>,
) {
    for node_id in graph.node_ids() {
        if !reachable.contains(&node_id) {
            errors.push(ValidationError::UnreachableNode { node_id });
        }
    }
}

// ─── v004: only leaf nodes carry expressions ──────────────────────────────────

fn check_only_leaves_have_expressions(graph: &AilGraph, errors: &mut Vec<ValidationError>) {
    for node in graph.all_nodes() {
        if node.children.is_some() && node.expression.is_some() {
            errors.push(ValidationError::ExpressionOnStructuralNode { node_id: node.id });
        }
    }
}

// ─── v005: top-level Do has ≥1 Before + ≥1 After contract ───────────────────

fn check_do_has_pre_and_post_contracts(graph: &AilGraph, errors: &mut Vec<ValidationError>) {
    for node in graph.all_nodes() {
        if node.pattern != Pattern::Do {
            continue;
        }
        if !is_top_level_do(node.id, graph) {
            continue;
        }
        let has_before = node
            .contracts
            .iter()
            .any(|c| c.kind == ContractKind::Before);
        let has_after = node.contracts.iter().any(|c| c.kind == ContractKind::After);
        if !has_before {
            errors.push(ValidationError::MissingPreContract { node_id: node.id });
        }
        if !has_after {
            errors.push(ValidationError::MissingPostContract { node_id: node.id });
        }
    }
}

/// A `Do` node is "top-level" when its parent (via Ev) is not itself a `Do` node.
fn is_top_level_do(node_id: NodeId, graph: &AilGraph) -> bool {
    match graph.parent_of(node_id) {
        Ok(Some(parent_id)) => graph
            .get_node(parent_id)
            .map(|n| n.pattern != Pattern::Do)
            .unwrap_or(true),
        _ => true,
    }
}

// ─── v006: all type references resolve ───────────────────────────────────────

fn check_type_refs_resolve(graph: &AilGraph, errors: &mut Vec<ValidationError>) {
    // Collect names of all graph-defined types (Define, Describe, Error).
    let defined_names: HashSet<&str> = graph
        .all_nodes()
        .filter(|n| {
            matches!(
                n.pattern,
                Pattern::Define | Pattern::Describe | Pattern::Error
            )
        })
        .filter_map(|n| n.metadata.name.as_deref())
        .collect();

    for node in graph.all_nodes() {
        let m = &node.metadata;

        // Gather all type_ref strings from this node's metadata.
        let mut refs: Vec<&str> = Vec::new();
        for param in &m.params {
            refs.push(&param.type_ref);
        }
        if let Some(rt) = m.return_type.as_deref() {
            refs.push(rt);
        }
        if let Some(bt) = m.base_type.as_deref() {
            refs.push(bt);
        }
        for field in &m.fields {
            refs.push(&field.type_ref);
        }
        for field in &m.carries {
            refs.push(&field.type_ref);
        }

        for type_ref in refs {
            if type_ref.is_empty() {
                continue;
            }
            // TODO(phase-2): resolve qualified names (containing '.') against the module graph.
            if type_ref.contains('.') {
                continue;
            }
            if !defined_names.contains(type_ref) && !BUILTIN_TYPE_NAMES.contains(&type_ref) {
                errors.push(ValidationError::UnresolvedTypeReference {
                    node_id: node.id,
                    type_ref: type_ref.to_owned(),
                });
            }
        }
    }
}

// ─── v007: no duplicate names within a scope ─────────────────────────────────

fn check_no_duplicate_names_in_scope(graph: &AilGraph, errors: &mut Vec<ValidationError>) {
    // Scope = direct children of a single parent node.
    // Two nodes in different scopes may share names.
    for node in graph.all_nodes() {
        if node.children.is_none() {
            continue; // leaf — no children to check
        }
        let children = graph.children_of(node.id).unwrap_or_default();

        // Group children by their metadata.name.
        let mut name_to_ids: HashMap<&str, Vec<NodeId>> = HashMap::new();
        for child_id in &children {
            if let Ok(child) = graph.get_node(*child_id) {
                if let Some(name) = child.metadata.name.as_deref() {
                    name_to_ids.entry(name).or_default().push(*child_id);
                }
            }
        }

        for (name, ids) in name_to_ids {
            if ids.len() > 1 {
                errors.push(ValidationError::DuplicateNameInScope {
                    name: name.to_owned(),
                    scope_id: node.id,
                    node_ids: ids,
                });
            }
        }
    }
}

// ─── v008: following template phases ─────────────────────────────────────────

fn check_following_template_phases(graph: &AilGraph, errors: &mut Vec<ValidationError>) {
    for node in graph.all_nodes() {
        if node.pattern != Pattern::Do {
            continue;
        }
        // using-Do nodes point at a shared pattern and do not implement template
        // phases themselves.
        if node.metadata.using_pattern_name.is_some() {
            continue;
        }
        // Navigate by metadata: the `following <name>` clause authoritatively
        // identifies the template. Ed edges from a Do can also represent type,
        // error, or call references (see MCP auto-edge detection), so they
        // cannot be used as the template set.
        let template_name = match &node.metadata.following_template_name {
            Some(name) => name,
            None => continue,
        };
        let template_ids = graph.find_by_name(template_name).unwrap_or_default();
        if template_ids.is_empty() {
            continue;
        }

        let implemented_phases: HashSet<String> = graph
            .children_of(node.id)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|child_id| {
                graph
                    .get_node(child_id)
                    .ok()
                    .and_then(|c| c.metadata.name.clone())
            })
            .collect();

        for template_id in template_ids {
            let required_phases: Vec<String> = graph
                .children_of(template_id)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|child_id| {
                    graph
                        .get_node(child_id)
                        .ok()
                        .and_then(|c| c.metadata.name.clone())
                })
                .collect();

            for phase in required_phases {
                if !implemented_phases.contains(&phase) {
                    errors.push(ValidationError::MissingTemplatePhase {
                        node_id: node.id,
                        phase,
                    });
                }
            }
        }
    }
}

// ─── v009: using-Do node constraints ─────────────────────────────────────────

fn check_using_do_nodes(graph: &AilGraph, errors: &mut Vec<ValidationError>) {
    for node in graph.all_nodes() {
        if node.pattern != Pattern::Do {
            continue;
        }
        if node.metadata.using_pattern_name.is_none() {
            continue;
        }
        // A using-Do must be a leaf (children must be None).
        if node.children.is_some() {
            errors.push(ValidationError::UsingDoHasChildren { node_id: node.id });
        }
        // A using-Do must have at least one outgoing Ed edge (wired by assembler).
        let diagonal_refs = graph.outgoing_diagonal_refs_of(node.id).unwrap_or_default();
        if diagonal_refs.is_empty() {
            errors.push(ValidationError::UsingDoMissingEdge { node_id: node.id });
        }
    }
}
