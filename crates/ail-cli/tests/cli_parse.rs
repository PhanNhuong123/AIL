//! Clap argument-parsing tests for the `ail` CLI.
//!
//! Each test calls `Cli::try_parse_from` with a synthetic argument list and
//! asserts that the parsed subcommand contains the expected field values.
//! These tests verify the clap configuration, not command logic.

use std::path::PathBuf;

use ail_cli::{Cli, Command};
use clap::Parser;

fn parse(args: &[&str]) -> Command {
    Cli::try_parse_from(args).expect("parse failed").command
}

#[test]
fn cli_parse_init_with_name() {
    let cmd = parse(&["ail", "init", "myproject"]);
    let Command::Init { name } = cmd else {
        panic!("expected Init");
    };
    assert_eq!(name, "myproject");
}

#[test]
fn cli_parse_build_defaults() {
    let cmd = parse(&["ail", "build"]);
    let Command::Build {
        watch,
        contracts,
        source_map,
        check_breaking,
        check_migration,
        target,
        from_db: _,
    } = cmd
    else {
        panic!("expected Build");
    };
    assert!(!watch);
    assert!(contracts.is_none());
    assert!(!source_map);
    assert!(!check_breaking);
    assert!(!check_migration);
    assert!(target.is_none());
}

#[test]
fn cli_parse_build_contracts_off() {
    let cmd = parse(&["ail", "build", "--contracts", "off"]);
    let Command::Build { contracts, .. } = cmd else {
        panic!("expected Build");
    };
    assert_eq!(contracts.as_deref(), Some("off"));
}

#[test]
fn cli_parse_build_source_map() {
    let cmd = parse(&["ail", "build", "--source-map"]);
    let Command::Build { source_map, .. } = cmd else {
        panic!("expected Build");
    };
    assert!(source_map);
}

#[test]
fn cli_parse_build_watch() {
    let cmd = parse(&["ail", "build", "--watch"]);
    let Command::Build { watch, .. } = cmd else {
        panic!("expected Build");
    };
    assert!(watch);
}

#[test]
fn cli_parse_build_check_breaking() {
    let cmd = parse(&["ail", "build", "--check-breaking"]);
    let Command::Build { check_breaking, .. } = cmd else {
        panic!("expected Build");
    };
    assert!(check_breaking);
}

#[test]
fn cli_parse_build_check_migration() {
    let cmd = parse(&["ail", "build", "--check-migration"]);
    let Command::Build {
        check_migration, ..
    } = cmd
    else {
        panic!("expected Build");
    };
    assert!(check_migration);
}

#[test]
fn cli_parse_build_target_python() {
    let cmd = parse(&["ail", "build", "--target", "python"]);
    let Command::Build { target, .. } = cmd else {
        panic!("expected Build");
    };
    assert_eq!(target.as_deref(), Some("python"));
}

#[test]
fn cli_parse_build_target_typescript() {
    let cmd = parse(&["ail", "build", "--target", "typescript"]);
    let Command::Build { target, .. } = cmd else {
        panic!("expected Build");
    };
    assert_eq!(target.as_deref(), Some("typescript"));
}

#[test]
fn cli_parse_verify_no_file() {
    let cmd = parse(&["ail", "verify"]);
    let Command::Verify {
        file,
        from_db: _,
        format,
    } = cmd
    else {
        panic!("expected Verify");
    };
    assert!(file.is_none());
    assert_eq!(format, "text");
}

#[test]
fn cli_parse_verify_with_file() {
    let cmd = parse(&["ail", "verify", "src/main.ail"]);
    let Command::Verify {
        file,
        from_db: _,
        format,
    } = cmd
    else {
        panic!("expected Verify");
    };
    assert_eq!(file, Some(PathBuf::from("src/main.ail")));
    assert_eq!(format, "text");
}

#[test]
fn cli_parse_verify_format_json() {
    let cmd = parse(&["ail", "verify", "--format", "json"]);
    let Command::Verify {
        file,
        from_db: _,
        format,
    } = cmd
    else {
        panic!("expected Verify");
    };
    assert!(file.is_none());
    assert_eq!(format, "json");
}

#[test]
fn cli_parse_serve() {
    let cmd = parse(&["ail", "serve"]);
    assert!(matches!(cmd, Command::Serve));
}

// ── Phase 10 gap closure: search + reindex parse tests ───────────────────────

#[test]
fn t111_cli_parses_search_setup() {
    let cmd = parse(&["ail", "search", "--setup"]);
    let Command::Search {
        query,
        budget,
        setup,
        semantic,
        bm25_only,
    } = cmd
    else {
        panic!("expected Search");
    };
    assert!(setup, "--setup flag must be true");
    assert!(
        query.is_none(),
        "query must be None when only --setup is given"
    );
    assert_eq!(budget, 20, "default budget must be 20");
    assert!(!semantic);
    assert!(!bm25_only);
}

#[test]
fn t111_cli_parses_search_query() {
    let cmd = parse(&["ail", "search", "wallet balance"]);
    let Command::Search {
        query,
        setup,
        semantic,
        bm25_only,
        ..
    } = cmd
    else {
        panic!("expected Search");
    };
    assert_eq!(query.as_deref(), Some("wallet balance"));
    assert!(!setup);
    assert!(!semantic);
    assert!(!bm25_only);
}

#[test]
fn t111_cli_parses_search_semantic() {
    let cmd = parse(&["ail", "search", "--semantic", "wallet balance"]);
    let Command::Search {
        query,
        semantic,
        bm25_only,
        ..
    } = cmd
    else {
        panic!("expected Search");
    };
    assert_eq!(query.as_deref(), Some("wallet balance"));
    assert!(semantic, "--semantic flag must be true");
    assert!(!bm25_only);
}

#[test]
fn t111_cli_parses_reindex() {
    let cmd = parse(&["ail", "reindex"]);
    let Command::Reindex { embeddings } = cmd else {
        panic!("expected Reindex");
    };
    assert!(!embeddings, "default --embeddings must be false");
}

#[test]
fn t111_cli_parses_reindex_embeddings() {
    let cmd = parse(&["ail", "reindex", "--embeddings"]);
    let Command::Reindex { embeddings } = cmd else {
        panic!("expected Reindex");
    };
    assert!(embeddings, "--embeddings flag must be true");
}

// ── Phase 14 task 14.3: agent parse tests ────────────────────────────────────

/// Minimal `ail agent "Add validation"` — only required positional arg.
#[test]
fn command_agent_minimal() {
    let cmd = parse(&["ail", "agent", "Add validation"]);
    let Command::Agent {
        task,
        model,
        mcp_port,
        max_iterations,
        steps_per_plan,
    } = cmd
    else {
        panic!("expected Agent");
    };
    assert_eq!(task, "Add validation");
    assert!(model.is_none(), "model must default to None");
    assert_eq!(mcp_port, 7777, "mcp_port must default to 7777");
    assert!(
        max_iterations.is_none(),
        "max_iterations must default to None"
    );
    assert!(
        steps_per_plan.is_none(),
        "steps_per_plan must default to None"
    );
}

/// All flags populated: `ail agent "X" --model ... --mcp-port ... --max-iterations ... --steps-per-plan ...`
#[test]
fn command_agent_all_flags() {
    let cmd = parse(&[
        "ail",
        "agent",
        "X",
        "--model",
        "anthropic:m",
        "--mcp-port",
        "9999",
        "--max-iterations",
        "100",
        "--steps-per-plan",
        "30",
    ]);
    let Command::Agent {
        task,
        model,
        mcp_port,
        max_iterations,
        steps_per_plan,
    } = cmd
    else {
        panic!("expected Agent");
    };
    assert_eq!(task, "X");
    assert_eq!(model.as_deref(), Some("anthropic:m"));
    assert_eq!(mcp_port, 9999);
    assert_eq!(max_iterations, Some(100));
    assert_eq!(steps_per_plan, Some(30));
}

/// Default mcp_port is always 7777 — locked default, must not change.
#[test]
fn command_agent_default_mcp_port() {
    let cmd = parse(&["ail", "agent", "some task"]);
    let Command::Agent { mcp_port, .. } = cmd else {
        panic!("expected Agent");
    };
    assert_eq!(mcp_port, 7777, "default mcp_port must be exactly 7777");
}

/// `ail agent` with no positional task arg must fail to parse.
#[test]
fn command_agent_missing_task_fails() {
    let result = Cli::try_parse_from(["ail", "agent"]);
    assert!(
        result.is_err(),
        "parsing `ail agent` with no task should fail"
    );
}

// ── Phase 17 task 17.3: sheaf parse tests ────────────────────────────────────

/// `ail sheaf` with no args → all fields None.
#[test]
fn cli_parse_sheaf_default() {
    let cmd = parse(&["ail", "sheaf"]);
    let Command::Sheaf {
        node,
        format,
        from_db,
    } = cmd
    else {
        panic!("expected Sheaf");
    };
    assert!(node.is_none(), "node must be None by default");
    assert!(format.is_none(), "format must be None by default");
    assert!(from_db.is_none(), "from_db must be None by default");
}

/// `ail sheaf --node transfer_money` → `node = Some("transfer_money")`.
#[test]
fn cli_parse_sheaf_with_node() {
    let cmd = parse(&["ail", "sheaf", "--node", "transfer_money"]);
    let Command::Sheaf { node, .. } = cmd else {
        panic!("expected Sheaf");
    };
    assert_eq!(node.as_deref(), Some("transfer_money"));
}

/// `ail sheaf --format json` → `format = Some("json")`.
#[test]
fn cli_parse_sheaf_format_json() {
    let cmd = parse(&["ail", "sheaf", "--format", "json"]);
    let Command::Sheaf { format, .. } = cmd else {
        panic!("expected Sheaf");
    };
    assert_eq!(format.as_deref(), Some("json"));
}

/// `ail sheaf --format text` → `format = Some("text")`.
#[test]
fn cli_parse_sheaf_format_text() {
    let cmd = parse(&["ail", "sheaf", "--format", "text"]);
    let Command::Sheaf { format, .. } = cmd else {
        panic!("expected Sheaf");
    };
    assert_eq!(format.as_deref(), Some("text"));
}

/// `ail sheaf --node transfer_money --format json` → both fields set.
#[test]
fn cli_parse_sheaf_with_node_and_format() {
    let cmd = parse(&[
        "ail",
        "sheaf",
        "--node",
        "transfer_money",
        "--format",
        "json",
    ]);
    let Command::Sheaf {
        node,
        format,
        from_db,
    } = cmd
    else {
        panic!("expected Sheaf");
    };
    assert_eq!(node.as_deref(), Some("transfer_money"));
    assert_eq!(format.as_deref(), Some("json"));
    assert!(from_db.is_none());
}

/// `ail sheaf --from-db /some/path.ail.db` → `from_db = Some(PathBuf)`.
#[test]
fn cli_parse_sheaf_with_from_db() {
    let cmd = parse(&["ail", "sheaf", "--from-db", "/some/path.ail.db"]);
    let Command::Sheaf { from_db, .. } = cmd else {
        panic!("expected Sheaf");
    };
    assert_eq!(from_db, Some(PathBuf::from("/some/path.ail.db")));
}
