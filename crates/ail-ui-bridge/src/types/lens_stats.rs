use serde::{Deserialize, Serialize};

/// The active lens for the IDE stage view.
///
/// Each variant corresponds to a different projection of the graph data. The
/// frontend uses the `Lens` value to select which panel to render and which
/// `LensStats` variant to display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lens {
    Structure,
    Rules,
    Verify,
    Data,
    Tests,
}

/// Per-lens metric bundle for a scope within the project graph.
///
/// Discriminated union tagged with `"lens"` so the frontend can `switch` on
/// `stats.lens` for type narrowing. Computed by `lens::compute_lens_metrics`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "lens", rename_all = "snake_case")]
pub enum LensStats {
    /// Structural counts: modules, functions, steps, and nodes in scope.
    Structure {
        modules: usize,
        functions: usize,
        steps: usize,
        nodes: usize,
    },
    /// Rule / contract counts within scope.
    Rules {
        total: usize,
        unproven: usize,
        broken: usize,
    },
    /// Formal verification counts within scope.
    Verify {
        proven: usize,
        unproven: usize,
        counterexamples: usize,
    },
    /// Type and signal (branch) counts within scope.
    Data { types: Vec<String>, signals: usize },
    /// Test counts within scope (Phase 15 placeholder — always zero).
    Tests {
        total: usize,
        passing: usize,
        failing: usize,
    },
}

impl LensStats {
    /// Return the zero value for the given lens.
    pub fn zero(lens: Lens) -> Self {
        match lens {
            Lens::Structure => LensStats::Structure {
                modules: 0,
                functions: 0,
                steps: 0,
                nodes: 0,
            },
            Lens::Rules => LensStats::Rules {
                total: 0,
                unproven: 0,
                broken: 0,
            },
            Lens::Verify => LensStats::Verify {
                proven: 0,
                unproven: 0,
                counterexamples: 0,
            },
            Lens::Data => LensStats::Data {
                types: Vec::new(),
                signals: 0,
            },
            Lens::Tests => LensStats::Tests {
                total: 0,
                passing: 0,
                failing: 0,
            },
        }
    }
}
