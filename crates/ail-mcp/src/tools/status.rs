//! `ail.status` tool — project pipeline stage and counts.

use ail_graph::types::Pattern;

use crate::context::ProjectContext;
use crate::types::tool_io::StatusOutput;

/// Return a snapshot of the current pipeline stage, node count, edge count,
/// and number of `Do` nodes (each of which must carry contracts).
pub(crate) fn run_status(context: &ProjectContext) -> StatusOutput {
    let graph = context.graph();
    let do_node_count = graph
        .all_nodes()
        .filter(|n| n.pattern == Pattern::Do)
        .count();

    StatusOutput {
        pipeline_stage: context.stage_name().to_owned(),
        node_count: graph.node_count(),
        edge_count: graph.edge_count(),
        do_node_count,
    }
}
