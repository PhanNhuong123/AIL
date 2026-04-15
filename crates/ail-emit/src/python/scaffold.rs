use ail_contract::VerifiedGraph;

use crate::types::{EmitOutput, EmittedFile, FileOwnership};

/// Emit scaffold files — starter templates that are created once and never overwritten.
///
/// Returns a single `scaffolded/__init__.py` that imports from the generated package.
/// Developers can edit this file freely; the emitter will not touch it after creation.
///
/// The `_verified` parameter is accepted for API consistency with other emitters.
/// It is unused in v0.1 because the scaffold is a fixed template.
///
/// ## Layout assumption (v0.1)
/// `scaffolded/` and `generated/` are peer directories under the same output root.
/// The scaffold uses absolute imports (`from generated.types import *`) that are
/// valid when the output root is on `PYTHONPATH`.
pub fn emit_scaffold_files(_verified: &VerifiedGraph) -> EmitOutput {
    EmitOutput {
        files: vec![EmittedFile {
            path: "scaffolded/__init__.py".to_owned(),
            content: SCAFFOLD_INIT_PY.to_owned(),
            ownership: FileOwnership::Scaffolded,
        }],
    }
}

/// Content of `scaffolded/__init__.py`.
///
/// Uses absolute imports because `scaffolded/` and `generated/` are sibling directories.
/// The wildcard imports are safe because `generated/types.py` and
/// `generated/functions.py` both define `__all__`.
const SCAFFOLD_INIT_PY: &str = "\
# Scaffold entrypoint — created once, never overwritten.
# Edit freely: add re-exports, application wiring, or custom logic.
from __future__ import annotations

from generated.types import *
from generated.functions import *
";
