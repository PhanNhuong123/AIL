//! Lens metrics — pure projection of `GraphJson` into per-lens stats.
//!
//! `compute_lens_metrics` is the single source of truth for lens stats.
//! The frontend must not recompute these values (invariant 16.1-D).

mod collectors;
mod scope;

use crate::types::graph_json::GraphJson;
use crate::types::lens_stats::{Lens, LensStats};

use scope::{resolve_scope, Scope};

/// Compute per-lens metrics for an optional scope within `graph`.
///
/// - If `scope_id` is `None`, metrics cover the whole project.
/// - If `scope_id` is `Some(id)` and the id resolves to a known node, metrics
///   are restricted to that scope.
/// - If `scope_id` is `Some(id)` and the id does not resolve, returns
///   `LensStats::zero(lens)`.
pub fn compute_lens_metrics(graph: &GraphJson, lens: Lens, scope_id: Option<&str>) -> LensStats {
    let scope = match scope_id {
        None => Scope::Project,
        Some(id) => match resolve_scope(graph, id) {
            Some(s) => s,
            None => return LensStats::zero(lens),
        },
    };

    match lens {
        Lens::Structure => collectors::lens_structure(graph, &scope),
        Lens::Rules => collectors::lens_rules(graph, &scope),
        Lens::Verify => collectors::lens_verify(graph, &scope),
        Lens::Data => collectors::lens_data(graph, &scope),
        Lens::Tests => collectors::lens_tests(),
    }
}
