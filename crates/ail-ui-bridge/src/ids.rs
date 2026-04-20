use std::collections::BTreeMap;

use ail_graph::{GraphBackend, NodeId, Pattern};

/// Stable path-like string IDs for graph nodes.
///
/// IDs look like `wallet_service.src.transfer_money.01_validate`.
/// Collisions (two siblings with the same sanitised name) are resolved by
/// appending the last 8 hex chars of the UUID.
///
/// Both maps use [`BTreeMap`] for deterministic iteration order, satisfying
/// the CLAUDE.md requirement. `NodeId` does not implement `Ord`, so the
/// forward map is keyed by the UUID string representation instead.
pub struct IdMap {
    /// UUID string → path-like string (deterministic via BTreeMap).
    pub forward: BTreeMap<String, String>,
    /// path-like string → `NodeId` (deterministic via BTreeMap).
    pub reverse: BTreeMap<String, NodeId>,
}

impl IdMap {
    /// Build the ID map by depth-first walking the graph from all root nodes.
    pub fn build(backend: &dyn GraphBackend) -> Self {
        let mut map = IdMap {
            forward: BTreeMap::new(),
            reverse: BTreeMap::new(),
        };

        let roots = backend.root_nodes().unwrap_or_default();
        for root_id in roots {
            let root_segment = node_segment(backend, root_id);
            map.insert(root_id, root_segment.clone());
            map.walk_children(backend, root_id, &root_segment);
        }

        map
    }

    fn walk_children(&mut self, backend: &dyn GraphBackend, parent_id: NodeId, parent_path: &str) {
        let children = backend.children(parent_id).unwrap_or_default();
        // Track segments used at this level to detect collisions.
        // BTreeMap for deterministic collision resolution order.
        let mut seen: BTreeMap<String, u32> = BTreeMap::new();

        for child_id in children {
            let raw_seg = node_segment(backend, child_id);
            let count = seen.entry(raw_seg.clone()).or_insert(0);

            let segment = if *count == 0 {
                // Check if this exact path is already claimed by a different node.
                let candidate = format!("{parent_path}.{raw_seg}");
                if self.reverse.contains_key(&candidate) {
                    // Collision — append last 8 chars of UUID.
                    let suffix = uuid_suffix(child_id);
                    format!("{parent_path}.{raw_seg}-{suffix}")
                } else {
                    candidate
                }
            } else {
                // Sibling with same segment — append UUID suffix to distinguish.
                let suffix = uuid_suffix(child_id);
                format!("{parent_path}.{raw_seg}-{suffix}")
            };

            *count += 1;
            self.insert(child_id, segment.clone());
            self.walk_children(backend, child_id, &segment);
        }
    }

    fn insert(&mut self, id: NodeId, path: String) {
        self.forward.insert(id.to_string(), path.clone());
        self.reverse.insert(path, id);
    }

    /// Look up the path ID for a `NodeId`. Returns an empty string if not found.
    pub fn get_path(&self, id: NodeId) -> &str {
        self.forward
            .get(&id.to_string())
            .map(String::as_str)
            .unwrap_or("")
    }

    /// Look up the `NodeId` for a path string.
    pub fn get_id(&self, path: &str) -> Option<NodeId> {
        self.reverse.get(path).copied()
    }
}

/// Derive a sanitised path segment for a node.
///
/// Uses `metadata.name` when present, falling back to `intent`. Replaces
/// whitespace and non-alphanumeric characters with underscores and lowercases
/// the result.
fn node_segment(backend: &dyn GraphBackend, id: NodeId) -> String {
    let Some(node) = backend.get_node(id).ok().flatten() else {
        return uuid_suffix(id);
    };

    let raw = node
        .metadata
        .name
        .as_deref()
        .unwrap_or(&node.intent)
        .to_lowercase();

    sanitise(&raw)
}

/// Sanitise a string to a safe path segment: keep alphanumerics and underscores,
/// replace everything else with `_`, collapse runs, strip leading/trailing `_`.
pub fn sanitise(s: &str) -> String {
    // Map each character to itself (alphanumeric / underscore) or to a sentinel
    // `None` to mark a run boundary, then split on `None` and join with `_`.
    // This single pass replaces the previous O(n²) while-replace loop.
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                Some(c)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .split(|opt| opt.is_none())
        .filter_map(|chunk| {
            let seg: String = chunk.iter().filter_map(|o| *o).collect();
            if seg.is_empty() {
                None
            } else {
                Some(seg)
            }
        })
        .collect::<Vec<_>>()
        .join("_")
}

/// Return the last 8 hex characters of a `NodeId` UUID for collision suffixes.
fn uuid_suffix(id: NodeId) -> String {
    let s = id.to_string().replace('-', "");
    let len = s.len();
    if len >= 8 {
        s[len - 8..].to_string()
    } else {
        s
    }
}

/// Classify a node as "external" based on its pattern.
///
/// MVP: no nodes are external in the default cluster setup.
///
/// TODO(16.x): implement pattern-based external classification
/// (e.g. nodes imported from other modules or marked with a special tag).
#[allow(dead_code)]
pub(crate) fn is_external(_pattern: &Pattern) -> bool {
    false
}
