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
