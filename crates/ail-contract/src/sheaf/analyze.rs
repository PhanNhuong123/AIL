//! Convenience entry point for sheaf analysis starting from a `TypedGraph`.
//!
//! This module is gated behind `#[cfg(feature = "z3-verify")]` because it
//! internally calls `detect_obstructions`, which requires Z3.
//!
//! The function exists exclusively to give `ail verify` a `&TypedGraph`-only
//! entry point. The caller clones the `TypedGraph` before `verify(typed)`
//! consumes it, then passes the clone here.

use ail_types::TypedGraph;

use crate::sheaf::{build_nerve, CechNerve};
use crate::types::VerifiedGraph;
use crate::z3_verify::{detect_obstructions, ObstructionResult};

/// Build the Čech nerve and detect H1 obstructions for a `TypedGraph`.
///
/// This is a best-effort, panic-free entry point. It wraps the typed graph in
/// a `VerifiedGraph` (skipping the contract-verification pass — the caller is
/// responsible for not relying on `VerifiedGraph`'s stage-gate semantics here),
/// then runs `build_nerve` followed by `detect_obstructions`.
///
/// Returns `(nerve, obstructions)`. Neither component returns `Result`;
/// per-overlap errors produce `ObstructionStatus::Unknown` entries instead of
/// propagating (invariant 17.2-H — zero panics).
pub fn analyze_sheaf_obstructions(typed: &TypedGraph) -> (CechNerve, Vec<ObstructionResult>) {
    // Clone the typed graph so the caller retains ownership of the original.
    let typed_clone = typed.clone();

    // Wrap into VerifiedGraph via the in-crate `pub(crate)` constructor. We do
    // NOT re-run contract verification: this helper is called by `ail verify`
    // *after* contract verification has FAILED, to provide localization
    // diagnostics over the rejected typed graph. `detect_obstructions` only
    // reads node-local data (params, contracts, graph topology) and never
    // relies on `VerifiedGraph`'s stage-gate semantics, so this construction
    // is sound for diagnostic-only use.
    let verified = VerifiedGraph::new(typed_clone);

    let nerve = build_nerve(&verified);
    let obstructions = detect_obstructions(&nerve, &verified);

    (nerve, obstructions)
}
