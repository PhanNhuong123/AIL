//! `ail init <name>` — scaffold a new AIL project directory.
//!
//! Creates:
//! ```text
//! <name>/
//! ├── ail.config.toml   ← default project config
//! ├── src/
//! │   └── main.ail      ← skeleton AIL file
//! ├── generated/        ← emitter output (AIL-owned)
//! └── scaffolded/       ← scaffold output (developer-owned after first write)
//! ```

use std::fs;
use std::path::Path;

use crate::error::CliError;

/// Create a new AIL project named `name` under `parent`.
///
/// Callers pass the parent directory explicitly so tests can use a temp
/// directory without mutating the process current directory.
pub fn run_init(parent: &Path, name: &str) -> Result<(), CliError> {
    let root = parent.join(name);

    fs::create_dir_all(root.join("src"))?;
    fs::create_dir_all(root.join("generated"))?;
    fs::create_dir_all(root.join("scaffolded"))?;

    fs::write(root.join("src").join("main.ail"), skeleton_ail())?;
    fs::write(root.join("ail.config.toml"), default_config(name))?;

    println!("Initialized project '{name}'.");
    Ok(())
}

fn skeleton_ail() -> &'static str {
    "describe Main as\n  id:Text\n"
}

fn default_config(name: &str) -> String {
    format!(
        r#"[project]
name = "{name}"
version = "0.1.0"

[build]
target = "python"
contracts = "on"
source_map = true
async = false
"#
    )
}
