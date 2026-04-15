//! `ail run` — not yet implemented in v0.1.
//!
//! The entry-point convention (which `do` node becomes `__main__`) is planned
//! for v0.2. Use `ail build` and invoke the generated code directly until then.

use std::path::Path;

use crate::error::CliError;

/// Entry point for `ail run`.
pub fn run_run(_root: &Path) -> Result<(), CliError> {
    Err(CliError::NotImplemented { feature: "ail run" })
}
