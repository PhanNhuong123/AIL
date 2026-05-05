//! Sidecar layout payload.
//!
//! AIL grammar deliberately has no `@layout(x, y)` annotation — node
//! coordinates are not part of the source-of-truth `.ail` text. v4.1 stores
//! per-project layout overrides in a sidecar JSON file at
//! `<project>/.ail/layout.json`. The frontend overrides its computed
//! swim/system layout with these saved coordinates whenever an entry exists.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One node's persisted layout. Coordinates are in the same SVG-space the
/// frontend swim/system layout helpers use.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LayoutEntry {
    pub x: f64,
    pub y: f64,
}

/// Project-wide layout overrides.
///
/// `nodes` is keyed by the path-like ID exposed in `GraphJson` (e.g.
/// `wallet_service.src.transfer_money.s_validate`). Missing keys mean the
/// frontend should keep its computed layout for that node.
///
/// Schema is intentionally flat — extending later (per-edge routing,
/// per-function viewport) only requires adding sibling fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectLayout {
    /// Schema version for forward-compatible reads. Bump when the on-disk
    /// shape gains breaking fields.
    #[serde(default = "default_layout_version")]
    pub version: u32,
    #[serde(default)]
    pub nodes: HashMap<String, LayoutEntry>,
}

fn default_layout_version() -> u32 {
    ProjectLayout::CURRENT_VERSION
}

impl Default for ProjectLayout {
    fn default() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            nodes: HashMap::new(),
        }
    }
}

impl ProjectLayout {
    pub const CURRENT_VERSION: u32 = 1;

    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace `entry` for `node_id`. Returns `Self` so callers can
    /// chain in tests.
    pub fn with_node(mut self, node_id: impl Into<String>, entry: LayoutEntry) -> Self {
        self.nodes.insert(node_id.into(), entry);
        self
    }

    /// Merge `updates` into `self`, overwriting any existing entries.
    pub fn merge(&mut self, updates: HashMap<String, LayoutEntry>) {
        self.nodes.extend(updates);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_default_empty() {
        let l = ProjectLayout::default();
        assert_eq!(l.version, 1);
        assert!(l.nodes.is_empty());
    }

    #[test]
    fn test_layout_merge_overwrites_existing() {
        let mut l = ProjectLayout::new()
            .with_node("a", LayoutEntry { x: 0.0, y: 0.0 })
            .with_node("b", LayoutEntry { x: 10.0, y: 10.0 });
        let mut updates = HashMap::new();
        updates.insert("a".to_string(), LayoutEntry { x: 99.0, y: 99.0 });
        updates.insert("c".to_string(), LayoutEntry { x: 5.0, y: 5.0 });
        l.merge(updates);
        assert_eq!(l.nodes["a"], LayoutEntry { x: 99.0, y: 99.0 });
        assert_eq!(l.nodes["b"], LayoutEntry { x: 10.0, y: 10.0 });
        assert_eq!(l.nodes["c"], LayoutEntry { x: 5.0, y: 5.0 });
    }

    #[test]
    fn test_layout_serde_roundtrip() {
        let layout = ProjectLayout::new()
            .with_node("foo", LayoutEntry { x: 1.5, y: 2.5 })
            .with_node("bar", LayoutEntry { x: -3.0, y: 4.0 });
        let json = serde_json::to_string(&layout).expect("serialize");
        // Schema must be camelCase for frontend mirror.
        assert!(json.contains("\"version\":1"));
        let back: ProjectLayout = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, layout);
    }

    #[test]
    fn test_layout_missing_version_defaults_to_one() {
        let json = "{ \"nodes\": {} }";
        let layout: ProjectLayout = serde_json::from_str(json).expect("deserialize");
        assert_eq!(layout.version, 1);
    }

    #[test]
    fn test_layout_missing_nodes_defaults_to_empty() {
        let json = "{ \"version\": 1 }";
        let layout: ProjectLayout = serde_json::from_str(json).expect("deserialize");
        assert!(layout.nodes.is_empty());
    }
}
