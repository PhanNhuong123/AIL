//! `ail test` — build the project and run generated pytest contract tests.

use std::path::Path;
use std::process::Command;

use crate::commands::build::BuildArgs;
use crate::error::CliError;

/// Entry point for `ail test`.
pub fn run_test(root: &Path) -> Result<(), CliError> {
    // Build with default settings (contracts on, no watch).
    let args = BuildArgs {
        contracts: None,
        source_map: false,
        watch: false,
        check_breaking: false,
        check_migration: false,
        target: None,
    };
    crate::commands::build::run_build(root, &args)?;

    // Check that the test file was generated.
    let test_file = root.join("generated").join("test_contracts.py");
    if !test_file.exists() {
        println!("No contract tests to run.");
        return Ok(());
    }

    // Verify pytest is available before trying to spawn it.
    let check = Command::new("python")
        .args(["-m", "pytest", "--version"])
        .output();

    match check {
        Ok(out) if out.status.success() => {}
        _ => {
            return Err(CliError::MissingTool {
                message: "pytest not found".to_owned(),
                hint: "Install it with: pip install pytest".to_owned(),
            });
        }
    }

    // Spawn pytest and forward its exit code.
    let status = Command::new("python")
        .args(["-m", "pytest", "generated/test_contracts.py", "-v"])
        .current_dir(root)
        .status()?;

    if !status.success() {
        return Err(CliError::Pipeline {
            errors: format!("pytest exited with status {}", status.code().unwrap_or(-1)),
        });
    }

    Ok(())
}
