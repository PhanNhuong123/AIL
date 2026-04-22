use std::collections::BTreeMap;

use ail_graph::GraphBackend;

use crate::ids::IdMap;
use crate::types::graph_json::ExternalJson;

/// Classify external nodes by walking every node in the id map and checking
/// whether any of their outgoing diagonal refs target nodes not indexed by the
/// project walk (i.e. `id_map.get_path(target)` returns `""`).
///
/// Returns a deterministically sorted `Vec<ExternalJson>` (deduplicated by id).
/// For wallet_service, this returns an empty Vec because all `Ed` refs stay
/// within the project graph.
pub fn classify_externals(backend: &dyn GraphBackend, id_map: &IdMap) -> Vec<ExternalJson> {
    // BTreeMap ensures deterministic iteration and automatic deduplication.
    let mut result: BTreeMap<String, ExternalJson> = BTreeMap::new();

    for node_id in id_map.reverse.values() {
        let refs = backend.outgoing_diagonal_refs(*node_id).unwrap_or_default();
        for target_id in refs {
            let target_path = id_map.get_path(target_id);
            if !target_path.is_empty() {
                // Already indexed — not external.
                continue;
            }

            // Target not in id_map — it's external.
            let target_uuid = target_id.to_string().replace('-', "");
            let suffix_start = target_uuid.len().saturating_sub(8);
            let id = format!("external_{}", &target_uuid[suffix_start..]);

            if result.contains_key(&id) {
                continue;
            }

            let name = backend
                .get_node(target_id)
                .ok()
                .flatten()
                .and_then(|n| n.metadata.name.clone())
                .unwrap_or_else(|| id.clone());

            result.insert(
                id.clone(),
                ExternalJson {
                    id,
                    name,
                    description: None,
                },
            );
        }
    }

    result.into_values().collect()
}
