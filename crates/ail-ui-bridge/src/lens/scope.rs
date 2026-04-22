//! Scope resolution for lens metrics.

use crate::types::graph_json::{FunctionJson, GraphJson, ModuleJson, StepJson};

/// Scope of the metric computation.
pub(super) enum Scope<'a> {
    Project,
    Module(&'a ModuleJson),
    Function {
        #[allow(dead_code)]
        module: &'a ModuleJson,
        function: &'a FunctionJson,
    },
    Step {
        #[allow(dead_code)]
        module: &'a ModuleJson,
        #[allow(dead_code)]
        function: &'a FunctionJson,
        step: &'a StepJson,
    },
}

/// Resolve a scope id to the corresponding [`Scope`].
///
/// Returns `None` if the id is not found in the graph.
pub(super) fn resolve_scope<'a>(graph: &'a GraphJson, id: &str) -> Option<Scope<'a>> {
    for module in &graph.modules {
        if module.id == id {
            return Some(Scope::Module(module));
        }
        for function in &module.functions {
            if function.id == id {
                return Some(Scope::Function { module, function });
            }
            if let Some(steps) = &function.steps {
                for step in steps {
                    if step.id == id {
                        return Some(Scope::Step {
                            module,
                            function,
                            step,
                        });
                    }
                }
            }
        }
    }
    None
}
