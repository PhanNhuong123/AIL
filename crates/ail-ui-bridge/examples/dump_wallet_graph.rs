// Acceptance helper: dump the wallet_service example as GraphJson so the
// IDE browser preview can fetch it and exercise SystemView / ModuleView /
// FlowView / NodeView / Outline / breadcrumbs against real parser output.
//
// Run from the repo root:
//   cargo run -p ail-ui-bridge --example dump_wallet_graph -- ide/static/wallet_graph.json

use ail_ui_bridge::pipeline::{load_verified_from_path, read_project_name};
use ail_ui_bridge::serialize::serialize_graph;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project_root = manifest_dir
        .join("..")
        .join("..")
        .join("examples")
        .join("wallet_service");
    let src = project_root.join("src");

    let verified = load_verified_from_path(&src).expect("wallet pipeline must succeed");
    let name = read_project_name(&project_root);
    let graph = serialize_graph(&verified, &name);

    let json = serde_json::to_string_pretty(&graph).expect("serialize");

    let args: Vec<String> = std::env::args().collect();
    if let Some(out) = args.get(1) {
        std::fs::write(out, &json).expect("write output");
        eprintln!("wrote {} bytes to {}", json.len(), out);
    } else {
        println!("{}", json);
    }
}
