//! Behaviour tests for `ail agent` — `run_agent` function.
//!
//! # Approach
//!
//! These tests use **Approach B**: a temporary directory with a fake `python`
//! script is prepended to `PATH` so that `run_agent` invokes the fake binary
//! instead of any real Python.
//!
//! ## Windows limitation
//!
//! On Windows, `std::process::Command` resolves executables by trying each
//! extension in `PATHEXT` (`.COM`, `.EXE`, `.BAT`, `.CMD`) for every directory
//! in `PATH`.  Because `.EXE` takes precedence over `.CMD`/`.BAT`, a real
//! `python.exe` installed anywhere in `PATH` (including Microsoft Store's
//! `%LOCALAPPDATA%\Microsoft\WindowsApps\`) wins over a fake `python.cmd` in a
//! prepended temp directory.  Tests 1–3 that rely on fake-binary exit-code
//! interception are therefore gated on `#[cfg(not(windows))]`.
//!
//! Test 4 (`AgentNotInstalled` when PATH contains no python) works on all
//! platforms because it replaces PATH with an empty-directory that contains
//! neither a `.exe` nor a script of any kind.
//!
//! # Isolation note
//!
//! `std::env::set_var` is process-global.  Rust's test harness runs tests in
//! parallel by default, so these tests use a `Mutex` to serialise PATH
//! mutation and restore the original value on exit.

use std::sync::Mutex;

use ail_cli::{run_agent, AgentArgs, CliError};
use tempfile::TempDir;

// Serialise all PATH-mutating tests so they do not interfere with each other.
static PATH_LOCK: Mutex<()> = Mutex::new(());

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Build a minimal [`AgentArgs`] for testing.
fn minimal_args(task: &str) -> AgentArgs {
    AgentArgs {
        task: task.to_string(),
        model: None,
        mcp_port: 7777,
        max_iterations: None,
        steps_per_plan: None,
    }
}

/// Return a temporary directory that is safe to use as `cwd` for `run_agent`.
fn temp_cwd() -> (TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().to_path_buf();
    (tmp, path)
}

/// Restore PATH to `old`.
fn restore_path(old: &str) {
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("PATH", old);
    }
}

// ─── Unix-only fake-binary tests (tests 1–3) ─────────────────────────────────
//
// On Unix, writing a shell script named `python` and `python3` with `chmod
// +x` into a temp dir that is prepended to PATH reliably intercepts the probe
// and the real invocation, because the kernel resolves the shebang without
// PATHEXT complications.

#[cfg(not(windows))]
mod unix_subprocess_tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use ail_cli::{run_agent, AgentArgs, CliError};
    use tempfile::TempDir;

    use super::{minimal_args, restore_path, temp_cwd, PATH_LOCK};

    /// Write a fake `python` (and `python3`) shell script that exits with
    /// `exit_code` into `dir` and marks both executable.
    fn write_fake_python(dir: &TempDir, exit_code: i32) {
        let script = format!("#!/bin/sh\nexit {exit_code}\n");
        for name in ["python", "python3"] {
            let p = dir.path().join(name);
            fs::write(&p, &script).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    /// Prepend `dir` to PATH and return the old PATH.
    fn prepend_path(dir: &TempDir) -> String {
        let old = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("{}:{old}", dir.path().display());
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("PATH", &new_path);
        }
        old
    }

    /// Fake `python` that exits 0 → `run_agent` returns `Ok(())`.
    #[test]
    fn agent_returns_ok_when_python_exits_zero() {
        let _lock = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let fake_dir = tempfile::tempdir().unwrap();
        write_fake_python(&fake_dir, 0);
        let old_path = prepend_path(&fake_dir);
        let (cwd_dir, cwd) = temp_cwd();

        let result = run_agent(&cwd, &minimal_args("Add validation"));

        restore_path(&old_path);
        drop(cwd_dir);

        assert!(result.is_ok(), "expected Ok(()), got: {result:?}");
    }

    /// Fake `python` that exits with code 7 → `AgentFailed { code: 7 }`.
    #[test]
    fn agent_returns_agent_failed_with_exit_code() {
        let _lock = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let fake_dir = tempfile::tempdir().unwrap();
        write_fake_python(&fake_dir, 7);
        let old_path = prepend_path(&fake_dir);
        let (cwd_dir, cwd) = temp_cwd();

        let result = run_agent(&cwd, &minimal_args("task"));

        restore_path(&old_path);
        drop(cwd_dir);

        match result {
            Err(CliError::AgentFailed { code, .. }) => {
                assert_eq!(code, 7, "exit code must be 7");
            }
            other => panic!("expected AgentFailed {{code: 7}}, got: {other:?}"),
        }
    }

    /// Fake `python` exits 2 (provider config error) → `AgentFailed { code: 2 }`.
    #[test]
    fn agent_returns_agent_failed_for_provider_config_error() {
        let _lock = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let fake_dir = tempfile::tempdir().unwrap();
        write_fake_python(&fake_dir, 2);
        let old_path = prepend_path(&fake_dir);
        let (cwd_dir, cwd) = temp_cwd();

        let result = run_agent(&cwd, &minimal_args("task"));

        restore_path(&old_path);
        drop(cwd_dir);

        match result {
            Err(CliError::AgentFailed { code, .. }) => {
                assert_eq!(code, 2, "exit code must be 2");
            }
            other => panic!("expected AgentFailed {{code: 2}}, got: {other:?}"),
        }
    }
}

// ─── Test 4: no python in PATH → AgentNotInstalled (all platforms) ───────────

/// When PATH points to a directory with no `python` or `python3` binary,
/// `run_agent` returns `Err(CliError::AgentNotInstalled)`.
///
/// This test works on all platforms: an empty temp dir contains no executable
/// named `python` (regardless of extension), so both probe attempts fail.
#[test]
fn agent_returns_agent_not_installed_when_python_missing() {
    let _lock = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // An empty temp dir — no python binary at all.
    let empty_dir = tempfile::tempdir().unwrap();
    let old_path = std::env::var("PATH").unwrap_or_default();

    // Replace PATH entirely with just the empty dir.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("PATH", empty_dir.path());
    }

    let (cwd_dir, cwd) = temp_cwd();
    let result = run_agent(&cwd, &minimal_args("task"));

    restore_path(&old_path);
    drop(cwd_dir);

    match result {
        Err(CliError::AgentNotInstalled) => {}
        other => panic!("expected AgentNotInstalled, got: {other:?}"),
    }
}
