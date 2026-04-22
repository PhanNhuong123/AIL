//! Per-lens metric collectors.
//!
//! Five pure functions — one per lens — each taking the graph plus a resolved
//! scope and returning the matching `LensStats` variant. All collectors use
//! deterministic ordering (`BTreeSet` for de-dup, sorted iteration).

use std::collections::BTreeSet;

use crate::types::graph_json::GraphJson;
use crate::types::lens_stats::LensStats;
use crate::types::node_detail::NodeDetail;

use super::scope::Scope;

pub(super) fn lens_structure(graph: &GraphJson, scope: &Scope<'_>) -> LensStats {
    match scope {
        Scope::Project => {
            let modules = graph.modules.len();
            let functions: usize = graph.modules.iter().map(|m| m.functions.len()).sum();
            let steps: usize = graph
                .modules
                .iter()
                .flat_map(|m| m.functions.iter())
                .map(|f| f.steps.as_deref().map_or(0, |s| s.len()))
                .sum();
            let nodes = graph.project.node_count;
            LensStats::Structure {
                modules,
                functions,
                steps,
                nodes,
            }
        }
        Scope::Module(m) => {
            let functions = m.functions.len();
            let steps: usize = m
                .functions
                .iter()
                .map(|f| f.steps.as_deref().map_or(0, |s| s.len()))
                .sum();
            let nodes = m.node_count;
            LensStats::Structure {
                modules: 1,
                functions,
                steps,
                nodes,
            }
        }
        Scope::Function { function, .. } => {
            let steps = function.steps.as_deref().map_or(0, |s| s.len());
            LensStats::Structure {
                modules: 0,
                functions: 1,
                steps,
                nodes: steps + 1,
            }
        }
        Scope::Step { .. } => LensStats::Structure {
            modules: 0,
            functions: 0,
            steps: 1,
            nodes: 1,
        },
    }
}

/// Compute rule-coverage metrics for the given scope.
///
/// `total` counts every rule entry across all nodes in scope.
/// `broken` counts rules attached to any failing node (`verification.ok == false`).
/// If a node has 3 rules and fails, all 3 are counted as broken.
/// `unproven` remains 0 until Phase 16 surfaces per-rule Z3 counterexample evidence.
pub(super) fn lens_rules(graph: &GraphJson, scope: &Scope<'_>) -> LensStats {
    // Rules live in NodeDetail entries. We scan the detail map for entries
    // within the scope and count rule entries by source/verification status.
    let mut total = 0usize;
    let mut broken = 0usize;

    match scope {
        Scope::Project => {
            for detail in graph.detail.values() {
                total += detail.rules.len();
                if !detail.verification.ok {
                    broken += detail.rules.len();
                }
            }
        }
        Scope::Module(m) => {
            for path in module_path_ids(m) {
                if let Some(detail) = graph.detail.get(path) {
                    total += detail.rules.len();
                    if !detail.verification.ok {
                        broken += detail.rules.len();
                    }
                }
            }
        }
        Scope::Function { function, .. } => {
            if let Some(detail) = graph.detail.get(&function.id) {
                total += detail.rules.len();
                if !detail.verification.ok {
                    broken += detail.rules.len();
                }
            }
            if let Some(steps) = &function.steps {
                for step in steps {
                    if let Some(detail) = graph.detail.get(&step.id) {
                        total += detail.rules.len();
                        if !detail.verification.ok {
                            broken += detail.rules.len();
                        }
                    }
                }
            }
        }
        Scope::Step { step, .. } => {
            if let Some(detail) = graph.detail.get(&step.id) {
                total += detail.rules.len();
                if !detail.verification.ok {
                    broken += detail.rules.len();
                }
            }
        }
    }

    // `unproven` is a schema placeholder wired to 0 for Phase 15 because the
    // MVP pipeline marks every successfully-verified rule as proven. Phase 16
    // will populate this once per-rule Z3 counterexample evidence is available.
    LensStats::Rules {
        total,
        unproven: 0,
        broken,
    }
}

pub(super) fn lens_verify(graph: &GraphJson, scope: &Scope<'_>) -> LensStats {
    let paths: Vec<&str> = match scope {
        Scope::Project => graph.detail.keys().map(String::as_str).collect(),
        Scope::Module(m) => module_path_ids(m).collect(),
        Scope::Function { function, .. } => {
            let mut v: Vec<&str> = vec![function.id.as_str()];
            if let Some(steps) = &function.steps {
                v.extend(steps.iter().map(|s| s.id.as_str()));
            }
            v
        }
        Scope::Step { step, .. } => vec![step.id.as_str()],
    };

    let mut proven = 0usize;
    let mut unproven_count = 0usize;
    let mut counterexamples = 0usize;

    for path in paths {
        if let Some(detail) = graph.detail.get(path) {
            if detail.verification.ok {
                proven += 1;
            } else {
                unproven_count += 1;
                if detail.verification.counterexample.is_some() {
                    counterexamples += 1;
                }
            }
        }
    }

    LensStats::Verify {
        proven,
        unproven: unproven_count,
        counterexamples,
    }
}

pub(super) fn lens_data(graph: &GraphJson, scope: &Scope<'_>) -> LensStats {
    // Collect type names from node details' receives + returns within scope.
    // Deduplicate and sort for determinism.
    let mut type_set: BTreeSet<String> = BTreeSet::new();
    let mut signals = 0usize;

    match scope {
        Scope::Project => {
            for detail in graph.detail.values() {
                collect_types_from_detail(detail, &mut type_set);
            }
            signals += count_signals(graph);
        }
        Scope::Module(m) => {
            for path in module_path_ids(m) {
                if let Some(detail) = graph.detail.get(path) {
                    collect_types_from_detail(detail, &mut type_set);
                }
            }
            signals += m
                .functions
                .iter()
                .flat_map(|f| f.steps.iter().flatten())
                .filter(|s| s.branch.is_some())
                .count();
        }
        Scope::Function { function, .. } => {
            if let Some(detail) = graph.detail.get(&function.id) {
                collect_types_from_detail(detail, &mut type_set);
            }
            if let Some(steps) = &function.steps {
                for step in steps {
                    if let Some(detail) = graph.detail.get(&step.id) {
                        collect_types_from_detail(detail, &mut type_set);
                    }
                    if step.branch.is_some() {
                        signals += 1;
                    }
                }
            }
        }
        Scope::Step { step, .. } => {
            if let Some(detail) = graph.detail.get(&step.id) {
                collect_types_from_detail(detail, &mut type_set);
            }
            if step.branch.is_some() {
                signals += 1;
            }
        }
    }

    LensStats::Data {
        types: type_set.into_iter().collect(),
        signals,
    }
}

/// Phase 15 placeholder — wallet_service has no test markers yet.
pub(super) fn lens_tests() -> LensStats {
    LensStats::Tests {
        total: 0,
        passing: 0,
        failing: 0,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn module_path_ids(m: &crate::types::graph_json::ModuleJson) -> impl Iterator<Item = &str> {
    std::iter::once(m.id.as_str())
        .chain(m.functions.iter().map(|f| f.id.as_str()))
        .chain(
            m.functions
                .iter()
                .flat_map(|f| f.steps.iter().flatten().map(|s| s.id.as_str())),
        )
}

fn collect_types_from_detail(detail: &NodeDetail, type_set: &mut BTreeSet<String>) {
    for r in &detail.receives {
        if !r.desc.is_empty() {
            type_set.insert(r.desc.clone());
        }
    }
    for r in &detail.returns {
        if !r.name.is_empty() {
            type_set.insert(r.name.clone());
        }
    }
}

fn count_signals(graph: &GraphJson) -> usize {
    let branch_steps: usize = graph
        .modules
        .iter()
        .flat_map(|m| m.functions.iter())
        .flat_map(|f| f.steps.iter().flatten())
        .filter(|s| s.branch.is_some())
        .count();

    let data_relations: usize = graph
        .relations
        .iter()
        .filter(|r| r.style.as_deref() == Some("data"))
        .count();

    branch_steps + data_relations
}
