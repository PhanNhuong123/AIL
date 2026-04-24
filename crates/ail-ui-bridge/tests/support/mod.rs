#![allow(dead_code)]
//! Shared builders for integration tests.
//!
//! Included by each consuming test binary via:
//!
//! ```ignore
//! #[path = "support/mod.rs"]
//! mod support;
//! use support::*;
//! ```
//!
//! Each `tests/*.rs` file is a separate compilation unit, so every file that
//! needs these helpers must carry its own `mod support;` declaration.

use std::collections::BTreeMap;

use ail_ui_bridge::types::graph_json::{
    FunctionJson, GraphJson, ModuleJson, ProjectJson, StepJson,
};
use ail_ui_bridge::types::status::Status;

pub fn empty_graph() -> GraphJson {
    GraphJson {
        project: ProjectJson {
            id: "test".to_string(),
            name: "test".to_string(),
            description: String::new(),
            node_count: 0,
            module_count: 0,
            fn_count: 0,
            status: Status::Ok,
        },
        clusters: Vec::new(),
        modules: Vec::new(),
        externals: Vec::new(),
        relations: Vec::new(),
        types: Vec::new(),
        errors: Vec::new(),
        issues: Vec::new(),
        detail: BTreeMap::new(),
    }
}

pub fn make_module(id: &str, name: &str, description: &str, fns: Vec<FunctionJson>) -> ModuleJson {
    let fn_count = fns.len();
    ModuleJson {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        cluster: "default".to_string(),
        cluster_name: "test".to_string(),
        cluster_color: "#2997ff".to_string(),
        status: Status::Ok,
        node_count: fn_count + 1,
        functions: fns,
    }
}

pub fn make_fn(id: &str, name: &str, status: Status, steps: Option<Vec<StepJson>>) -> FunctionJson {
    FunctionJson {
        id: id.to_string(),
        name: name.to_string(),
        status,
        steps,
    }
}

pub fn make_step(id: &str, name: &str, intent: &str) -> StepJson {
    StepJson {
        id: id.to_string(),
        name: name.to_string(),
        status: Status::Ok,
        intent: intent.to_string(),
        branch: None,
    }
}

pub fn graph_with_modules(modules: Vec<ModuleJson>) -> GraphJson {
    let fn_count: usize = modules.iter().map(|m| m.functions.len()).sum();
    GraphJson {
        project: ProjectJson {
            id: "test".to_string(),
            name: "test".to_string(),
            description: String::new(),
            node_count: modules.len() + fn_count,
            module_count: modules.len(),
            fn_count,
            status: Status::Ok,
        },
        clusters: Vec::new(),
        modules,
        externals: Vec::new(),
        relations: Vec::new(),
        types: Vec::new(),
        errors: Vec::new(),
        issues: Vec::new(),
        detail: BTreeMap::new(),
    }
}
