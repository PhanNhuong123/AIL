use std::env;
use std::path::PathBuf;

fn main() {
    if let Err(e) = materialize_sidecars() {
        println!("cargo:warning=sidecar materialization skipped: {e}");
    }
    tauri_build::build();
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
