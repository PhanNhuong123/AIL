//! `ail-contract` — static contract checks, Z3 formal verification, and `VerifiedGraph`.
//!
//! This crate is the third stage of the AIL compiler pipeline. It owns:
//! - Static scope checks over `TypedGraph` ([`check_static_contracts`]).
//! - Z3 encoding and satisfiability checks (behind the `z3-verify` feature).
//! - `VerifiedGraph`, the third hard pipeline gate.
//!
//! ## Pipeline position
//!
//! ```text
//! TypedGraph → verify() → VerifiedGraph → (ail-emit)
//! ```
//!
//! ## Entry points
//!
//! - [`verify`] — run static checks (and Z3 if the feature is enabled); returns [`VerifiedGraph`].
//! - [`check_static_contracts`] — static-only pass, no Z3 dependency.
//!
//! ## Feature flags
//!
//! - `z3-verify` — enables Z3 encoding (`z3_encode` module) and formal verification (`z3_verify` module).
//!   Requires a Z3 system library and LLVM `libclang` on Windows.

mod checks;
mod errors;
mod types;
mod verify;

#[cfg(feature = "z3-verify")]
pub mod z3_encode;
#[cfg(feature = "z3-verify")]
pub mod z3_verify;

pub use checks::check_static_contracts;
pub use errors::{ContractError, ContractStageError};
pub use types::{BreakingChange, ContractRecord, ContractSummary, VerifiedGraph};
pub use verify::verify;

#[cfg(feature = "z3-verify")]
pub use errors::{EncodeError, VerifyError};
#[cfg(feature = "z3-verify")]
pub use z3_verify::verify_contracts;

#[cfg(test)]
#[cfg(feature = "z3-verify")]
mod z3_smoke_tests {
    use z3::{ast::Int, Config, Context, SatResult, Solver};

    #[test]
    fn z3_smoke_test_links_and_creates_context() {
        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let solver = Solver::new(&ctx);
        let x = Int::new_const(&ctx, "x");
        let zero = Int::from_i64(&ctx, 0);
        solver.assert(&x.gt(&zero));
        assert_eq!(solver.check(), SatResult::Sat);
    }

    #[test]
    fn z3_smoke_test_detects_unsat() {
        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let solver = Solver::new(&ctx);
        let x = Int::new_const(&ctx, "x");
        let zero = Int::from_i64(&ctx, 0);
        solver.assert(&x.gt(&zero));
        solver.assert(&x.lt(&zero));
        assert_eq!(solver.check(), SatResult::Unsat);
    }
}
