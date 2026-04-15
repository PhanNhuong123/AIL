//! `ail-cli` library — exposes the CLI types for testing.
//!
//! The binary entry-point (`main.rs`) calls [`run`]. Tests import [`Cli`] and
//! [`Command`] directly so they can exercise clap argument parsing without
//! spawning a subprocess.

pub mod commands;
pub mod error;

use std::env;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

pub use commands::{
    build::{run_build, BuildArgs},
    init::run_init,
    run_cmd::run_run,
    serve::run_serve,
    status::run_status,
    test_cmd::run_test,
    verify::run_verify,
};
pub use error::CliError;

/// Run the CLI from the process argument list.
pub fn run() -> Result<(), CliError> {
    let cli = Cli::parse();
    let cwd = env::current_dir()?;

    match cli.command {
        Command::Init { name } => run_init(&cwd, &name),

        Command::Build {
            watch,
            contracts,
            source_map,
            check_breaking,
            check_migration,
            target: _,
        } => {
            let args = BuildArgs {
                contracts: contracts.as_deref(),
                source_map,
                watch,
                check_breaking,
                check_migration,
            };
            run_build(&cwd, &args)
        }

        Command::Verify { file } => {
            let file_path = file.as_deref();
            run_verify(&cwd, file_path)
        }

        Command::Test => run_test(&cwd),

        Command::Run => run_run(&cwd),

        Command::Serve => run_serve(&cwd),

        Command::Status => run_status(&cwd),
    }
}

// ── Clap types ────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "ail", about = "AIL compiler and toolchain")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Scaffold a new AIL project directory.
    Init {
        /// Name of the new project.
        name: String,
    },

    /// Run the full pipeline and emit Python output files.
    Build {
        /// Rebuild whenever `.ail` sources change (mtime polling, 500ms interval).
        #[arg(long)]
        watch: bool,

        /// Contract emission mode: on (default), comments, or off.
        #[arg(long, value_name = "MODE")]
        contracts: Option<String>,

        /// Print the generated source map JSON to stdout after building.
        #[arg(long)]
        source_map: bool,

        /// Detect breaking API changes (not yet implemented).
        #[arg(long)]
        check_breaking: bool,

        /// Generate migration hints when types change (not yet implemented).
        #[arg(long)]
        check_migration: bool,

        /// Emission target language (v0.1: python only).
        #[arg(long)]
        target: Option<String>,
    },

    /// Verify the project pipeline without emitting output.
    Verify {
        /// Path hint — v0.1 always verifies the whole project.
        file: Option<PathBuf>,
    },

    /// Build the project and run generated pytest contract tests.
    Test,

    /// Build and run the project entry point (not yet implemented in v0.1).
    Run,

    /// Start the AIL MCP server over stdio.
    Serve,

    /// Show the highest pipeline stage reached and node/edge counts.
    Status,
}
