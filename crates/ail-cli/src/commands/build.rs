//! `ail build` — run the full pipeline and emit Python output files.
//!
//! Flags:
//! - `--contracts on|comments|off`  controls contract emission (default `on`)
//! - `--source-map`                 print the source map JSON to stdout after build
//! - `--watch`                      poll `src/*.ail` for mtime changes and rebuild
//! - `--check-breaking`             not yet implemented
//! - `--check-migration`            not yet implemented
//! - `--target <target>`            reserved; only `python` is supported in v0.1

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

use ail_contract::verify;
use ail_emit::{
    emit_function_definitions, emit_scaffold_files, emit_type_definitions, ContractMode,
    EmitConfig, FileOwnership,
};
use ail_graph::validation::validate_graph;
use ail_text::parse_directory;
use ail_types::type_check;

use crate::error::CliError;

/// Arguments for `ail build`, forwarded from the clap subcommand.
pub struct BuildArgs<'a> {
    pub contracts: Option<&'a str>,
    pub source_map: bool,
    pub watch: bool,
    pub check_breaking: bool,
    pub check_migration: bool,
}

/// Entry point for `ail build`.
pub fn run_build(root: &Path, args: &BuildArgs<'_>) -> Result<(), CliError> {
    // Reject not-yet-implemented flags early.
    if args.check_breaking {
        return Err(CliError::NotImplemented {
            feature: "--check-breaking",
        });
    }
    if args.check_migration {
        return Err(CliError::NotImplemented {
            feature: "--check-migration",
        });
    }

    let contract_mode = parse_contract_mode(args.contracts)?;
    let config = EmitConfig {
        contract_mode,
        async_mode: false,
        ..Default::default()
    };

    if args.watch {
        run_watch(root, &config, args.source_map)
    } else {
        build_once(root, &config, args.source_map)
    }
}

// ── Core build ────────────────────────────────────────────────────────────────

fn build_once(root: &Path, config: &EmitConfig, print_source_map: bool) -> Result<(), CliError> {
    let verified = run_pipeline(root)?;

    let type_out = emit_type_definitions(&verified).map_err(|errs| CliError::Emit {
        errors: format_emit_errors(&errs),
    })?;
    let fn_out = emit_function_definitions(&verified, config).map_err(|errs| CliError::Emit {
        errors: format_emit_errors(&errs),
    })?;
    let scaffold_out = emit_scaffold_files(&verified);

    let all_files: Vec<_> = type_out
        .files
        .into_iter()
        .chain(fn_out.files)
        .chain(scaffold_out.files)
        .collect();

    let n = all_files.len();
    write_files(root, &all_files)?;

    if print_source_map {
        if let Some(map_file) = all_files.iter().find(|f| f.path.ends_with(".ailmap.json")) {
            println!("{}", map_file.content);
        }
    }

    println!("Built {n} file(s).");
    Ok(())
}

// ── Pipeline ──────────────────────────────────────────────────────────────────

pub(crate) fn run_pipeline(root: &Path) -> Result<ail_contract::VerifiedGraph, CliError> {
    let graph = parse_directory(root).map_err(|e| CliError::Pipeline {
        errors: e.to_string(),
    })?;

    let valid = validate_graph(graph).map_err(|errs| CliError::Pipeline {
        errors: errs
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    })?;

    let typed = type_check(valid, &[]).map_err(|errs| CliError::Pipeline {
        errors: errs
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    })?;

    let verified = verify(typed).map_err(|errs| CliError::Pipeline {
        errors: errs
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    })?;

    Ok(verified)
}

// ── File writing with ownership routing ───────────────────────────────────────

fn write_files(root: &Path, files: &[ail_emit::EmittedFile]) -> Result<(), CliError> {
    for file in files {
        let dest = root.join(&file.path);

        // Ensure the parent directory exists.
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        match file.ownership {
            // AIL owns — always overwrite.
            FileOwnership::Generated => {
                fs::write(&dest, &file.content)?;
            }
            // Developer owns — write only if file does not already exist.
            FileOwnership::Scaffolded => {
                if !dest.exists() {
                    fs::write(&dest, &file.content)?;
                }
            }
        }
    }
    Ok(())
}

// ── Watch mode (mtime polling) ────────────────────────────────────────────────

fn run_watch(root: &Path, config: &EmitConfig, print_source_map: bool) -> Result<(), CliError> {
    // v0.1 uses mtime polling; migrate to the `notify` crate in v0.2 for
    // event-driven watching with fewer race conditions and less CPU overhead.
    println!("[watching] Ctrl-C to stop.");

    let mut last_mtimes = snapshot_ail_mtimes(root);

    // Initial build.
    if let Err(e) = build_once(root, config, print_source_map) {
        eprintln!("[watch] build failed: {e}");
    }

    loop {
        thread::sleep(Duration::from_millis(500));

        let current = snapshot_ail_mtimes(root);
        if current != last_mtimes {
            last_mtimes = current;
            println!("[watch] change detected, rebuilding…");
            if let Err(e) = build_once(root, config, print_source_map) {
                eprintln!("[watch] build failed: {e}");
            }
        }
    }
}

/// Collect the `mtime` of every `*.ail` file reachable under `root`.
fn snapshot_ail_mtimes(root: &Path) -> HashMap<PathBuf, Option<SystemTime>> {
    let mut map = HashMap::new();
    collect_ail_files(root, &mut map);
    map
}

fn collect_ail_files(dir: &Path, map: &mut HashMap<PathBuf, Option<SystemTime>>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_ail_files(&path, map);
        } else if path.extension().and_then(|e| e.to_str()) == Some("ail") {
            let mtime = fs::metadata(&path).ok().and_then(|m| m.modified().ok());
            map.insert(path, mtime);
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Parse `--contracts` string into [`ContractMode`].
fn parse_contract_mode(raw: Option<&str>) -> Result<ContractMode, CliError> {
    match raw {
        None | Some("on") => Ok(ContractMode::On),
        Some("comments") => Ok(ContractMode::Comments),
        Some("off") => Ok(ContractMode::Off),
        Some(other) => Err(CliError::InvalidContracts {
            value: other.to_owned(),
        }),
    }
}

fn format_emit_errors(errs: &[ail_emit::EmitError]) -> String {
    errs.iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}
