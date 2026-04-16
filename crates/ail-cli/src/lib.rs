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
    migrate::{
        migrate_graph, run_export, run_migrate, run_verify_graph, MigrationReport, VerifyResult,
    },
    reindex::run_reindex,
    run_cmd::run_run,
    search::run_search,
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
            target,
        } => {
            let args = BuildArgs {
                contracts: contracts.as_deref(),
                source_map,
                watch,
                check_breaking,
                check_migration,
                target: target.as_deref(),
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

        Command::Search {
            query,
            budget,
            setup,
            semantic,
            bm25_only,
        } => run_search(&cwd, query.as_deref(), budget, setup, semantic, bm25_only),

        Command::Reindex { embeddings } => run_reindex(&cwd, embeddings),

        Command::Migrate { from, to, verify } => run_migrate(&from, &to, verify),

        Command::Export { from, to } => run_export(&from, &to),
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

        /// Emission target language: python (default) or typescript.
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

    /// Set up or run semantic search for the current project.
    Search {
        /// Query string. Omit when using --setup.
        query: Option<String>,

        /// Maximum number of results to return.
        #[arg(long, default_value_t = 20)]
        budget: usize,

        /// Verify the embedding model directory and print setup instructions.
        #[arg(long)]
        setup: bool,

        /// Use hybrid (BM25 + semantic) search with ONNX embeddings.
        #[arg(long)]
        semantic: bool,

        /// Use BM25 keyword search only (no semantic ranking).
        #[arg(long)]
        bm25_only: bool,
    },

    /// Rebuild the embedding index for the current project database.
    Reindex {
        /// Clear and rebuild embedding vectors (requires model files at ~/.ail/models/).
        #[arg(long)]
        embeddings: bool,
    },

    /// Migrate a filesystem `.ail` project to a `.ail.db` SQLite database.
    Migrate {
        /// Source directory containing `.ail` files.
        #[arg(long)]
        from: PathBuf,

        /// Destination path for the new `.ail.db` file.
        #[arg(long)]
        to: PathBuf,

        /// Verify roundtrip fidelity after migrating.
        #[arg(long)]
        verify: bool,
    },

    /// Export a `.ail.db` database back to `.ail` text files.
    ///
    /// Output is written to `<to>/export.ail` as a single file.
    Export {
        /// Source `.ail.db` database path.
        #[arg(long)]
        from: PathBuf,

        /// Destination directory for the exported `.ail` file.
        #[arg(long)]
        to: PathBuf,
    },
}
