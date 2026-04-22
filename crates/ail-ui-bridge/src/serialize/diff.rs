use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::graph_json::{FunctionJson, GraphJson, ModuleJson, StepJson};
use crate::types::patch::{
    FunctionPatchEntry, FunctionRemoval, GraphPatchJson, StepPatchEntry, StepRemoval,
};

/// Compute an incremental diff between two `GraphJson` values.
///
/// Delegates to [`diff_graph_at`] with the current wall-clock second as the
/// timestamp. No `Full` variant exists — patches must never trigger a full
/// reload (constraint 16.1-C).
pub fn diff_graph(prev: &GraphJson, next: &GraphJson) -> GraphPatchJson {
    diff_graph_at(prev, next, now_secs())
}

/// Compute an incremental diff between two `GraphJson` values with a
/// caller-supplied timestamp.
///
/// Exposed for deterministic testing: callers pass `0` or a fixed value so
/// the timestamp does not vary between test runs.
///
/// The diff covers three granularity levels:
/// - **Module**: added/modified/removed by id. "Modified" = same id but
///   semantic hash (id, name, description, cluster, status, node_count) changed.
/// - **Function**: added/modified/removed. Each entry carries its `module_id`.
///   "Modified" = same id, different semantic hash (id, name, status).
/// - **Step**: added/modified/removed. Each entry carries its `function_id`.
///   "Modified" = same id, different semantic hash (id, name, status, intent,
///   branch).
///
/// All outputs are deterministic: input maps use `BTreeMap`, outputs are sorted
/// by the entity id.
pub fn diff_graph_at(prev: &GraphJson, next: &GraphJson, now: u64) -> GraphPatchJson {
    // --- Module diff ---
    let prev_mods: BTreeMap<&str, &ModuleJson> =
        prev.modules.iter().map(|m| (m.id.as_str(), m)).collect();
    let next_mods: BTreeMap<&str, &ModuleJson> =
        next.modules.iter().map(|m| (m.id.as_str(), m)).collect();

    let mut modules_added: Vec<ModuleJson> = Vec::new();
    let mut modules_modified: Vec<ModuleJson> = Vec::new();
    let mut modules_removed: Vec<String> = Vec::new();

    for (mid, m) in &next_mods {
        match prev_mods.get(mid) {
            None => modules_added.push((*m).clone()),
            Some(prev_m) => {
                if module_hash(m) != module_hash(prev_m) {
                    modules_modified.push((*m).clone());
                }
            }
        }
    }
    for mid in prev_mods.keys() {
        if !next_mods.contains_key(mid) {
            modules_removed.push(mid.to_string());
        }
    }
    modules_removed.sort();

    // --- Function diff ---
    // Index: function_id -> (module_id, FunctionJson)
    let prev_fns: BTreeMap<&str, (&str, &FunctionJson)> = prev
        .modules
        .iter()
        .flat_map(|m| {
            m.functions
                .iter()
                .map(|f| (f.id.as_str(), (m.id.as_str(), f)))
        })
        .collect();
    let next_fns: BTreeMap<&str, (&str, &FunctionJson)> = next
        .modules
        .iter()
        .flat_map(|m| {
            m.functions
                .iter()
                .map(|f| (f.id.as_str(), (m.id.as_str(), f)))
        })
        .collect();

    let mut functions_added: Vec<FunctionPatchEntry> = Vec::new();
    let mut functions_modified: Vec<FunctionPatchEntry> = Vec::new();
    let mut functions_removed: Vec<FunctionRemoval> = Vec::new();

    for (fid, (module_id, f)) in &next_fns {
        match prev_fns.get(fid) {
            None => functions_added.push(FunctionPatchEntry {
                module_id: module_id.to_string(),
                function: (*f).clone(),
            }),
            Some((_, prev_f)) => {
                if function_hash(f) != function_hash(prev_f) {
                    functions_modified.push(FunctionPatchEntry {
                        module_id: module_id.to_string(),
                        function: (*f).clone(),
                    });
                }
            }
        }
    }
    for (fid, (module_id, _)) in &prev_fns {
        if !next_fns.contains_key(fid) {
            functions_removed.push(FunctionRemoval {
                module_id: module_id.to_string(),
                function_id: fid.to_string(),
            });
        }
    }
    functions_removed.sort_by(|a, b| a.function_id.cmp(&b.function_id));

    // --- Step diff ---
    // Index: step_id -> (function_id, StepJson)
    let prev_steps: BTreeMap<&str, (&str, &StepJson)> = prev
        .modules
        .iter()
        .flat_map(|m| m.functions.iter())
        .flat_map(|f| {
            f.steps
                .iter()
                .flatten()
                .map(|s| (s.id.as_str(), (f.id.as_str(), s)))
        })
        .collect();
    let next_steps: BTreeMap<&str, (&str, &StepJson)> = next
        .modules
        .iter()
        .flat_map(|m| m.functions.iter())
        .flat_map(|f| {
            f.steps
                .iter()
                .flatten()
                .map(|s| (s.id.as_str(), (f.id.as_str(), s)))
        })
        .collect();

    let mut steps_added: Vec<StepPatchEntry> = Vec::new();
    let mut steps_modified: Vec<StepPatchEntry> = Vec::new();
    let mut steps_removed: Vec<StepRemoval> = Vec::new();

    for (sid, (function_id, s)) in &next_steps {
        match prev_steps.get(sid) {
            None => steps_added.push(StepPatchEntry {
                function_id: function_id.to_string(),
                step: (*s).clone(),
            }),
            Some((_, prev_s)) => {
                if step_hash(s) != step_hash(prev_s) {
                    steps_modified.push(StepPatchEntry {
                        function_id: function_id.to_string(),
                        step: (*s).clone(),
                    });
                }
            }
        }
    }
    for (sid, (function_id, _)) in &prev_steps {
        if !next_steps.contains_key(sid) {
            steps_removed.push(StepRemoval {
                function_id: function_id.to_string(),
                step_id: sid.to_string(),
            });
        }
    }
    steps_removed.sort_by(|a, b| a.step_id.cmp(&b.step_id));

    GraphPatchJson {
        modules_added,
        modules_modified,
        modules_removed,
        functions_added,
        functions_modified,
        functions_removed,
        steps_added,
        steps_modified,
        steps_removed,
        timestamp: now,
    }
}

/// Return current Unix time in seconds.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Compute a semantic hash for a module (excludes child collections so only
/// module-level metadata changes trigger a "modified" entry).
fn module_hash(m: &ModuleJson) -> u64 {
    // Serialize the semantic fields to a stable JSON string, then hash.
    let repr = serde_json::to_string(&serde_json::json!({
        "id": m.id,
        "name": m.name,
        "description": m.description,
        "cluster": m.cluster,
        "status": m.status,
        "nodeCount": m.node_count,
    }))
    .unwrap_or_default();
    hash_str(&repr)
}

/// Compute a semantic hash for a function.
fn function_hash(f: &FunctionJson) -> u64 {
    let repr = serde_json::to_string(&serde_json::json!({
        "id": f.id,
        "name": f.name,
        "status": f.status,
    }))
    .unwrap_or_default();
    hash_str(&repr)
}

/// Compute a semantic hash for a step.
fn step_hash(s: &StepJson) -> u64 {
    let repr = serde_json::to_string(&serde_json::json!({
        "id": s.id,
        "name": s.name,
        "status": s.status,
        "intent": s.intent,
        "branch": s.branch,
    }))
    .unwrap_or_default();
    hash_str(&repr)
}

/// Hash values are stable within a single process only; do not persist them.
fn hash_str(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}
