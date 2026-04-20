//! Tests for `read_agent_config` — the TOML `[agent]` section parser.
//!
//! Mirrors `coverage_config.rs`: feature-independent, pure TOML-parsing logic
//! exercised against the filesystem via `tempfile`.

use std::fs;
use std::path::Path;

use ail_cli::read_agent_config;

fn write_config(root: &Path, body: &str) {
    fs::write(root.join("ail.config.toml"), body).unwrap();
}

#[test]
fn t154_absent_file_returns_default() {
    let tmp = tempfile::tempdir().unwrap();

    let cfg = read_agent_config(tmp.path());

    assert!(cfg.model.is_none(), "model should default to None");
    assert!(
        cfg.max_iterations.is_none(),
        "max_iterations should default to None"
    );
    assert!(
        cfg.steps_per_plan.is_none(),
        "steps_per_plan should default to None"
    );
}

#[test]
fn t154_absent_section_returns_default() {
    let tmp = tempfile::tempdir().unwrap();

    // Write a config with a different section but NOT [agent].
    write_config(tmp.path(), "[database]\nbackend = \"auto\"\n");

    let cfg = read_agent_config(tmp.path());

    assert!(cfg.model.is_none());
    assert!(cfg.max_iterations.is_none());
    assert!(cfg.steps_per_plan.is_none());
}

#[test]
fn t154_parses_model_with_double_quotes() {
    let tmp = tempfile::tempdir().unwrap();

    write_config(
        tmp.path(),
        "[agent]\nmodel = \"anthropic:claude-sonnet-4-5\"\n",
    );

    let cfg = read_agent_config(tmp.path());

    assert_eq!(cfg.model.as_deref(), Some("anthropic:claude-sonnet-4-5"));
    assert!(cfg.max_iterations.is_none());
    assert!(cfg.steps_per_plan.is_none());
}

#[test]
fn t154_parses_max_iterations() {
    let tmp = tempfile::tempdir().unwrap();

    write_config(tmp.path(), "[agent]\nmax_iterations = 100\n");

    let cfg = read_agent_config(tmp.path());

    assert_eq!(cfg.max_iterations, Some(100));
    assert!(cfg.model.is_none());
    assert!(cfg.steps_per_plan.is_none());
}

#[test]
fn t154_parses_steps_per_plan() {
    let tmp = tempfile::tempdir().unwrap();

    write_config(tmp.path(), "[agent]\nsteps_per_plan = 30\n");

    let cfg = read_agent_config(tmp.path());

    assert_eq!(cfg.steps_per_plan, Some(30));
    assert!(cfg.model.is_none());
    assert!(cfg.max_iterations.is_none());
}

#[test]
fn t154_ignores_unknown_keys() {
    let tmp = tempfile::tempdir().unwrap();

    write_config(
        tmp.path(),
        "[agent]\nfoo = \"bar\"\nmodel = \"openai:gpt-4o\"\n",
    );

    let cfg = read_agent_config(tmp.path());

    // Unknown `foo` key is silently skipped; `model` still parses.
    assert_eq!(cfg.model.as_deref(), Some("openai:gpt-4o"));
}

#[test]
fn t154_ignores_timeout_seconds_key() {
    let tmp = tempfile::tempdir().unwrap();

    // `timeout_seconds` is documented in the reference spec but not yet
    // supported by the Python side. It must parse without panic and leave
    // the config at defaults.
    write_config(
        tmp.path(),
        "[agent]\ntimeout_seconds = 120\nmax_iterations = 75\n",
    );

    let cfg = read_agent_config(tmp.path());

    assert_eq!(cfg.max_iterations, Some(75));
    assert!(cfg.model.is_none());
    assert!(cfg.steps_per_plan.is_none());
}
