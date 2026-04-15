/// Controls how contract assertions are emitted into generated Python code.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ContractMode {
    /// Emit `assert <expr>  # <kind>: <raw>` statements (default — development).
    #[default]
    On,
    /// Emit `# assert <expr>  # <kind>: <raw>` comment lines only (staging).
    Comments,
    /// Omit all contract output entirely (production).
    Off,
}
