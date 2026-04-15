//! End-to-end pipeline tests: `.ail` source files → full Rust pipeline → generated Python.
//!
//! Each test copies the `wallet_full` fixture into a temporary directory, runs
//! `run_build()`, then inspects the files written to disk.  This verifies the
//! complete path from human-readable AIL text through parse, validate, type-check,
//! verify, and emit stages.
//!
//! ## wallet_full fixture (7 files)
//! Located at `crates/ail-text/tests/fixtures/wallet_full/`:
//! - `positive_amount.ail`  — `define PositiveAmount:number where value > 0`
//! - `wallet_balance.ail`   — `define WalletBalance:number where value >= 0`
//! - `user.ail`             — `describe User as balance:WalletBalance`
//! - `transfer_result.ail`  — `describe TransferResult as sender:User, ...`
//! - `transfer_money.ail`   — `do transfer money` with 3 contracts + 1 Let child
//! - `deduct_money.ail`     — `do deduct money` with 3 contracts + 1 Let child
//! - `add_money.ail`        — `do add money` with 2 contracts + 1 Let child
//!
//! ## Generated output layout
//! ```
//! generated/
//!   types.py                — Python type classes from Define/Describe/Error nodes
//!   functions.py            — Python function definitions from Do nodes
//!   test_contracts.py       — pytest stubs for contract documentation
//!   functions.ailmap.json   — function-level source map
//!   __init__.py             — package marker
//! scaffolded/
//!   __init__.py             — developer-owned scaffold (write-once)
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use ail_cli::{run_build, BuildArgs};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn wallet_full_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..") // crates/
        .join("ail-text")
        .join("tests")
        .join("fixtures")
        .join("wallet_full")
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

fn default_args() -> BuildArgs<'static> {
    BuildArgs {
        contracts: None,
        source_map: false,
        watch: false,
        check_breaking: false,
        check_migration: false,
    }
}

// ── File-existence tests ──────────────────────────────────────────────────────

/// `generated/types.py` is produced and contains at least one AIL type name.
#[test]
fn e2e_wallet_build_generates_types_py() {
    let tmp = tempfile::tempdir().unwrap();
    copy_dir_all(&wallet_full_dir(), tmp.path()).unwrap();

    run_build(tmp.path(), &default_args()).expect("build should succeed on wallet_full");

    let path = tmp.path().join("generated").join("types.py");
    assert!(path.exists(), "generated/types.py missing after build");

    let content = fs::read_to_string(&path).unwrap();
    assert!(
        content.contains("WalletBalance") || content.contains("PositiveAmount"),
        "types.py should contain at least one generated type; got:\n{content}"
    );
}

/// `generated/functions.py` is produced and contains the `transfer_money` function
/// with contract assertions injected.
#[test]
fn e2e_wallet_build_generates_functions_py() {
    let tmp = tempfile::tempdir().unwrap();
    copy_dir_all(&wallet_full_dir(), tmp.path()).unwrap();

    run_build(tmp.path(), &default_args()).expect("build should succeed");

    let path = tmp.path().join("generated").join("functions.py");
    assert!(path.exists(), "generated/functions.py missing after build");

    let content = fs::read_to_string(&path).unwrap();
    assert!(
        content.contains("transfer_money"),
        "functions.py should define transfer_money; got:\n{content}"
    );
    // ContractMode::On emits `assert {expr}  # {label}: {raw}`.
    // Match the specific before-contract expression from transfer_money.ail.
    assert!(
        content.contains("assert sender_balance >= amount"),
        "functions.py should contain before-contract assertion 'sender_balance >= amount'; got:\n{content}"
    );
    assert!(
        content.contains("# before:"),
        "functions.py should contain before-contract label '# before:'; got:\n{content}"
    );
    // The specific before-contract from transfer_money.ail.
    assert!(
        content.contains("sender_balance"),
        "expected 'sender_balance' parameter in functions.py"
    );
}

/// `generated/test_contracts.py` is produced with pytest stubs.
#[test]
fn e2e_wallet_build_generates_test_contracts_py() {
    let tmp = tempfile::tempdir().unwrap();
    copy_dir_all(&wallet_full_dir(), tmp.path()).unwrap();

    run_build(tmp.path(), &default_args()).expect("build should succeed");

    let path = tmp.path().join("generated").join("test_contracts.py");
    assert!(
        path.exists(),
        "generated/test_contracts.py missing after build"
    );

    let content = fs::read_to_string(&path).unwrap();
    assert!(
        content.contains("import pytest"),
        "test_contracts.py must import pytest"
    );
    // Class name ends in "Contracts" — e.g. `TransferMoneyContracts`.
    assert!(
        content.contains("Contracts"),
        "test_contracts.py must define a ...Contracts class"
    );
    assert!(
        content.contains("pytest.skip"),
        "test stubs must call pytest.skip"
    );
}

/// `scaffolded/__init__.py` is produced with the correct re-export imports.
#[test]
fn e2e_wallet_build_generates_scaffold_init() {
    let tmp = tempfile::tempdir().unwrap();
    copy_dir_all(&wallet_full_dir(), tmp.path()).unwrap();

    run_build(tmp.path(), &default_args()).expect("build should succeed");

    let path = tmp.path().join("scaffolded").join("__init__.py");
    assert!(
        path.exists(),
        "scaffolded/__init__.py missing after build"
    );

    let content = fs::read_to_string(&path).unwrap();
    assert!(
        content.contains("from generated.types import"),
        "scaffold init must re-export generated types"
    );
    assert!(
        content.contains("from generated.functions import"),
        "scaffold init must re-export generated functions"
    );
}

/// `generated/functions.ailmap.json` is produced (function-level source map).
#[test]
fn e2e_wallet_build_generates_source_map() {
    let tmp = tempfile::tempdir().unwrap();
    copy_dir_all(&wallet_full_dir(), tmp.path()).unwrap();

    run_build(tmp.path(), &default_args()).expect("build should succeed");

    let path = tmp
        .path()
        .join("generated")
        .join("functions.ailmap.json");
    assert!(
        path.exists(),
        "generated/functions.ailmap.json missing after build"
    );

    // Source map must start with `{` (JSON object).
    let content = fs::read_to_string(&path).unwrap();
    assert!(
        content.trim_start().starts_with('{'),
        "functions.ailmap.json must be a JSON object; got: {content}"
    );
}

/// End-to-end: all expected output files are present after a single build.
#[test]
fn e2e_full_pipeline_all_outputs_present() {
    let tmp = tempfile::tempdir().unwrap();
    copy_dir_all(&wallet_full_dir(), tmp.path()).unwrap();

    run_build(tmp.path(), &default_args()).expect("build should succeed on wallet_full");

    let generated = tmp.path().join("generated");
    assert!(generated.is_dir(), "generated/ directory must exist");
    assert!(
        generated.join("types.py").exists(),
        "generated/types.py missing"
    );
    assert!(
        generated.join("functions.py").exists(),
        "generated/functions.py missing"
    );
    assert!(
        generated.join("test_contracts.py").exists(),
        "generated/test_contracts.py missing"
    );
    assert!(
        generated.join("functions.ailmap.json").exists(),
        "generated/functions.ailmap.json missing"
    );
    assert!(
        generated.join("__init__.py").exists(),
        "generated/__init__.py missing"
    );
    assert!(
        tmp.path().join("scaffolded").join("__init__.py").exists(),
        "scaffolded/__init__.py missing"
    );
}

/// `generated/functions.py` contains `deduct_money` with its before-contract injected.
#[test]
fn e2e_wallet_build_generates_deduct_money() {
    let tmp = tempfile::tempdir().unwrap();
    copy_dir_all(&wallet_full_dir(), tmp.path()).unwrap();

    run_build(tmp.path(), &default_args()).expect("build should succeed");

    let path = tmp.path().join("generated").join("functions.py");
    assert!(path.exists(), "generated/functions.py missing after build");

    let content = fs::read_to_string(&path).unwrap();
    assert!(
        content.contains("deduct_money"),
        "functions.py should define deduct_money; got:\n{content}"
    );
    assert!(
        content.contains("assert balance >= amount"),
        "functions.py should contain before-contract 'balance >= amount'; got:\n{content}"
    );
}

/// `generated/functions.py` contains `add_money` with its after-contract injected.
#[test]
fn e2e_wallet_build_generates_add_money() {
    let tmp = tempfile::tempdir().unwrap();
    copy_dir_all(&wallet_full_dir(), tmp.path()).unwrap();

    run_build(tmp.path(), &default_args()).expect("build should succeed");

    let path = tmp.path().join("generated").join("functions.py");
    assert!(path.exists(), "generated/functions.py missing after build");

    let content = fs::read_to_string(&path).unwrap();
    assert!(
        content.contains("add_money"),
        "functions.py should define add_money; got:\n{content}"
    );
    assert!(
        content.contains("assert amount > 0"),
        "functions.py should contain before-contract 'amount > 0' for add_money; got:\n{content}"
    );
}

// ── Optional pytest test ──────────────────────────────────────────────────────

/// Run pytest on the generated contract stubs.
///
/// Skipped automatically when `python` or `pytest` is not available in the
/// environment — this keeps CI clean on systems without a Python installation.
///
/// Generated classes are named `TestXContracts` so pytest collects them.
/// All stubs call `pytest.skip()`, so pytest exits 0 (all skipped = pass).
#[test]
fn e2e_pytest_passes_on_generated_code() {
    // Skip if Python / pytest is unavailable.
    let available = std::process::Command::new("python")
        .args(["-m", "pytest", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !available {
        eprintln!("e2e_pytest_passes_on_generated_code: skipped (python/pytest not found)");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    copy_dir_all(&wallet_full_dir(), tmp.path()).unwrap();
    run_build(tmp.path(), &default_args()).expect("build should succeed");

    let test_py = tmp.path().join("generated").join("test_contracts.py");
    if !test_py.exists() {
        return; // no test file generated — skip silently
    }

    let output = std::process::Command::new("python")
        .args(["-m", "pytest", "-v", test_py.to_str().unwrap()])
        .current_dir(tmp.path())
        .env("PYTHONPATH", tmp.path())
        .output()
        .expect("pytest should run");

    let exit_code = output.status.code().unwrap_or(-1);
    assert!(
        output.status.success(),
        "pytest should exit 0 (all tests skipped); got exit {exit_code}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
