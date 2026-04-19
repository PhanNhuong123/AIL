//! `ail agent <task>` — spawn the Python LangGraph agent as a child process.
//!
//! Bridges argv to `python -m ail_agent`, with `Stdio::inherit()` so progress
//! lines stream live to the terminal.
//!
//! # Python probe
//!
//! Rather than adding the `which` crate as a new dependency, this module uses
//! an in-process probe: it tries to spawn `python --version` (then `python3
//! --version`) with stdout/stderr suppressed. If the spawn succeeds the
//! candidate binary name is returned; if both fail `CliError::AgentNotInstalled`
//! is returned. This is fully cross-platform and adds no new deps.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::CliError;

/// Arguments parsed from clap, normalised for `run_agent`.
pub struct AgentArgs {
    pub task: String,
    pub model: Option<String>,
    pub mcp_port: u16,
    pub max_iterations: Option<usize>,
    pub steps_per_plan: Option<usize>,
}

/// Entry point for `ail agent`.
///
/// Probes for a usable `python` binary, builds the `python -m ail_agent ...`
/// invocation with inherited stdio, and maps the child exit code to
/// [`CliError`].
pub fn run_agent(cwd: &Path, args: &AgentArgs) -> Result<(), CliError> {
    let python = find_python()?;

    let mut cmd = Command::new(&python);
    cmd.arg("-m").arg("ail_agent").arg(&args.task);

    if let Some(m) = &args.model {
        cmd.arg("--model").arg(m);
    }
    cmd.arg("--mcp-port").arg(args.mcp_port.to_string());
    if let Some(n) = args.max_iterations {
        cmd.arg("--max-iterations").arg(n.to_string());
    }
    if let Some(n) = args.steps_per_plan {
        cmd.arg("--steps-per-plan").arg(n.to_string());
    }

    cmd.current_dir(cwd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = cmd.status().map_err(|e| {
        // If the OS-level spawn fails even after a successful probe (e.g. the
        // binary was removed between probe and exec), report NotFound clearly.
        if e.kind() == std::io::ErrorKind::NotFound {
            CliError::AgentNotInstalled
        } else {
            CliError::AgentFailed {
                code: 1,
                message: format!("failed to spawn `{}`: {}", python, e),
            }
        }
    })?;

    if status.success() {
        return Ok(());
    }
    let code = status.code().unwrap_or(1);
    Err(CliError::AgentFailed {
        code,
        message: format!("agent exited with code {code}"),
    })
}

/// Probe `python` then `python3` by attempting a `--version` spawn.
///
/// Stdout and stderr are suppressed so no output leaks to the terminal during
/// the probe. Returns the first binary name whose spawn succeeds, or
/// [`CliError::AgentNotInstalled`] if neither is available.
fn find_python() -> Result<String, CliError> {
    for candidate in ["python", "python3"] {
        let probe = Command::new(candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if probe.is_ok() {
            return Ok(candidate.to_string());
        }
    }
    Err(CliError::AgentNotInstalled)
}
