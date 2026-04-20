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
    agent::{read_agent_config, run_agent, AgentArgs, AgentConfig},
    build::{run_build, BuildArgs},
    context::run_context,
    coverage::{read_coverage_config, run_coverage},
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
            from_db,
        } => {
            let args = BuildArgs {
                contracts: contracts.as_deref(),
                source_map,
                watch,
                check_breaking,
                check_migration,
                target: target.as_deref(),
                from_db: from_db.as_deref(),
            };
            run_build(&cwd, &args)
        }

        Command::Verify { file, from_db } => {
            let file_path = file.as_deref();
            run_verify(&cwd, file_path, from_db.as_deref())
        }

        Command::Context {
            task,
            node,
            from_db,
        } => run_context(&cwd, task.as_deref(), node.as_deref(), from_db.as_deref()),

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

        Command::Coverage {
            node,
            all,
            warm_cache,
            from_db,
        } => run_coverage(&cwd, node, all, warm_cache, from_db),

        Command::Agent {
            task,
            model,
            mcp_port,
            max_iterations,
            steps_per_plan,
        } => {
            let args = commands::agent::AgentArgs {
                task,
                model,
                mcp_port,
                max_iterations,
                steps_per_plan,
            };
            commands::agent::run_agent(&cwd, &args)
        }
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

        /// Load the project from this `.ail.db` database instead of auto-detecting.
        #[arg(long, value_name = "PATH")]
        from_db: Option<PathBuf>,
    },

    /// Verify the project pipeline without emitting output.
    Verify {
        /// Path hint — v0.1 always verifies the whole project.
        file: Option<PathBuf>,

        /// Load the project from this `.ail.db` database instead of auto-detecting.
        #[arg(long, value_name = "PATH")]
        from_db: Option<PathBuf>,
    },

    /// Print a CIC context packet for a task or named node.
    ///
    /// Second and later calls for the same node hit the SQLite CIC cache.
    Context {
        /// Natural-language task description. Selects the most relevant `Do` node by BM25.
        #[arg(long, value_name = "TEXT")]
        task: Option<String>,

        /// Target a named node directly (e.g. `transfer_money`). Takes precedence over `--task`.
        #[arg(long, value_name = "NAME")]
        node: Option<String>,

        /// Load the project from this `.ail.db` database instead of auto-detecting.
        #[arg(long, value_name = "PATH")]
        from_db: Option<PathBuf>,
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

    /// Compute and display semantic coverage for project nodes.
    ///
    /// Requires the SQLite backend and (for computation) the `embeddings` feature.
    Coverage {
        /// Compute/display coverage for a single named node or node id.
        #[arg(long, value_name = "NAME_OR_ID")]
        node: Option<String>,

        /// Summarise coverage across all non-leaf nodes.
        #[arg(long, conflicts_with = "node")]
        all: bool,

        /// Recompute and persist coverage for all non-leaf nodes (warms the cache).
        #[arg(long = "warm-cache", conflicts_with = "node", conflicts_with_all = ["all"])]
        warm_cache: bool,

        /// Override the SQLite database path.
        #[arg(long, value_name = "PATH")]
        from_db: Option<PathBuf>,
    },

    /// Run the LangGraph agent against a natural-language task.
    Agent {
        /// Natural-language task to perform.
        task: String,

        /// Provider and model in 'provider:model' form (e.g. 'anthropic:claude-sonnet-4-5', 'openai:gpt-4o'). Falls back to '[agent] model' in ail.config.toml, then the Python-side default 'anthropic:claude-sonnet-4-5'.
        #[arg(long)]
        model: Option<String>,

        /// Reserved for a future network-MCP transport; the current implementation spawns 'ail serve' over stdio and ignores this value.
        #[arg(long, default_value_t = 7777)]
        mcp_port: u16,

        /// Maximum planner/coder/verify loop iterations. Falls back to '[agent] max_iterations', then 50.
        #[arg(long)]
        max_iterations: Option<usize>,

        /// Maximum coder steps per plan before forced replan (AIL-G0143 budget guard). Falls back to '[agent] steps_per_plan', then 20.
        #[arg(long)]
        steps_per_plan: Option<usize>,
    },
}
