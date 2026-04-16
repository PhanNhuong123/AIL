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
    let Command::Verify { file } = cmd else {
        panic!("expected Verify");
    };
    assert!(file.is_none());
}

#[test]
fn cli_parse_verify_with_file() {
    let cmd = parse(&["ail", "verify", "src/main.ail"]);
    let Command::Verify { file } = cmd else {
        panic!("expected Verify");
    };
    assert_eq!(file, Some(PathBuf::from("src/main.ail")));
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
