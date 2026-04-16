/// Controls how contract assertions are emitted into generated code.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ContractMode {
    /// Emit live runtime checks: `assert expr` (Python) / `pre(expr, "raw")` (TypeScript).
    /// Default — for development and testing.
    #[default]
    On,
    /// Emit commented-out checks: `# assert expr` (Python) / `// PRE: raw` (TypeScript).
    Comments,
    /// Omit all contract output entirely (production).
    Off,
    /// Emit contracts only in generated test files, not in production code.
    /// Production fn files behave as `Off`; test-file generators use `On` semantics.
    Test,
}
