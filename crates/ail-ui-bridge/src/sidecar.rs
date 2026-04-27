//! Sidecar path resolution and health-check commands — Phase 16.5.
//!
//! This module exposes:
//!
//! - Pure helpers (`seed_sidecar_nonce`, `next_sidecar_run_id_string`,
//!   `parse_ail_dev_mode`, `parse_version_line`) — no I/O beyond `env::var`,
//!   no locks (invariant 16.5-A).
//! - Dev-mode path resolver (`resolve_core_binary_path_dev`) — walks
//!   `target/debug` then `target/release`; used only when `AIL_DEV=1`.
//! - Bundle-mode core resolution via `tauri-plugin-shell` `ShellExt::sidecar`
//!   which auto-handles the target-triple suffix (e.g.
//!   `ail-x86_64-pc-windows-msvc.exe`) — the old `resolve_core_binary_path`
//!   using `app.path().resolve("binaries/ail", …)` was incorrect because
//!   Tauri 2 `externalBin` bundles binaries WITH the triple suffix (C1 fix).
//! - `resolve_agent_wrapper_path` — bundle-mode wrapper script resolver
//!   (unchanged; wrapper scripts are NOT Tauri sidecars so they keep flat names).
//! - `health_check_core` and `health_check_agent` Tauri commands — two-phase
//!   lock (seq reserved → lock released → spawn) so `BridgeState` is never
//!   held across subprocess I/O (invariant 16.5-C).
//!
//! # Dev mode
//!
//! When `AIL_DEV` equals `"1"` (and only `"1"` — invariant 16.5-H):
//! - `resolve_core_binary_path_dev` walks `target/debug` then `target/release`.
//! - `resolve_agent_wrapper_path` returns `Err` (caller uses
//!   `python -m ail_agent` directly via `spawn_python_agent_version`).
//! - Both health commands adapt accordingly.

#![cfg(feature = "tauri-commands")]

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_shell::ShellExt;
use tokio::process::Command;

use crate::commands::BridgeState;
use crate::errors::BridgeError;
use crate::types::sidecar_result::{HealthCheckPayload, SidecarMode};

// ---------------------------------------------------------------------------
// Pure helpers (invariant 16.5-A: no I/O beyond env::var, no locks, no spawn)
// ---------------------------------------------------------------------------

/// Seed a 64-bit nonce from `SystemTime::now_ns ^ pid`. Mirrors `agent::seed_nonce`.
pub fn seed_sidecar_nonce() -> u64 {
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    ns ^ (std::process::id() as u64)
}

/// Reserve the next sidecar health-check run id.
///
/// Format: `"sidecar-{seq_hex}-{nonce_hex}"`. Returns a hex-encoded string that
/// is clearly distinct from agent/verifier/sheaf run ids. Incrementing `seq`
/// before format ensures the first call returns `"sidecar-1-{nonce:x}"`.
pub fn next_sidecar_run_id_string(seq: &mut u64, nonce: u64) -> String {
    *seq = seq.wrapping_add(1);
    format!("sidecar-{:x}-{:x}", *seq, nonce)
}

/// Returns `true` iff the `AIL_DEV` environment variable equals exactly `"1"`.
///
/// `"true"`, `"yes"`, `"0"`, `""` all return `false` (invariant 16.5-H).
pub fn parse_ail_dev_mode() -> bool {
    std::env::var("AIL_DEV").as_deref() == Ok("1")
}

/// Parse the version string from `<name> <version>` clap auto-version output.
///
/// Returns `Some(version)` for exactly two whitespace-separated tokens.
/// Returns `None` for zero tokens, one token, or three+ tokens.
pub fn parse_version_line(stdout: &str) -> Option<String> {
    let line = stdout.lines().next()?;
    let mut parts = line.split_whitespace();
    let _name = parts.next()?;
    let ver = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some(ver.to_string())
}

// ---------------------------------------------------------------------------
// Path resolvers
// ---------------------------------------------------------------------------

/// Resolve the `ail-cli` binary path in **dev mode only** (`AIL_DEV=1`).
///
/// Walks `target/debug` then `target/release` relative to the workspace root
/// (found by walking ancestors for `Cargo.toml`).
///
/// In bundle mode, use `app.shell().sidecar("ail")` (via `ShellExt`) instead
/// — it auto-handles the target-triple suffix that Tauri 2 `externalBin`
/// appends (e.g. `ail-x86_64-pc-windows-msvc.exe`).
pub(crate) fn resolve_core_binary_path_dev() -> Result<PathBuf, BridgeError> {
    let workspace_root = std::env::current_dir()
        .ok()
        .and_then(|p| {
            p.ancestors()
                .find(|a| a.join("Cargo.toml").is_file())
                .map(Path::to_path_buf)
        })
        .ok_or_else(|| BridgeError::InvalidInput {
            reason: "AIL_DEV: workspace root not found".into(),
        })?;
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    for profile in ["debug", "release"] {
        let candidate = workspace_root
            .join("target")
            .join(profile)
            .join(format!("ail{suffix}"));
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(BridgeError::InvalidInput {
        reason: "AIL_DEV: target/debug/ail or target/release/ail not found — run `cargo build -p ail-cli`".into(),
    })
}

/// Resolve the `ail-agent` wrapper script path.
///
/// Dev mode (`AIL_DEV=1`): returns `Err` — the caller should use
/// `python -m ail_agent` directly (via `spawn_python_agent_version`).
/// Bundle mode: resolves `binaries/ail-agent.cmd` (Windows) or
/// `binaries/ail-agent.sh` (POSIX) via Tauri's `PathResolver`.
pub(crate) fn resolve_agent_wrapper_path<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<PathBuf, BridgeError> {
    if parse_ail_dev_mode() {
        return Err(BridgeError::InvalidInput {
            reason: "AIL_DEV: ail-agent uses python -m ail_agent directly (no wrapper)".into(),
        });
    }
    let suffix = if cfg!(windows) { ".cmd" } else { ".sh" };
    app.path()
        .resolve(
            format!("binaries/ail-agent{suffix}"),
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|e| BridgeError::InvalidInput {
            reason: format!("sidecar resolution failed: ail-agent: {e}"),
        })
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Health-check the `ail-cli` sidecar by invoking `ail --version`.
///
/// Dev mode: resolves binary via `resolve_core_binary_path_dev` and spawns
/// with `tokio::process::Command`.
/// Bundle mode: uses `app.shell().sidecar("ail")` from `tauri-plugin-shell`
/// which auto-resolves the target-triple suffix (C1 fix — the old
/// `app.path().resolve("binaries/ail", …)` looked for a literal path without
/// the triple and would never find the bundled binary).
///
/// Follows the two-phase lock pattern: (1) lock → reserve run_id → release,
/// (2) resolve path + spawn OFF-LOCK (invariant 16.5-C).
#[tauri::command]
pub async fn health_check_core<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, BridgeState>,
) -> Result<HealthCheckPayload, BridgeError> {
    let _run_id = {
        let mut inner = state.lock().map_err(|_| BridgeError::InvalidInput {
            reason: "state lock poisoned".to_string(),
        })?;
        let nonce = inner.sidecar_id_nonce;
        next_sidecar_run_id_string(&mut inner.sidecar_health_seq, nonce)
    };
    let dev = parse_ail_dev_mode();
    let mode = if dev {
        SidecarMode::Dev
    } else {
        SidecarMode::Bundled
    };
    if dev {
        match resolve_core_binary_path_dev() {
            Ok(path) => spawn_and_parse_version(&path, "ail-core", mode).await,
            Err(e) => Ok(HealthCheckPayload {
                component: "ail-core".into(),
                ok: false,
                mode,
                version: None,
                error: Some(e.to_string()),
            }),
        }
    } else {
        // Bundle mode: let tauri-plugin-shell resolve the triple-suffixed binary.
        spawn_and_parse_shell_sidecar(&app, "ail", "ail-core", mode).await
    }
}

/// Health-check the `ail-agent` sidecar by invoking `--version`.
///
/// Dev mode: runs `python -m ail_agent --version`.
/// Bundle mode: runs the wrapper script `--version`.
///
/// Follows the two-phase lock pattern (invariant 16.5-C).
#[tauri::command]
pub async fn health_check_agent<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, BridgeState>,
) -> Result<HealthCheckPayload, BridgeError> {
    let _run_id = {
        let mut inner = state.lock().map_err(|_| BridgeError::InvalidInput {
            reason: "state lock poisoned".to_string(),
        })?;
        let nonce = inner.sidecar_id_nonce;
        next_sidecar_run_id_string(&mut inner.sidecar_health_seq, nonce)
    };
    // Bind once — used for both the mode enum and the branch (I3 fix).
    let dev = parse_ail_dev_mode();
    let mode = if dev {
        SidecarMode::Dev
    } else {
        SidecarMode::Bundled
    };
    if dev {
        return spawn_python_agent_version("ail-agent", mode).await;
    }
    match resolve_agent_wrapper_path(&app) {
        Ok(path) => spawn_and_parse_version(&path, "ail-agent", mode).await,
        Err(e) => Ok(HealthCheckPayload {
            component: "ail-agent".into(),
            ok: false,
            mode,
            version: None,
            error: Some(e.to_string()),
        }),
    }
}

// ---------------------------------------------------------------------------
// Private spawn helpers
// ---------------------------------------------------------------------------

/// Bundle-mode: use `tauri-plugin-shell` `ShellExt::sidecar` to spawn
/// `<name> --version`. The shell plugin resolves the target-triple suffix
/// automatically, so `sidecar("ail")` finds `ail-x86_64-pc-windows-msvc.exe`
/// (or the equivalent on other platforms) inside the bundle.
async fn spawn_and_parse_shell_sidecar<R: Runtime>(
    app: &AppHandle<R>,
    sidecar_name: &str,
    component: &str,
    mode: SidecarMode,
) -> Result<HealthCheckPayload, BridgeError> {
    let sidecar_cmd = match app.shell().sidecar(sidecar_name) {
        Ok(cmd) => cmd,
        Err(e) => {
            return Ok(HealthCheckPayload {
                component: component.to_string(),
                ok: false,
                mode,
                version: None,
                error: Some(format!("sidecar resolution failed: {e}")),
            });
        }
    };
    match sidecar_cmd.arg("--version").output().await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            match parse_version_line(&stdout) {
                Some(ver) => Ok(HealthCheckPayload {
                    component: component.to_string(),
                    ok: true,
                    mode,
                    version: Some(ver),
                    error: None,
                }),
                None => Ok(HealthCheckPayload {
                    component: component.to_string(),
                    ok: false,
                    mode,
                    version: None,
                    error: Some(format!("unexpected --version output: {:?}", stdout.trim())),
                }),
            }
        }
        Err(e) => Ok(HealthCheckPayload {
            component: component.to_string(),
            ok: false,
            mode,
            version: None,
            error: Some(format!("spawn failed: {e}")),
        }),
    }
}

/// Spawn `<path> --version`, collect stdout, parse the version line.
async fn spawn_and_parse_version(
    path: &Path,
    component: &str,
    mode: SidecarMode,
) -> Result<HealthCheckPayload, BridgeError> {
    match Command::new(path).arg("--version").output().await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            match parse_version_line(&stdout) {
                Some(ver) => Ok(HealthCheckPayload {
                    component: component.to_string(),
                    ok: true,
                    mode,
                    version: Some(ver),
                    error: None,
                }),
                None => Ok(HealthCheckPayload {
                    component: component.to_string(),
                    ok: false,
                    mode,
                    version: None,
                    error: Some(format!("unexpected --version output: {:?}", stdout.trim())),
                }),
            }
        }
        Err(e) => Ok(HealthCheckPayload {
            component: component.to_string(),
            ok: false,
            mode,
            version: None,
            error: Some(format!("spawn failed: {e}")),
        }),
    }
}

/// Spawn `python[3] -m ail_agent --version`, collect stdout, parse the version.
async fn spawn_python_agent_version(
    component: &str,
    mode: SidecarMode,
) -> Result<HealthCheckPayload, BridgeError> {
    let py = if cfg!(windows) { "python" } else { "python3" };
    match Command::new(py)
        .args(["-m", "ail_agent", "--version"])
        .output()
        .await
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            match parse_version_line(&stdout) {
                Some(ver) => Ok(HealthCheckPayload {
                    component: component.to_string(),
                    ok: true,
                    mode,
                    version: Some(ver),
                    error: None,
                }),
                None => Ok(HealthCheckPayload {
                    component: component.to_string(),
                    ok: false,
                    mode,
                    version: None,
                    error: Some(format!("unexpected --version output: {:?}", stdout.trim())),
                }),
            }
        }
        Err(e) => Ok(HealthCheckPayload {
            component: component.to_string(),
            ok: false,
            mode,
            version: None,
            error: Some(format!("spawn failed: {e}")),
        }),
    }
}
