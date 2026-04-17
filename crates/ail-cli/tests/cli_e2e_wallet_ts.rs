//! Phase 12 Task 12.2 — TypeScript end-to-end tests for `examples/wallet_service/`.
//!
//! Covers the full chain: `ail build --target typescript` on the example →
//! structural assertions on the emitted `dist-ts/` tree → `npx tsc --noEmit`
//! under strict settings → `npx vitest run` of the emitted test suite →
//! a synthetic contract-violation spec that proves the inlined `ail-runtime.ts`
//! plus `createT` factories throw on invalid data.
//!
//! Rust-only tests run everywhere. Node-gated tests shell out to
//! `node` / `npm` / `npx` and skip cleanly (via `eprintln!("[skip] …")` +
//! early return) when Node is unavailable, npm install fails, or the user
//! sets `AIL_SKIP_TS_NODE=1`. Matches the `cli_e2e_wallet_build_from_db_pytest_passes`
//! policy in `cli_e2e_wallet_sqlite.rs`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ail_cli::{run_build, BuildArgs};

// ── Shared helpers (mirror cli_e2e_wallet_sqlite.rs) ──────────────────────────

fn wallet_example_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("wallet_service")
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

fn fresh_example_project() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().to_path_buf();
    copy_dir_all(&wallet_example_dir(), &project).unwrap();
    (tmp, project)
}

fn ts_build_args<'a>() -> BuildArgs<'a> {
    BuildArgs {
        contracts: None,
        source_map: false,
        watch: false,
        check_breaking: false,
        check_migration: false,
        target: Some("typescript"),
        from_db: None,
    }
}

fn py_build_args<'a>() -> BuildArgs<'a> {
    BuildArgs {
        contracts: None,
        source_map: false,
        watch: false,
        check_breaking: false,
        check_migration: false,
        target: None,
        from_db: None,
    }
}

// ── Node-gated helpers ────────────────────────────────────────────────────────

// On Windows, `npm` / `npx` ship as both an extension-less shell script and a
// `.cmd` batch file. Rust's `Command::new` picks up the script, which is not a
// real Windows executable, so we must target the `.cmd` variant explicitly.
#[cfg(windows)]
const NPM_BIN: &str = "npm.cmd";
#[cfg(windows)]
const NPX_BIN: &str = "npx.cmd";
#[cfg(not(windows))]
const NPM_BIN: &str = "npm";
#[cfg(not(windows))]
const NPX_BIN: &str = "npx";

/// Returns `true` when `node --version` succeeds and `AIL_SKIP_TS_NODE` is
/// unset. Lets developers force-skip the shell-out path with
/// `AIL_SKIP_TS_NODE=1 cargo test ...`.
fn node_available() -> bool {
    if std::env::var_os("AIL_SKIP_TS_NODE").is_some() {
        return false;
    }
    matches!(
        Command::new("node").arg("--version").output(),
        Ok(out) if out.status.success()
    )
}

/// Runs `npm install --no-audit --no-fund` inside `dist_ts`. Returns `true`
/// when the install succeeds; callers that rely on `npx tsc` / `npx vitest`
/// must skip when this returns `false` (offline CI, private registry, etc.).
fn npm_install(dist_ts: &Path) -> bool {
    let status = Command::new(NPM_BIN)
        .args(["install", "--no-audit", "--no-fund"])
        .current_dir(dist_ts)
        .status();
    matches!(status, Ok(s) if s.success())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// 12.2-a: `ail build --target typescript` emits the expected `dist-ts/` tree
/// for `examples/wallet_service/`. Rust-only; no Node needed.
#[test]
fn cli_e2e_wallet_ts_build_emits_expected_layout() {
    let (_tmp, project) = fresh_example_project();
    run_build(&project, &ts_build_args()).expect("TypeScript build should succeed");

    let dist_ts = project.join("dist-ts");
    let must_exist = [
        "tsconfig.json",
        "package.json",
        "ail-runtime.ts",
        "types/wallet_balance.ts",
        "types/positive_amount.ts",
        "types/user.ts",
        "types/transfer_result.ts",
        "types/index.ts",
        "fn/add_money.ts",
        "fn/deduct_money.ts",
        "fn/transfer_money.ts",
        "fn/index.ts",
        "tests/add_money.test.ts",
        "tests/deduct_money.test.ts",
        "tests/transfer_money.test.ts",
    ];
    for rel in must_exist {
        assert!(
            dist_ts.join(rel).is_file(),
            "expected emitted file missing: {rel}"
        );
    }

    // wallet_service has no Error nodes, so `errors/` must not be created.
    assert!(
        !dist_ts.join("errors").exists(),
        "errors/ directory should not be emitted when no Error nodes exist"
    );
}

/// 12.2-b: Generated `tsconfig.json` pins strict mode. Rust-only.
#[test]
fn cli_e2e_wallet_ts_tsconfig_is_strict() {
    let (_tmp, project) = fresh_example_project();
    run_build(&project, &ts_build_args()).expect("TypeScript build should succeed");

    let tsconfig = fs::read_to_string(project.join("dist-ts/tsconfig.json"))
        .expect("tsconfig.json must exist after build");
    assert!(
        tsconfig.contains("\"strict\": true"),
        "tsconfig.json must pin `\"strict\": true`"
    );
    assert!(
        tsconfig.contains("\"noUncheckedIndexedAccess\": true"),
        "tsconfig.json must pin `\"noUncheckedIndexedAccess\": true`"
    );
}

/// 12.2-c: Inlined `ail-runtime.ts` exports the three runtime contract helpers.
/// Rust-only; proves the runtime surface is present without needing Node.
#[test]
fn cli_e2e_wallet_ts_runtime_exports_pre_post_keep() {
    let (_tmp, project) = fresh_example_project();
    run_build(&project, &ts_build_args()).expect("TypeScript build should succeed");

    let runtime = fs::read_to_string(project.join("dist-ts/ail-runtime.ts"))
        .expect("ail-runtime.ts must exist after build");
    assert!(
        runtime.contains("export function pre("),
        "ail-runtime.ts must export `pre`"
    );
    assert!(
        runtime.contains("export function post("),
        "ail-runtime.ts must export `post`"
    );
    assert!(
        runtime.contains("export function keep("),
        "ail-runtime.ts must export `keep`"
    );
}

/// 12.2-d: Structural parity between the Python and TypeScript emitters on
/// `wallet_service`. The generated Python `def <snake>(` set must equal the
/// TypeScript `fn/<snake>.ts` stem set. Rust-only.
#[test]
fn cli_e2e_wallet_ts_structural_parity_with_python() {
    let (_py_tmp, py_project) = fresh_example_project();
    run_build(&py_project, &py_build_args()).expect("Python build should succeed");

    let (_ts_tmp, ts_project) = fresh_example_project();
    run_build(&ts_project, &ts_build_args()).expect("TypeScript build should succeed");

    let py_defs = collect_py_def_names(&py_project.join("generated/functions.py"));
    let ts_stems = collect_ts_fn_stems(&ts_project.join("dist-ts/fn"));

    assert!(
        !py_defs.is_empty(),
        "Python emitter must produce at least one `def` line"
    );
    assert_eq!(
        py_defs, ts_stems,
        "Python def names and TypeScript fn file stems must match as a set"
    );
}

/// 12.2-e: `npx tsc --noEmit` on the generated `dist-ts/` exits 0.
/// Gated on `node_available()` + a successful `npm install`.
#[test]
fn cli_e2e_wallet_ts_tsc_noemit_passes() {
    if !node_available() {
        eprintln!("[skip] node not available (or AIL_SKIP_TS_NODE set)");
        return;
    }
    let (_tmp, project) = fresh_example_project();
    run_build(&project, &ts_build_args()).expect("TypeScript build should succeed");
    let dist_ts = project.join("dist-ts");

    if !npm_install(&dist_ts) {
        eprintln!("[skip] npm install failed in dist-ts");
        return;
    }

    let status = Command::new(NPX_BIN)
        .args(["tsc", "--noEmit"])
        .current_dir(&dist_ts)
        .status()
        .expect("npx tsc should launch");
    assert!(status.success(), "npx tsc --noEmit must exit 0");
}

/// 12.2-f: `npx vitest run` on the generated suite exits 0.
/// Gated on `node_available()` + a successful `npm install`.
#[test]
fn cli_e2e_wallet_ts_vitest_run_passes() {
    if !node_available() {
        eprintln!("[skip] node not available (or AIL_SKIP_TS_NODE set)");
        return;
    }
    let (_tmp, project) = fresh_example_project();
    run_build(&project, &ts_build_args()).expect("TypeScript build should succeed");
    let dist_ts = project.join("dist-ts");

    if !npm_install(&dist_ts) {
        eprintln!("[skip] npm install failed in dist-ts");
        return;
    }

    let status = Command::new(NPX_BIN)
        .args(["vitest", "run"])
        .current_dir(&dist_ts)
        .status()
        .expect("npx vitest should launch");
    assert!(status.success(), "npx vitest run must exit 0");
}

/// 12.2-g: The inlined `createPositiveAmount` factory throws on
/// non-positive input, proving the runtime contract layer actually fires.
/// Writes a synthetic spec into the hermetic temp copy — never touches the
/// committed example — and runs it with vitest.
#[test]
fn cli_e2e_wallet_ts_runtime_throws_on_invalid_amount() {
    if !node_available() {
        eprintln!("[skip] node not available (or AIL_SKIP_TS_NODE set)");
        return;
    }
    let (_tmp, project) = fresh_example_project();
    run_build(&project, &ts_build_args()).expect("TypeScript build should succeed");
    let dist_ts = project.join("dist-ts");

    if !npm_install(&dist_ts) {
        eprintln!("[skip] npm install failed in dist-ts");
        return;
    }

    let spec = "import { describe, it, expect } from 'vitest';\n\
        import { createPositiveAmount } from '../types/positive_amount';\n\
        \n\
        describe('runtime contract enforcement', () => {\n\
          it('rejects non-positive PositiveAmount', () => {\n\
            expect(() => createPositiveAmount(0)).toThrow();\n\
            expect(() => createPositiveAmount(-5)).toThrow();\n\
          });\n\
        });\n";
    fs::write(dist_ts.join("tests/contract_violation.test.ts"), spec)
        .expect("write contract violation spec");

    let status = Command::new(NPX_BIN)
        .args(["vitest", "run", "tests/contract_violation.test.ts"])
        .current_dir(&dist_ts)
        .status()
        .expect("npx vitest should launch");
    assert!(
        status.success(),
        "contract violation spec must pass (runtime check must fire)"
    );
}

// ── Parsing helpers ───────────────────────────────────────────────────────────

fn collect_py_def_names(functions_py: &Path) -> BTreeSet<String> {
    let text = fs::read_to_string(functions_py).expect("functions.py must exist");
    let mut out = BTreeSet::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("def ") {
            if let Some(paren_idx) = rest.find('(') {
                let name = rest[..paren_idx].trim();
                if !name.is_empty() {
                    out.insert(name.to_owned());
                }
            }
        }
    }
    out
}

fn collect_ts_fn_stems(fn_dir: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for entry in fs::read_dir(fn_dir).expect("fn/ dir must exist") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => continue,
        };
        if stem == "index" {
            continue;
        }
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if ext != "ts" {
            continue;
        }
        out.insert(stem.to_owned());
    }
    out
}
