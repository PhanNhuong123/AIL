//! Pipeline entry point for contract verification (Phase 3 Task 3.4).
//!
//! [`verify`] is the single public function that combines static contract scope
//! checks (Phase 3.1) and optional Z3 verification (Phase 3.3) into a single
//! call that produces the [`VerifiedGraph`] stage gate.
//!
//! # Verification order
//!
//! 1. **Static checks** (always run, no feature flag required): scope checks
//!    from Phase 3.1 (`check_static_contracts`). If any static error is found,
//!    the function returns immediately. Z3 is **not** invoked — fail fast.
//! 2. **Z3 verification** (only with `z3-verify` feature): full satisfiability
//!    and postcondition entailment checks from Phase 3.3 (`verify_contracts`).
//!
//! # Ownership on error
//!
//! `verify` consumes the `TypedGraph` regardless of the outcome, consistent
//! with `validate_graph` and `type_check`. When the function returns `Err`,
//! the caller must re-run `type_check` to obtain a fresh `TypedGraph`.

use ail_types::TypedGraph;

use crate::checks::check_static_contracts;
use crate::errors::ContractStageError;
use crate::types::VerifiedGraph;

#[cfg(feature = "z3-verify")]
use crate::z3_verify::verify_contracts;

/// Run contract verification over a [`TypedGraph`] and produce a
/// [`VerifiedGraph`].
///
/// Verification runs in two phases:
/// 1. **Static scope checks** (always): reference scope, `old()` placement,
///    raise error validity, and template phase coverage.
/// 2. **Z3 verification** (`z3-verify` feature): type-constraint
///    satisfiability, contradiction detection, and postcondition entailment.
///
/// Static errors short-circuit before Z3: if any static error is found the
/// function returns `Err` immediately without invoking the solver.
///
/// # Errors
///
/// Returns `Err(errors)` if any check fails. `errors` contains at least one
/// [`ContractStageError`]. On error the `TypedGraph` is consumed — the caller
/// must re-run `type_check` to obtain a fresh one.
///
/// # Example
///
/// ```no_run
/// # use ail_graph::validate_graph;
/// # use ail_types::type_check;
/// # use ail_contract::verify;
/// # let graph = ail_graph::AilGraph::new();
/// let valid = validate_graph(graph).expect("validation failed");
/// let typed = type_check(valid, &[]).expect("type check failed");
/// let verified = verify(typed).expect("contract verification failed");
/// println!("{:?}", verified);
/// ```
pub fn verify(typed: TypedGraph) -> Result<VerifiedGraph, Vec<ContractStageError>> {
    // ── Phase 1: static contract scope checks ────────────────────────────────
    let static_errors = check_static_contracts(&typed);
    if !static_errors.is_empty() {
        return Err(static_errors
            .into_iter()
            .map(ContractStageError::StaticCheck)
            .collect());
    }

    // ── Phase 2: Z3 verification (feature-gated) ─────────────────────────────
    #[cfg(feature = "z3-verify")]
    {
        let z3_errors = verify_contracts(&typed);
        if !z3_errors.is_empty() {
            return Err(z3_errors
                .into_iter()
                .map(ContractStageError::Z3Verify)
                .collect());
        }
    }

    Ok(VerifiedGraph::new(typed))
}
