use std::env;
use std::io;
use std::path::PathBuf;

fn main() {
    if let Err(e) = materialize_sidecars() {
        // Windows ERROR_SHARING_VIOLATION (raw os error 32) fires when a
        // running `ail-ide.exe` (e.g. an open dev IDE) holds a lock on the
        // sidecar binary. The next build picks the file back up and the
        // missing copy is a no-op-style transient — emit it as a regular
        // log line instead of a `cargo:warning=` so the dev console stays
        // quiet (closes review finding **N5**).
        if is_sharing_violation(e.as_ref()) {
            println!(
                "cargo:rustc-env=AIL_SIDECAR_LOCK_NOTICE=1\ncargo:rerun-if-changed=build.rs"
            );
            println!(
                "ail-ide build.rs: sidecar copy skipped (file in use, retry on next build)"
            );
        } else {
            println!("cargo:warning=sidecar materialization skipped: {e}");
        }
    }
    tauri_build::build();
}

/// Detect the Windows ERROR_SHARING_VIOLATION (raw os error 32) which
/// surfaces during dev rebuilds while the previous IDE process still holds
/// the sidecar file open. Other I/O errors are real and should still warn.
///
/// Takes `&(dyn Error + 'static)` (not `&Box<dyn Error>`) so clippy's
/// `borrowed_box` lint stays clean. The `'static` bound is required by
/// `downcast_ref`. Callers pass `e.as_ref()` where `e: Box<dyn Error>`.
fn is_sharing_violation(err: &(dyn std::error::Error + 'static)) -> bool {
    err.downcast_ref::<io::Error>()
        .and_then(|e| e.raw_os_error())
        == Some(32)
}

/// Copy built sidecar binaries into `binaries/` before Tauri bundles.
///
/// This runs at build time. When source files are absent (clean checkout, CI
/// without prior build steps), the function is a no-op — `cargo check` stays
/// clean (invariant 16.5-F / 16.6-B).
///
/// `rerun-if-changed` directives are emitted OUTSIDE the `if src.exists()`
/// guards so cargo re-runs this script when source files are added or deleted
/// later — not just when they change (I1 fix).
///
/// Platform determination uses `env::var("TARGET")` (the cargo TARGET triple),
/// NOT `cfg!(windows)` — `cfg!` returns HOST, not TARGET, and would silently
/// mis-resolve under cross-compilation (invariant 16.6-E).
fn materialize_sidecars() -> Result<(), Box<dyn std::error::Error>> {
    let triple = env::var("TARGET")?;
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let binaries_dir = manifest_dir.join("binaries");
    std::fs::create_dir_all(&binaries_dir)?;

    // Navigate two levels up from ide/src-tauri to workspace root.
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .ok_or("cannot resolve workspace root from CARGO_MANIFEST_DIR")?;

    let exe_suffix = if triple.contains("windows") {
        ".exe"
    } else {
        ""
    };

    // Copy ail-cli binary (Tauri externalBin — triple-suffix naming).
    let src_ail = workspace_root
        .join("target")
        .join("release")
        .join(format!("ail{exe_suffix}"));
    let dst_ail = binaries_dir.join(format!("ail-{triple}{exe_suffix}"));
    // Emit rerun directive OUTSIDE the exists() check so cargo re-runs when
    // the source appears or disappears (I1 fix).
    println!("cargo:rerun-if-changed={}", src_ail.display());
    if src_ail.exists() {
        std::fs::copy(&src_ail, &dst_ail)?;
    }

    // Copy frozen ail-agent binary built by `agents/scripts/build_sidecar.{sh,ps1}`.
    // Source path is platform-specific: agents/dist/ail-agent[.exe].
    // Dest is the platform-correct flat name in binaries/ — Tauri bundles it
    // via the `bundle.resources` glob `"binaries/ail-agent*"`. Single file per
    // platform; no doubling (invariant 16.6-E / D10).
    let src_agent_frozen = workspace_root
        .join("agents")
        .join("dist")
        .join(format!("ail-agent{exe_suffix}"));
    let dst_agent_frozen = binaries_dir.join(format!("ail-agent{exe_suffix}"));
    println!("cargo:rerun-if-changed={}", src_agent_frozen.display());
    if src_agent_frozen.exists() {
        std::fs::copy(&src_agent_frozen, &dst_agent_frozen)?;
    }

    Ok(())
}
