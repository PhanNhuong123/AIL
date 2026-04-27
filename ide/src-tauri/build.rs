use std::env;
use std::path::PathBuf;

fn main() {
    if let Err(e) = materialize_sidecars() {
        println!("cargo:warning=sidecar materialization skipped: {e}");
    }
    tauri_build::build();
}

/// Copy built sidecar binaries into `binaries/` with Tauri target-triple suffix.
///
/// This runs at build time. When source files are absent (clean checkout, CI
/// without a prior `cargo build --release -p ail-cli`), the function is a
/// no-op — `cargo check` stays clean (invariant 16.5-F).
///
/// `rerun-if-changed` directives are emitted OUTSIDE the `if src.exists()`
/// guards so cargo re-runs this script when source files are added or deleted
/// later — not just when they change (I1 fix).
///
/// Both `ail-agent.cmd` and `ail-agent.sh` are always copied regardless of
/// build triple: the files are tiny and `bundle.resources` references both
/// paths, so the bundler never warns about a missing resource on any platform
/// (M2 fix).
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

    // Copy ail-cli binary.
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

    // Copy ail-agent wrapper scripts.
    // The agent wrapper is NOT a Tauri sidecar (no triple-suffix naming) —
    // it is invoked via raw tokio::process::Command in agent.rs (D1 decision).
    // Both .cmd and .sh are always copied regardless of build triple so
    // bundle.resources references to both always succeed (M2 fix).
    let src_agent_cmd = workspace_root
        .join("agents")
        .join("scripts")
        .join("ail-agent.cmd");
    let src_agent_sh = workspace_root
        .join("agents")
        .join("scripts")
        .join("ail-agent.sh");

    // Rerun directives outside the exists() guards (I1 fix).
    println!("cargo:rerun-if-changed={}", src_agent_cmd.display());
    println!("cargo:rerun-if-changed={}", src_agent_sh.display());

    if src_agent_cmd.exists() {
        std::fs::copy(&src_agent_cmd, binaries_dir.join("ail-agent.cmd"))?;
    }
    if src_agent_sh.exists() {
        std::fs::copy(&src_agent_sh, binaries_dir.join("ail-agent.sh"))?;
    }

    Ok(())
}
