use ail_graph::NodeId;
use ail_ui_bridge::flowchart::{build_flowchart, generate_fallback_flowchart};
use ail_ui_bridge::ids::IdMap;
use ail_ui_bridge::pipeline::load_verified_from_path;
use ail_ui_bridge::types::flowchart::FlowNodeKind;
use std::path::{Path, PathBuf};
use std::str::FromStr;

fn wallet_src_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("wallet_service")
        .join("src")
}

/// Test 6: transfer_money flowchart has >= 3 nodes (Start + steps + End),
/// first is Start, last is End.
#[test]
fn test_flowchart_transfer() {
    let path = wallet_src_path();
    let verified = load_verified_from_path(&path).expect("wallet pipeline must succeed");
    let id_map = IdMap::build(verified.graph());

    // Find the transfer_money Do function path ID.
    // `id_map.forward` is keyed by UUID string since NodeId has no Ord.
    // Parse each key back to NodeId for the graph lookup.
    let transfer_path = id_map
        .forward
        .iter()
        .find(|(uuid_str, _)| {
            NodeId::from_str(uuid_str)
                .ok()
                .and_then(|node_id| verified.graph().get_node(node_id).ok().flatten())
                .map(|n| n.pattern == ail_graph::Pattern::Do && n.intent.contains("transfer"))
                .unwrap_or(false)
        })
        .map(|(_, v)| v.clone())
        .expect("transfer_money Do function must be in the id map");

    let chart = build_flowchart(verified.graph(), &id_map, &transfer_path)
        .expect("build_flowchart must succeed for transfer_money");

    assert!(
        chart.nodes.len() >= 3,
        "transfer_money flowchart must have >= 3 nodes (start + steps + end), got {}",
        chart.nodes.len()
    );
    assert_eq!(
        chart.nodes[0].kind,
        FlowNodeKind::Start,
        "first flowchart node must be Start"
    );
    assert_eq!(
        chart.nodes.last().unwrap().kind,
        FlowNodeKind::End,
        "last flowchart node must be End"
    );
}

/// Test 7: a node with no eligible children returns a [Start, End] fallback.
#[test]
fn test_flowchart_fallback_generation() {
    let chart = generate_fallback_flowchart("test.fn");

    assert_eq!(chart.nodes.len(), 2, "fallback must have exactly 2 nodes");
    assert_eq!(chart.nodes[0].kind, FlowNodeKind::Start);
    assert_eq!(chart.nodes[1].kind, FlowNodeKind::End);
    assert_eq!(chart.edges.len(), 1, "fallback must have exactly 1 edge");
    assert_eq!(chart.edges[0].from, "test.fn.start");
    assert_eq!(chart.edges[0].to, "test.fn.end");
}
