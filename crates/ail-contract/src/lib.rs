mod checks;
mod errors;

pub use checks::check_static_contracts;
pub use errors::ContractError;

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
