//! Čech nerve construction for Phase 17 sheaf consistency.
//!
//! Four invariants govern the output:
//!
//! 1. **Do-only sections** — only nodes with `Pattern::Do` receive a
//!    [`SheafSection`]; Describe, Check, and all other patterns are excluded.
//! 2. **Direct parent-child overlaps only** — the nerve records a
//!    [`SheafOverlap`] for each direct (depth-1) parent → child Do pair; no
//!    transitive grandparent-grandchild overlaps are emitted.
//! 3. **Variable-shared sibling overlaps** — a sibling overlap is only emitted
//!    when the two sibling sections share at least one top-level variable name;
//!    fully disjoint sibling pairs are skipped (review issue 17.1-A).
//! 4. **Deterministic output** — sections are sorted by `node_id.to_string()`;
//!    overlaps are sorted by `(node_a.to_string(), node_b.to_string())`.

mod builder;
mod types;

#[cfg(test)]
mod tests;

pub use builder::build_nerve;
pub use types::{CechNerve, SheafOverlap, SheafSection};
