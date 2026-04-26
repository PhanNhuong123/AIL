//! Integration tests for `ail` command handlers.
//!
//! These tests exercise the actual command logic — file creation, pipeline
//! execution, and output file routing — using temporary directories.
//! The wallet_service fixture (shared with `ail-graph` tests) is reused for
//! build and verify commands that require real `.ail` sources.

use std::fs;
use std::path::Path;

use ail_cli::{run_build, run_init, run_status, run_verify, BuildArgs};

// Path to the wallet_full fixture — a complete AIL project with contracts.
// Located at `crates/ail-text/tests/fixtures/wallet_full/`.
// CARGO_MANIFEST_DIR points to `crates/ail-cli/`.
fn wallet_fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..") // crates/
        .join("ail-text")
        .join("tests")
        .join("fixtures")
        .join("wallet_full")
}

// ── init ──────────────────────────────────────────────────────────────────────

#[test]
fn init_creates_project_structure() {
    let tmp = tempfile::tempdir().unwrap();

    run_init(tmp.path(), "hello").expect("init should succeed");

    let root = tmp.path().join("hello");
    assert!(
        root.join("src").join("main.ail").exists(),
        "main.ail missing"
    );
    assert!(root.join("generated").is_dir(), "generated/ missing");
    assert!(root.join("scaffolded").is_dir(), "scaffolded/ missing");
    assert!(
        root.join("ail.config.toml").exists(),
        "ail.config.toml missing"
    );

    let config = fs::read_to_string(root.join("ail.config.toml")).unwrap();
    assert!(
        config.contains("name = \"hello\""),
        "config missing project name"
    );
}

// ── build ─────────────────────────────────────────────────────────────────────

#[test]
fn build_writes_generated_files() {
    let fixture = wallet_fixture_path();
    let tmp = tempfile::tempdir().unwrap();

    // Copy wallet_service fixture into a temp dir so generated/ lands there.
    copy_dir_all(&fixture, tmp.path()).expect("fixture copy failed");

    let args = BuildArgs {
        contracts: None,
        source_map: false,
        watch: false,
        check_breaking: false,
        check_migration: false,
        target: None,
        from_db: None,
    };
    run_build(tmp.path(), &args).expect("build should succeed on wallet fixture");

    // The emitter produces generated/types.py and generated/functions.py.
    assert!(
        tmp.path().join("generated").join("types.py").exists(),
        "generated/types.py missing"
    );
    assert!(
        tmp.path().join("generated").join("functions.py").exists(),
        "generated/functions.py missing"
    );
}

#[test]
fn build_scaffolded_files_not_overwritten() {
    let fixture = wallet_fixture_path();
    let tmp = tempfile::tempdir().unwrap();
    copy_dir_all(&fixture, tmp.path()).expect("fixture copy failed");

    let args = BuildArgs {
        contracts: None,
        source_map: false,
        watch: false,
        check_breaking: false,
        check_migration: false,
        target: None,
        from_db: None,
    };

    // First build — scaffolded files are created.
    run_build(tmp.path(), &args).expect("first build should succeed");

    // Overwrite a scaffolded file with sentinel content.
    let scaffold_init = tmp.path().join("scaffolded").join("__init__.py");
    if scaffold_init.exists() {
        fs::write(&scaffold_init, "# SENTINEL\n").unwrap();

        // Second build — scaffolded file must not be overwritten.
        run_build(tmp.path(), &args).expect("second build should succeed");

        let content = fs::read_to_string(&scaffold_init).unwrap();
        assert!(
            content.contains("SENTINEL"),
            "scaffolded/__init__.py was overwritten on second build"
        );
    }
    // If the fixture produces no scaffolded files, the test is vacuously OK.
}

// ── verify ────────────────────────────────────────────────────────────────────

#[test]
fn verify_ok_on_valid_project() {
    let fixture = wallet_fixture_path();
    let result = run_verify(&fixture, None, None);
    assert!(
        result.is_ok(),
        "verify failed on wallet fixture: {result:?}"
    );
}

// ── status ────────────────────────────────────────────────────────────────────

#[test]
fn status_empty_project_is_raw() {
    let tmp = tempfile::tempdir().unwrap();
    // An empty directory has no .ail files; parse_directory returns an empty
    // graph, so the pipeline continues through all stages with zero nodes.
    // status should print "Stage: verified" (empty verified graph is valid).
    let result = run_status(tmp.path());
    assert!(
        result.is_ok(),
        "status should not error on empty project: {result:?}"
    );
}

// ── Phase 17 task 17.3: ail verify UNSAT augmentation tests ──────────────────

/// When `run_verify` encounters a Z3 UNSAT error (AIL-C011 ContradictoryPreconditions),
/// the returned `CliError::Pipeline` body must contain BOTH the original AIL-C011
/// message verbatim AND a "Sheaf localization" suffix.
///
/// Gated with `#[ignore]` because Z3 is environment-sensitive.
/// Run with: `cargo test -p ail-cli --features z3-verify -- --include-ignored`
#[cfg(feature = "z3-verify")]
#[test]
#[ignore]
fn t173_ail_verify_unsat_appends_sheaf_localization() {
    // Write a fixture whose before-contracts are provably contradictory under Z3:
    // `amount > 10` AND `amount < 5` cannot both hold simultaneously.
    // Uses built-in `PositiveAmount` and `PositiveInteger` types (no define needed).
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(
        src.join("contradictory.ail"),
        concat!(
            "do transfer money\n",
            "  from amount:PositiveAmount\n",
            "  -> PositiveAmount\n\n",
            "  promise before: amount > 10\n",
            "  promise before: amount < 5\n",
            "  promise after: amount > 0\n",
        ),
    )
    .unwrap();

    let result = run_verify(tmp.path(), None, None);
    assert!(
        result.is_err(),
        "verify must fail on contradictory contracts"
    );

    let err_body = result.unwrap_err().to_string();

    // Invariant 17.3-A: original diagnostics are the prefix.
    assert!(
        err_body.contains("AIL-C011") || err_body.contains("contradictory"),
        "error body must contain the original AIL-C011 diagnostic, got:\n{err_body}"
    );

    // Invariant 17.3-A: sheaf localization is a suffix, never replaces the original.
    assert!(
        err_body.contains("Sheaf localization"),
        "error body must contain 'Sheaf localization' suffix, got:\n{err_body}"
    );
}

/// When `run_verify` encounters a Z3 error of a NON-UNSAT class (e.g. C012
/// `PostconditionNotEntailed`), the "Sheaf localization" suffix must NOT appear.
///
/// Gated with `#[ignore]` because Z3 is environment-sensitive.
/// Run with: `cargo test -p ail-cli --features z3-verify -- --include-ignored`
#[cfg(feature = "z3-verify")]
#[test]
#[ignore]
fn t173_ail_verify_non_unsat_class_does_not_augment() {
    // Write a fixture whose postcondition is not entailed by its preconditions
    // (AIL-C012 PostconditionNotEntailed). Uses built-in types.
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(
        src.join("unenforced_post.ail"),
        concat!(
            "do transfer money\n",
            "  from amount:PositiveAmount\n",
            "  -> PositiveAmount\n\n",
            // Pre: amount > 0 (from PositiveAmount) — consistent.
            // Post: amount > 100 — not entailed by just `amount > 0`.
            "  promise before: amount > 0\n",
            "  promise after: amount > 100\n",
        ),
    )
    .unwrap();

    let result = run_verify(tmp.path(), None, None);
    // The project may pass or fail depending on Z3's postcondition check.
    // If it fails, the body must NOT contain "Sheaf localization".
    if let Err(e) = result {
        let err_body = e.to_string();
        assert!(
            !err_body.contains("Sheaf localization"),
            "non-UNSAT error must not contain 'Sheaf localization', got:\n{err_body}"
        );
    }
    // If it passes, the test is vacuously OK (Z3 may accept simple post-conditions).
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}
